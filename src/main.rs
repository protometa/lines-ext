use anyhow::Result;
use async_std::io;
use async_std::io::prelude::*;
use async_std::io::{stdin, BufRead, BufReader, Cursor, Lines};
use futures::future::{self, Future, Ready};
use futures::stream::{self, Stream, StreamExt, TryStream, TryStreamExt};
// use clap::{App, Arg};

pub fn chunk_by_line<R: BufRead>(
    lines: Lines<R>,
    delimiter: &str,
) -> impl Stream<Item = Result<String, std::io::Error>> {
    let delimiter1 = delimiter.to_owned();
    let delimiter2 = delimiter.to_owned();
    lines
        .chain(stream::once(async move { Ok(delimiter1) }))
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
