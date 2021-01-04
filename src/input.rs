use std::iter::empty;
use std::{future, io, pin::Pin};

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

/// Extension to `Read` trait.
pub trait TermReadAsync: Sized {
    type EventsStream: Stream<Item = Result<Event, io::Error>>;
    type KeysStream: Stream<Item = Result<Key, io::Error>>;

    /// An iterator over input events.
    fn events_stream(self) -> Self::EventsStream;

    /// An iterator over key inputs.
    fn keys_stream(self) -> Self::KeysStream;
}

impl<R: 'static + Send + AsyncRead + TermReadAsyncEventsAndRaw> TermReadAsync for R {
    type EventsStream = Pin<Box<dyn Stream<Item = Result<Event, io::Error>> + Send>>;
    type KeysStream = Pin<Box<dyn Stream<Item = Result<Key, io::Error>> + Send>>;

    fn events_stream(self) -> Self::EventsStream {
        Box::pin(
            self.events_and_raw_stream()
                .map(|event_and_raw| event_and_raw.map(|(event, _raw)| event)),
        )
    }

    fn keys_stream(self) -> Self::KeysStream {
        Box::pin(self.events_stream().filter_map(|event| {
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
