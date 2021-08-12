use anyhow::Result;
use async_std::io;
use async_std::io::prelude::*;
use async_std::io::{stdin, BufRead, BufReader, Cursor, Lines};
use core::pin::Pin;
use futures::future::{self, Future, Ready};
use futures::stream::{
    self, Chain, Once, Scan, Stream, StreamExt, TryFilter, TryFilterMap, TryStream, TryStreamExt,
};
// use clap::{App, Arg};
use futures::task::{Context, Poll};
use pin_project_lite::pin_project;

pub fn chunk_by_line<R: BufRead>(
    lines: Lines<R>,
    delimiter: &str,
) -> impl Stream<Item = Result<String, std::io::Error>> {
    let delimiter2 = delimiter.to_owned();
    lines
        .chain(stream::once(future::ready(Ok(delimiter.to_owned()))))
        // Stream of Result<String>
        .scan(
            String::new(),
            move |state, line| -> Ready<Option<Result<Option<String>, std::io::Error>>> {
                future::ready(
                    line.map(|line| {
                        Some(if line == delimiter2 {
                            let chunk = state.to_owned();
                            state.clear();
                            Some(chunk)
                        } else {
                            state.push_str(&line);
                            state.push('\n');
                            None
                        })
                    })
                    .transpose(),
                )
            },
        )
        // Stream of Result<Option<String>>
        .try_filter_map(|x| future::ready(Ok(x)))
        // Stream of Result<String>
        .try_filter(|x| future::ready(x.len() > 0))
}

pin_project! {
    /// Stream for the [`chunk_by_line`](self::ChunkByLineExt::chunk_by_line) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct ChunkByLine<'a, R: BufRead, F1, F2, F3>
    where
        F1: FnMut( &'a mut String, Result<String, std::io::Error>) -> Ready<Option<Result<Option<String>, std::io::Error>>>,
        F2: FnMut(Option<String>) -> Ready<Result<Option<String>, std::io::Error>>,
        F3: FnMut(String) -> bool,
    {
        #[pin]
        stream: TryFilter<
            TryFilterMap<
                Scan<
                    Chain<
                        Lines<R>,
                        Once<Ready<Result<String, std::io::Error>>>
                    >,
                    String,
                    Ready<Option<Result<Option<String>, std::io::Error>>>,
                    F1
                >, Ready<Result<Option<String>,std::io::Error>>,
                F2
            >,
            Ready<bool>,
            F3
        >,
    }
}

impl<'a, R: BufRead, F1, F2, F3> ChunkByLine<'a, R, F1, F2, F3>
where
    F1: FnMut(
        &'a mut String,
        Result<String, std::io::Error>,
    ) -> Ready<Option<Result<Option<String>, std::io::Error>>>,
    F2: FnMut(Option<String>) -> Ready<Result<Option<String>, std::io::Error>>,
    F3: FnMut(String) -> bool,
{
    pub(crate) fn new(lines: Lines<R>, delimiter: &str) -> Self {
        let delimiter_cp = delimiter.to_owned();
        let stream = lines
            .chain(stream::once(future::ready(Ok(delimiter.to_owned()))))
            // Stream of Result<String>
            .scan(
                String::new(),
                move |state, line| -> Ready<Option<Result<Option<String>, std::io::Error>>> {
                    future::ready(
                        line.map(|line| {
                            Some(if line == delimiter_cp {
                                let chunk = state.to_owned();
                                state.clear();
                                Some(chunk)
                            } else {
                                state.push_str(&line);
                                state.push('\n');
                                None
                            })
                        })
                        .transpose(),
                    )
                },
            )
            // Stream of Result<Option<String>>
            .try_filter_map(|x| future::ready(Ok(x)))
            // Stream of Result<String>
            .try_filter(|x| future::ready(x.len() > 0));
        Self { stream }
    }
    // delegate_access_inner!(stream, St, ());
}

impl<'a, R: BufRead, F1, F2, F3> Stream for ChunkByLine<'a, R, F1, F2, F3>
where
    F1: FnMut(
        &'a mut String,
        Result<String, std::io::Error>,
    ) -> Ready<Option<Result<Option<String>, std::io::Error>>>,
    F2: FnMut(Option<String>) -> Ready<Result<Option<String>, std::io::Error>>,
    F3: FnMut(String) -> bool,
{
    type Item = Result<String, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        this.stream.as_mut().poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

pub trait ChunkByLineExt<'a, R: BufRead, F1, F2, F3>
where
    F1: FnMut(
        &'a mut String,
        Result<String, std::io::Error>,
    ) -> Ready<Option<Result<Option<String>, std::io::Error>>>,
    F2: FnMut(Option<String>) -> Ready<Result<Option<String>, std::io::Error>>,
    F3: FnMut(String) -> bool,
{
    fn chunk_by_line(self, delimiter: &str) -> ChunkByLine<'a, R, F1, F2, F3>;
}

impl<'a, R: BufRead, F1, F2, F3> ChunkByLineExt<'a, R, F1, F2, F3> for Lines<R>
where
    F1: FnMut(
        &'a mut String,
        Result<String, std::io::Error>,
    ) -> Ready<Option<Result<Option<String>, std::io::Error>>>,
    F2: FnMut(Option<String>) -> Ready<Result<Option<String>, std::io::Error>>,
    F3: FnMut(String) -> bool,
{
    fn chunk_by_line(self, delimiter: &str) -> ChunkByLine<'a, R, F1, F2, F3> {
        ChunkByLine::new(self, delimiter)
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    println!("Hello World");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_std::test]
    async fn chunks_by_line() -> Result<()> {
        let lines = Cursor::new(
            b"~~~
multi
line
chunk
~~~
another
chunk
",
        )
        .lines();
        let docs: Vec<String> = chunk_by_line(lines, "~~~").try_collect().await?;
        assert_eq!(docs, vec!["multi\nline\nchunk\n", "another\nchunk\n"]);
        Ok(())
    }
}
