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

// TryFilter<
//     TryFilterMap<
//         Scan<
//             Chain<
//                 Lines<R>,
//                 Once<Ready<Result<String, std::io::Error>>>
//             >,
//             String,
//             Ready<Option<Result<Option<String>, std::io::Error>>>,
//             F1
//         >, Ready<Result<Option<String>,std::io::Error>>,
//         F2
//     >,
//     Ready<bool>,
//     F3
// >
// TryFilter< TryFilterMap< Scan< Chain< Lines<R>, Once<Ready<Result<String, std::io::Error>>> >, String, Ready<Option<Result<Option<String>, std::io::Error>>>, F1 >, Ready<Result<Option<String>,std::io::Error>>, F2 >, Ready<bool>, F3 >
// F1: FnMut( &'a mut String, Result<String, std::io::Error>) -> Ready<Option<Result<Option<String>, std::io::Error>>>,
// F2: FnMut(Option<String>) -> Ready<Result<Option<String>, std::io::Error>>,
// F3: FnMut(String) -> bool,

pin_project! {
    /// Stream for the [`chunk_by_line`](self::ChunkByLineExt::chunk_by_line) method.
    #[must_use = "streams do nothing unless polled"]
    pub struct ChunkByLine<S>
    {
        #[pin]
        stream: S,
    }
}

fn filter_empty(x: &String) -> Ready<bool> {
    future::ready(x.len() > 0)
}

type FnFilterEmpty = fn(&String) -> Ready<bool>;

// type FnScanner = Box<
//     dyn Fn(
//         &mut String,
//         std::result::Result<String, std::io::Error>,
//     )
//         -> futures::future::Ready<Option<std::result::Result<Option<String>, std::io::Error>>>,
// >;
// type FnFilterNone = Box<dyn Fn(Option<String>) -> Ready<Result<Option<String>, std::io::Error>>>;
type FnScanner = Box<
    (dyn for<'r> Fn(
        &'r mut String,
        std::result::Result<String, std::io::Error>,
    ) -> futures::future::Ready<
        Option<std::result::Result<Option<String>, std::io::Error>>,
    > + 'static),
>;
type FnFilterNone = Box<
    (dyn Fn(
        Option<String>,
    ) -> futures::future::Ready<std::result::Result<Option<String>, std::io::Error>>
         + 'static),
>;

type ChunkByLineStream<R> = TryFilterMap<
    Scan<Lines<R>, String, Ready<Option<Result<Option<String>, std::io::Error>>>, FnScanner>,
    Ready<Result<Option<String>, std::io::Error>>,
    FnFilterNone,
>;

impl<R: BufRead>
    ChunkByLine<
        TryFilterMap<
            futures::stream::Scan<
                async_std::io::Lines<R>,
                String,
                futures::future::Ready<Option<std::result::Result<Option<String>, std::io::Error>>>,
                Box<
                    (dyn for<'r> Fn(
                        &'r mut String,
                        std::result::Result<String, std::io::Error>,
                    ) -> futures::future::Ready<
                        Option<std::result::Result<Option<String>, std::io::Error>>,
                    > + 'static),
                >,
            >,
            futures::future::Ready<std::result::Result<Option<String>, std::io::Error>>,
            Box<
                (dyn Fn(
                    Option<String>,
                ) -> futures::future::Ready<
                    std::result::Result<Option<String>, std::io::Error>,
                > + 'static),
            >,
        >,
    >
{
    pub(crate) fn new(lines: Lines<R>, delimiter: &'static str) -> Self {
        let delimiter_cp = delimiter.clone();
        let f1: Box<
            dyn for<'r> Fn(
                &'r mut String,
                std::result::Result<String, std::io::Error>,
            ) -> futures::future::Ready<
                Option<std::result::Result<Option<String>, std::io::Error>>,
            >,
        > = Box::new(
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
        let f2: Box<
            dyn Fn(
                Option<String>,
            )
                -> futures::future::Ready<std::result::Result<Option<String>, std::io::Error>>,
        > = Box::new(|x| future::ready(Ok(x)));
        let stream = lines.scan(String::new(), f1).try_filter_map(f2);
        Self { stream }
    }
    // delegate_access_inner!(stream, St, ());
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

// pub trait ChunkByLineExt<R: BufRead> {
//     fn chunk_by_line(self, delimiter: &str) -> ChunkByLine<ChunkByLineStream<R>>;
// }

// impl<R: BufRead> ChunkByLineExt<R> for Lines<R> {
//     fn chunk_by_line(self, delimiter: &str) -> ChunkByLine<ChunkByLineStream<R>> {
//         ChunkByLine::new(self, delimiter)
//     }
// }

#[async_std::main]
async fn main() -> Result<()> {
    println!("Hello World");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BYTES: &[u8; 39] = b"~~~
multi
line
chunk
~~~
another
chunk
";

    const BYTES2: &[u8; 42] = b"~~~
mutli

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
        // let docs: Vec<String> = Cursor::new(BYTES2)
        //     .lines()
        //     .chunk_by_line("~~~")
        //     .try_collect()
        //     .await?;
        // assert_eq!(
        //     docs,
        //     vec!["~~~", "mutli", "line", "chunk", "~~~", "another", "chunk"]
        // );
        Ok(())
    }
}
