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

Reading from [tokio::io::stdin()](https://docs.rs/tokio/latest/tokio/io/fn.stdin.html) is still blocking operation. This can be problematic if you wish to gracefully tear down the input event stream as there's no way to interrupt an in-progress `read` system call. To address this you can use [tokio-fd](https://github.com/nanpuyue/tokio-fd)'s `AsyncFd` to wrap `stdin`. This will result in tokio polling `stdin` for readability, as it would a socket. It will then only attempt to read from `stdin` when the kernel indicates that data is available, avoiding blocking `read` calls and allowing you to cleanly tear down your input event stream.

## Credits

This is based on [termion-tokio](https://github.com/katyo/termion-tokio) by [Kayo Phoenix](https://github.com/katyo), which is in turn based on code within [termion](https://github.com/redox-os/termion).
