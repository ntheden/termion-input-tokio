use std::{future, io, pin::Pin};
use std::{
    iter::empty,
    task::{Context, Poll},
};

use bytes::{Buf, Bytes, BytesMut};
use futures::{Stream, StreamExt};
use termion::event::{self, Event, Key};
use tokio::io::AsyncRead;
use tokio_util::codec::{Decoder, FramedRead};

/// An iterator over input events and the bytes that define them
type EventsAndRawStream<R> = FramedRead<R, EventsAndRawDecoder>;

pub struct EventsAndRawDecoder;

impl Decoder for EventsAndRawDecoder {
    type Item = (Event, Vec<u8>);
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match src.len() {
            0 => Ok(None),
            1 => match src[0] {
                b'\x1B' => {
                    src.advance(1);
                    Ok(Some((Event::Key(Key::Esc), vec![b'\x1B'])))
                }
                c => {
                    if let Ok(res) = parse_event(c, &mut empty()) {
                        src.advance(1);
                        Ok(Some(res))
                    } else {
                        Ok(None)
                    }
                }
            },
            _ => {
                let (off, res) = if let Some((c, cs)) = src.split_first() {
                    let cur = Bytes::copy_from_slice(cs);
                    let mut it = cur.into_iter().map(Ok);
                    if let Ok(res) = parse_event(*c, &mut it) {
                        (1 + cs.len() - it.len(), Ok(Some(res)))
                    } else {
                        (0, Ok(None))
                    }
                } else {
                    (0, Ok(None))
                };

                src.advance(off);
                res
            }
        }
    }
}

fn parse_event<I>(item: u8, iter: &mut I) -> Result<(Event, Vec<u8>), io::Error>
where
    I: Iterator<Item = Result<u8, io::Error>>,
{
    let mut buf = vec![item];
    let result = {
        let mut iter = iter.inspect(|byte| {
            if let &Ok(byte) = byte {
                buf.push(byte);
            }
        });
        event::parse_event(item, &mut iter)
    };
    result
        .or(Ok(Event::Unsupported(buf.clone())))
        .map(|e| (e, buf))
}

// A type-erased stream of input objects.
pub struct InputStream<T>(Pin<Box<dyn Stream<Item = Result<T, io::Error>> + Send>>);

impl<T> InputStream<T> {
    fn new<S: Stream<Item = Result<T, io::Error>> + Send + 'static>(stream: S) -> Self {
        InputStream(Box::pin(stream))
    }
}

impl<T> Stream for InputStream<T> {
    type Item = Result<T, io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_next_unpin(cx)
    }
}

/// Extension to `Read` trait.
pub trait TermReadAsync: Sized {
    /// An iterator over input events.
    fn events_stream(self) -> InputStream<Event>;

    /// An iterator over key inputs.
    fn keys_stream(self) -> InputStream<Key>;
}

impl<R: 'static + Send + AsyncRead + TermReadAsyncEventsAndRaw> TermReadAsync for R {
    fn events_stream(self) -> InputStream<Event> {
        InputStream::new(
            self.events_and_raw_stream()
                .map(|event_and_raw| match event_and_raw {
                    Ok((event, _raw)) => Ok(event),
                    Err(e) => Err(e),
                }),
        )
    }

    fn keys_stream(self) -> InputStream<Key> {
        InputStream::new(self.events_stream().filter_map(|event| {
            future::ready(match event {
                Ok(Event::Key(k)) => Some(Ok(k)),
                Ok(_) => None,
                Err(e) => Some(Err(e)),
            })
        }))
    }
}

/// Extension to `TermReadAsync` trait. A separate trait in order to maintain backwards compatibility.
pub trait TermReadAsyncEventsAndRaw {
    /// An iterator over input events and the bytes that define them.
    fn events_and_raw_stream(self) -> EventsAndRawStream<Self>
    where
        Self: Sized;
}

impl<R: AsyncRead> TermReadAsyncEventsAndRaw for R {
    fn events_and_raw_stream(self) -> EventsAndRawStream<Self> {
        FramedRead::new(self, EventsAndRawDecoder)
    }
}
