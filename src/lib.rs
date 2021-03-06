//! This module contains an extension trait [`LinesExt`](self::LinesExt) for
//! `Stream<Item = Result<String>>` such as those returned by [`AsyncBufReadExt::lines`](futures::io::AsyncBufReadExt::lines).
//! The trait provides the [`chunk_by_line`](self::LinesExt::chunk_by_line) method which groups
//! lines into chunks given a delimiter line that separates chunks.
//!
//! ```
//! use async_std::io::Cursor;
//! use futures::AsyncBufReadExt;
//! use futures::stream::TryStreamExt;
//! use lines_ext::LinesExt;
//! # use std::io::Result;
//! # use async_std::task;
//!
//! # fn main() -> Result<()> {
//! # task::block_on(async {
//! let bytes = b"~~~
//! multi
//! line
//! chunk
//! ~~~
//! another
//! chunk
//! ";
//!
//! let chunks_stream = Cursor::new(bytes).lines().chunk_by_line("~~~");
//!
//! let chunks_vec: Vec<String> = chunks_stream.try_collect().await?;
//! assert_eq!(chunks_vec, vec!["multi\nline\nchunk\n", "another\nchunk\n"]);
//! # Ok(())
//! # })
//! # }
//! ```

use core::pin::Pin;
use futures::future::{self, Ready};
use futures::stream::{
    self, Chain, Once, Scan, Stream, StreamExt, TryFilter, TryFilterMap, TryStreamExt,
};
use futures::task::{Context, Poll};
use pin_project_lite::pin_project;
use std::io::Result;

type ChunkByLineStream<S> = TryFilter<
    TryFilterMap<
        Scan<
            Chain<S, Once<Ready<Result<String>>>>,
            String,
            Ready<Option<Result<Option<String>>>>,
            FnScanner,
        >,
        Ready<Result<Option<String>>>,
        FnFilterNone,
    >,
    Ready<bool>,
    FnFilterEmpty,
>;

type FnScanner = Box<dyn Fn(&mut String, Result<String>) -> Ready<Option<Result<Option<String>>>>>;

type FnFilterNone = fn(Option<String>) -> Ready<Result<Option<String>>>;

type FnFilterEmpty = fn(&String) -> Ready<bool>;

pin_project! {
    /// Stream returned by the [`chunk_by_line`](self::LinesExt::chunk_by_line) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct ChunkByLine<S: Stream<Item = Result<String>>>
    {
        #[pin]
        stream: ChunkByLineStream<S>,
    }
}

impl<S: Stream<Item = Result<String>>> ChunkByLine<S> {
    pub(crate) fn new(lines: S, delim: &str) -> Self {
        let stream = lines
            // Stream of Result<String>
            // append delim so scanner knows when to dump last
            .chain(stream::once(future::ready(Ok(delim.to_owned()))))
            .scan(String::new(), scanner(delim.to_owned()))
            // Stream of Result<Option<String>>
            .try_filter_map((|x| future::ready(Ok(x))) as FnFilterNone)
            // Stream of Result<String>
            .try_filter((|x| future::ready(!x.is_empty())) as FnFilterEmpty);

        Self { stream }
    }
    // delegate_access_inner!(stream, St, ()); // TODO?
}

fn scanner(delim: String) -> FnScanner {
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

impl<S: Stream<Item = Result<String>>> Stream for ChunkByLine<S> {
    type Item = Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        this.stream.as_mut().poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.stream.size_hint()
    }
}

pub trait LinesExt<S: Stream<Item = Result<String>>> {
    fn chunk_by_line(self, delim: &str) -> ChunkByLine<S>;
}

impl<S: Stream<Item = Result<String>>> LinesExt<S> for S {
    fn chunk_by_line(self, delim: &str) -> ChunkByLine<S> {
        ChunkByLine::new(self, delim)
    }
}
