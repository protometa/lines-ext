use anyhow::Result;
use async_std::io;
use async_std::io::prelude::*;
use async_std::io::{stdin, BufReader, Cursor};
use futures::future::{self, Future, Ready};
use futures::stream::{self, Stream, StreamExt, TryStream, TryStreamExt};
// use clap::{App, Arg};

#[async_std::main]
async fn main() -> Result<()> {
    println!("Hello World");
    let cursor = Cursor::new(
        b"
---
a: 1
b: 2
---
a: 2
b: 3
        ",
    );
    let lines = cursor.lines();

    let lines: Vec<String> = lines
        // Stream of Result<String>
        .scan(
            String::new(),
            |state, line| -> Ready<Option<Result<Option<String>, std::io::Error>>> {
                future::ready(
                    line.map(|line| {
                        Some(if line == "---" {
                            Some(state.trim().to_owned())
                        } else {
                            state.push('\n');
                            state.push_str(&line);
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
        .try_collect()
        .await?;

    dbg!(lines);

    // while let Some(line) = lines.next().await {
    //     dbg!(line?);
    // }

    Ok(())
}
