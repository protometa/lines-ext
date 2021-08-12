use anyhow::Result;

use async_std::io::{BufRead, Lines};
use core::pin::Pin;
use futures::future::{self, Ready};
use futures::stream::{
    self, Chain, Once, Scan, Stream, StreamExt, TryFilter, TryFilterMap, TryStreamExt,
};
use futures::task::{Context, Poll};
use pin_project_lite::pin_project;

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

type FnScanner = Box<
    dyn Fn(
        &mut String,
        Result<String, std::io::Error>,
    ) -> Ready<Option<Result<Option<String>, std::io::Error>>>,
>;

type FnFilterNone = fn(Option<String>) -> Ready<Result<Option<String>, std::io::Error>>;

type FnFilterEmpty = fn(&String) -> Ready<bool>;

pin_project! {
    /// Stream for the [`chunk_by_line`](self::ChunkByLineExt::chunk_by_line) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct ChunkByLine<R: BufRead>
    {
        #[pin]
        stream: ChunkByLineStream<R>,
    }
}

fn scanner(
    delim: String,
) -> Box<
    dyn Fn(
        &mut String,
        Result<String, std::io::Error>,
    ) -> Ready<Option<Result<Option<String>, std::io::Error>>>,
> {
    Box::new(move |state, line| {
        future::ready(
            line.map(|line| {
                Some(if line == delim {
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
    })
}

impl<R: BufRead> ChunkByLine<R> {
    pub(crate) fn new(lines: Lines<R>, delim: &str) -> Self {
        let stream = lines
            // Stream of Result<String>
            // append delim so scanner knows when to dump last
            .chain(stream::once(future::ready(Ok(delim.to_owned()))))
            .scan(String::new(), scanner(delim.to_owned()))
            // Stream of Result<Option<String>>
            .try_filter_map((|x| future::ready(Ok(x))) as FnFilterNone)
            // Stream of Result<String>
            .try_filter((|x| future::ready(x.len() > 0)) as FnFilterEmpty);

        Self { stream }
    }
    // delegate_access_inner!(stream, St, ()); // TODO?
}

impl<R: BufRead> Stream for ChunkByLine<R> {
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
    fn chunk_by_line(self, delim: &str) -> ChunkByLine<R>;
}

impl<R: BufRead> ChunkByLineExt<R> for Lines<R> {
    fn chunk_by_line(self, delim: &str) -> ChunkByLine<R> {
        ChunkByLine::new(self, delim)
    }
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
        assert_eq!(
            dbg!(docs),
            vec!["multi\n\nline\nchunk\n", "another\nchunk\n"]
        );
        Ok(())
    }
}
