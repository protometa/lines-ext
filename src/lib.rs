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
use futures::stream::{self, Stream, StreamExt, TryStreamExt};
use std::io::Result;

type ChunkByLine<'a> = Pin<Box<dyn Stream<Item = Result<String>> + Send + 'a>>;

pub trait LinesExt<'a, S: Stream<Item = Result<String>> + Send + 'a> {
    fn chunk_by_line(self, delim: &str) -> ChunkByLine<'a>;
}

impl<'a, S: Stream<Item = Result<String>> + Send + 'a> LinesExt<'a, S> for S {
    fn chunk_by_line(self, delim: &str) -> ChunkByLine<'a> {
        self.chain(stream::once(future::ready(Ok(delim.to_owned()))))
            // append delim so scanner knows when to dump last
            .chain(stream::once(future::ready(Ok(delim.to_owned()))))
            .scan(String::new(), scanner(delim.to_owned()))
            // Stream of Result<Option<String>>
            .try_filter_map(|x| future::ready(Ok(x)))
            // Stream of Result<String>
            .try_filter(|x| future::ready(!x.is_empty()))
            .boxed()
    }
}

type FnScanner =
    Box<dyn Fn(&mut String, Result<String>) -> Ready<Option<Result<Option<String>>>> + Send>;

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
