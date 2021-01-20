# termion-input-tokio

An adapter that exposes termion's input and key event iterators as asynchronous streams.

## Compatiblity

Compatible with Tokio v1.0.

## Usage

```rust
use futures::StreamExt;
use std::future;
use termion::{event::Key, raw::IntoRawMode};
use termion_input_tokio::TermReadAsync;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    // Disable line buffering, local echo, etc.
    let _raw_term = std::io::stdout().into_raw_mode()?;

    tokio::io::stdin()
        .keys_stream()
        // End the stream when 'q' is pressed.
        .take_while(|event| {
            future::ready(match event {
                Ok(Key::Char('q')) => false,
                _ => true,
            })
        })
        // Print each key that was pressed.
        .for_each(|event| async move {
            println!("{:?}\r", event);
        })
        .await;

    Ok(())
}
```

## Non-blocking Input

It is challenging to use true non-blocking reads with `stdin`. In the common case both `stdin` and `stdout` refer to the same file, typically a PTY. Since non-blocking mode is a per-file property, rather than a per-file-descriptor one, using `fcntl` with `O_NONBLOCK` to change `stdin` into non-blocking mode will also make `stdout` non-blocking. Since most code is not prepared to deal with `EWOULDBLOCK` when writing to `stdout`, asynchronous reads from `stdin` are typically typically performed using blocking operations on a secondary thread. This is how `AsyncRead` for [tokio::io::stdin()](https://docs.rs/tokio/latest/tokio/io/fn.stdin.html) is implemented.

## Credits

This is based on [termion-tokio](https://github.com/katyo/termion-tokio) by [Kayo Phoenix](https://github.com/katyo), which is in turn based on code within [termion](https://github.com/redox-os/termion).
