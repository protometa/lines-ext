# lines-ext

This module contains an extension trait [`LinesExt`](self::LinesExt) for
`Stream<Item = Result<String>>` such as those returned by [`AsyncBufReadExt::lines`](futures::io::AsyncBufReadExt::lines).
The trait provides the [`chunk_by_line`](self::LinesExt::chunk_by_line) method which groups
lines into chunks given a delimiter line that separates chunks.

```rust
use async_std::io::Cursor;
use futures::AsyncBufReadExt;
use futures::stream::TryStreamExt;
use lines_ext::LinesExt;

let bytes = b"~~~
multi
line
chunk
~~~
another
chunk
";

let chunks_stream = Cursor::new(bytes).lines().chunk_by_line("~~~");

let chunks_vec: Vec<String> = chunks_stream.try_collect().await?;
assert_eq!(chunks_vec, vec!["multi\nline\nchunk\n", "another\nchunk\n"]);
```
