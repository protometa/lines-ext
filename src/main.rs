use anyhow::Result;
use async_std::io;
use async_std::io::prelude::*;
use async_std::io::{stdin, BufReader, Cursor};
use futures::future::{self, Future};
use futures::stream::{self, Stream, StreamExt, TryStream, TryStreamExt};
// use clap::{App, Arg};

#[async_std::main]
async fn main() -> Result<()> {
    println!("Hello World");
    let cursor = Cursor::new(
        b"
            ---
            a: 1
            ---
            a: 2
        ",
    );
    let lines = cursor.lines();

    let lines: Vec<String> = lines
        .scan(
            String::new(),
            |state, line| -> futures::future::Ready<Option<Result<String, std::io::Error>>> {
                future::ready(Some(Ok(state.clone())))
            },
        )
        .try_collect()
        .await?;

    dbg!(lines);

    // while let Some(line) = lines.next().await {
    //     dbg!(line?);
    // }

    Ok(())
}
