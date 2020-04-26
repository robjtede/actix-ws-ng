use actix_http::{
    ws::{Codec, Frame, Message, ProtocolError},
    Payload,
};
use actix_web::Error;
use bytes::{Bytes, BytesMut};
use futures::stream::{Stream, StreamExt};
use std::{
    collections::VecDeque,
    io,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::mpsc::Receiver;
use tokio_util::codec::{Decoder, Encoder};

/// A response body for Websocket HTTP Requests
#[pin_project::pin_project]
pub struct StreamingBody {
    #[pin]
    session_rx: Receiver<Message>,

    messages: VecDeque<Message>,
    buf: BytesMut,
    codec: Codec,
    closing: bool,
}

/// A stream of Messages from a websocket client
///
/// Messages can be accessed via the stream's `.next()` method
#[pin_project::pin_project]
pub struct MessageStream {
    #[pin]
    payload: Payload,

    messages: VecDeque<Message>,
    buf: BytesMut,
    codec: Codec,
    closing: bool,
}

impl StreamingBody {
    pub(super) fn new(session_rx: Receiver<Message>) -> Self {
        StreamingBody {
            session_rx,
            messages: VecDeque::new(),
            buf: BytesMut::new(),
            codec: Codec::new(),
            closing: false,
        }
    }
}

impl MessageStream {
    pub(super) fn new(payload: Payload) -> Self {
        MessageStream {
            payload,
            messages: VecDeque::new(),
            buf: BytesMut::new(),
            codec: Codec::new(),
            closing: false,
        }
    }

    /// Wait for the next item from the message stream
    ///
    /// ```rust,ignore
    /// while let Some(Ok(msg)) = stream.next().await {
    ///     // handle message
    /// }
    /// ```
    #[allow(clippy::should_implement_trait)]
    pub async fn next(&'_ mut self) -> Option<Result<Message, ProtocolError>> {
        StreamExt::next(self).await
    }
}

impl Stream for StreamingBody {
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if *this.closing {
            return Poll::Ready(None);
        }

        loop {
            match Pin::new(&mut this.session_rx).poll_next(cx) {
                Poll::Ready(Some(msg)) => {
                    this.messages.push_back(msg);
                }
                Poll::Ready(None) => {
                    *this.closing = true;
                    break;
                }
                Poll::Pending => break,
            }
        }

        while let Some(msg) = this.messages.pop_front() {
            if let Err(e) = this.codec.encode(msg, &mut this.buf) {
                return Poll::Ready(Some(Err(e.into())));
            }
        }

        if !this.buf.is_empty() {
            return Poll::Ready(Some(Ok(this.buf.split().freeze())));
        }

        Poll::Pending
    }
}

impl Stream for MessageStream {
    type Item = Result<Message, ProtocolError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // Return the first message in the queue if one exists
        //
        // This is faster than polling and parsing
        if let Some(msg) = this.messages.pop_front() {
            return Poll::Ready(Some(Ok(msg)));
        }

        if !*this.closing {
            // Read in bytes until there's nothing left to read
            loop {
                match Pin::new(&mut this.payload).poll_next(cx) {
                    Poll::Ready(Some(Ok(bytes))) => {
                        this.buf.extend_from_slice(&bytes);
                    }
                    Poll::Ready(Some(Err(e))) => {
                        return Poll::Ready(Some(Err(ProtocolError::Io(io::Error::new(
                            io::ErrorKind::Other,
                            e.to_string(),
                        )))));
                    }
                    Poll::Ready(None) => {
                        *this.closing = true;
                        break;
                    }
                    Poll::Pending => break,
                }
            }
        }

        // Create messages until there's no more bytes left
        while let Some(frame) = this.codec.decode(&mut this.buf)? {
            let message = match frame {
                Frame::Text(bytes) => {
                    let s = std::str::from_utf8(&bytes)
                        .map_err(|e| {
                            ProtocolError::Io(io::Error::new(io::ErrorKind::Other, e.to_string()))
                        })?
                        .to_string();
                    Message::Text(s)
                }
                Frame::Binary(bytes) => Message::Binary(bytes),
                Frame::Ping(bytes) => Message::Ping(bytes),
                Frame::Pong(bytes) => Message::Pong(bytes),
                Frame::Close(reason) => Message::Close(reason),
                Frame::Continuation(item) => Message::Continuation(item),
            };

            this.messages.push_back(message);
        }

        // Return the first message in the queue
        if let Some(msg) = this.messages.pop_front() {
            return Poll::Ready(Some(Ok(msg)));
        }

        // If we've exhausted our message queue and we're closing, close the stream
        if *this.closing {
            return Poll::Ready(None);
        }

        Poll::Pending
    }
}
