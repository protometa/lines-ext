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

use futures::future::{self, Ready};
use futures::stream::{self, Stream, StreamExt, TryStreamExt};
use std::io::Result;

pub trait LinesExt<'a, S>
where
    S: Stream<Item = Result<String>> + Send + 'a,
{
    fn chunk_by_line(
        self,
        delim: impl Into<String>,
    ) -> impl Stream<Item = Result<String>> + Send + 'a;
}

impl<'a, S> LinesExt<'a, S> for S
where
    S: Stream<Item = Result<String>> + Send + 'a,
{
    fn chunk_by_line(
        self,
        delim: impl Into<String>,
    ) -> impl Stream<Item = Result<String>> + Send + 'a {
        let delim: String = delim.into();
        self
            // append delim so scanner knows when to dump last
            .chain(stream::once(future::ready(Ok(delim.to_owned()))))
            .scan(String::new(), scanner(delim.to_owned()))
            // Stream of Result<Option<String>>
            .try_filter_map(|x| future::ready(Ok(x)))
            // Stream of Result<String>
            .try_filter(|x| future::ready(!x.is_empty()))
    }
}

fn scanner(
    delim: String,
) -> impl Fn(&mut String, Result<String>) -> Ready<Option<Result<Option<String>>>> + Send {
    move |state, line| {
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
    }
}
