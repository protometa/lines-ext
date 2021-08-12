use anyhow::Result;

use async_std::io::{BufRead, Lines};
use core::pin::Pin;
use futures::future::{self, Ready};
use futures::stream::{
    self, Chain, Once, Scan, Stream, StreamExt, TryFilter, TryFilterMap, TryStreamExt,
};
use futures::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// Stream for the [`chunk_by_line`](self::ChunkByLineExt::chunk_by_line) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct ChunkByLine<S>
    {
        #[pin]
        stream: S,
    }
}

type FnScanner = Box<
    dyn Fn(
        &mut String,
        Result<String, std::io::Error>,
    ) -> Ready<Option<Result<Option<String>, std::io::Error>>>,
>;

type FnFilterNone = Box<dyn Fn(Option<String>) -> Ready<Result<Option<String>, std::io::Error>>>;

type FnFilterEmpty = Box<dyn Fn(&String) -> Ready<bool>>;

type ChunkByLineStream<R> = TryFilter<
    TryFilterMap<
        Scan<
            Chain<Lines<R>, Once<Ready<Result<String, std::io::Error>>>>,
            String,
            Ready<Option<Result<Option<String>, std::io::Error>>>,
            FnScanner,
        >,
        Ready<Result<Option<String>, std::io::Error>>,
        FnFilterNone,
    >,
    Ready<bool>,
    FnFilterEmpty,
>;

impl<R: BufRead> ChunkByLine<ChunkByLineStream<R>> {
    pub(crate) fn new(lines: Lines<R>, delimiter: &str) -> Self {
        let delimiter_cp = delimiter.to_owned();
        // collects chunks in scan state, emitting Some(chunk) on delimiter, else None
        let scanner: FnScanner = Box::new(
            move |state: &mut String,
                  line: Result<String, std::io::Error>|
                  -> Ready<Option<Result<Option<String>, std::io::Error>>> {
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
        );

        let filter_none: FnFilterNone = Box::new(|x| future::ready(Ok(x)));
        let filter_empty: FnFilterEmpty = Box::new(|x: &String| future::ready(x.len() > 0));

        let stream = lines
            // Stream of Result<String>
            // append delimiter so scanner knows when to dump last
            .chain(stream::once(future::ready(Ok(delimiter.to_owned()))))
            .scan(String::new(), scanner)
            // Stream of Result<Option<String>>
            .try_filter_map(filter_none)
            // Stream of Result<String>
            .try_filter(filter_empty);

        Self { stream }
    }
    // delegate_access_inner!(stream, St, ()); // TODO?
}

impl<S: Stream<Item = Result<String, std::io::Error>>> Stream for ChunkByLine<S> {
    type Item = Result<String, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        this.stream.as_mut().poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

pub trait ChunkByLineExt<R: BufRead> {
    fn chunk_by_line(self, delimiter: &str) -> ChunkByLine<ChunkByLineStream<R>>;
}

impl<R: BufRead> ChunkByLineExt<R> for Lines<R> {
    fn chunk_by_line(self, delimiter: &str) -> ChunkByLine<ChunkByLineStream<R>> {
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
    use super::ChunkByLineExt;
    use anyhow::Result;
    use async_std::io::prelude::*;
    use async_std::io::Cursor;
    use futures::stream::TryStreamExt;

    const BYTES: &[u8; 40] = b"~~~
multi

line
chunk
~~~
another
chunk
";

    #[async_std::test]
    async fn chunks_by_line() -> Result<()> {
        // let lines = Cursor::new(bytes2).lines();
        // let docs: Vec<String> = chunk_by_line(lines, "~~~").try_collect().await?;
        let docs: Vec<String> = Cursor::new(BYTES)
            .lines()
            .chunk_by_line("~~~")
            .try_collect()
            .await?;
        assert_eq!(docs, vec!["multi\n\nline\nchunk\n", "another\nchunk\n"]);
        Ok(())
    }
}
