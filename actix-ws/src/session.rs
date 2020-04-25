use actix_http::ws::{CloseReason, Message};
use bytes::Bytes;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub struct Session {
    inner: Option<Sender<Message>>,
    closed: Arc<AtomicBool>,
}

#[derive(Debug, thiserror::Error)]
#[error("Session is closed")]
pub struct Closed;

impl Session {
    pub(super) fn new(inner: Sender<Message>) -> Self {
        Session {
            inner: Some(inner),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    fn pre_check(&mut self) {
        if self.closed.load(Ordering::Relaxed) {
            self.inner.take();
        }
    }

    pub async fn text<T>(&mut self, msg: T) -> Result<(), Closed>
    where
        T: Into<String>,
    {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            inner
                .send(Message::Text(msg.into()))
                .await
                .map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    pub async fn binary<T>(&mut self, msg: T) -> Result<(), Closed>
    where
        T: Into<Bytes>,
    {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            inner
                .send(Message::Binary(msg.into()))
                .await
                .map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    pub async fn ping(&mut self, msg: &[u8]) -> Result<(), Closed> {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            inner
                .send(Message::Ping(Bytes::copy_from_slice(msg)))
                .await
                .map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    pub async fn pong(&mut self, msg: &[u8]) -> Result<(), Closed> {
        self.pre_check();
        if let Some(inner) = self.inner.as_mut() {
            inner
                .send(Message::Pong(Bytes::copy_from_slice(msg)))
                .await
                .map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }

    pub async fn close(&mut self, reason: Option<CloseReason>) -> Result<(), Closed> {
        self.pre_check();
        if let Some(mut inner) = self.inner.take() {
            self.closed.store(true, Ordering::Relaxed);
            inner.send(Message::Close(reason)).await.map_err(|_| Closed)
        } else {
            Err(Closed)
        }
    }
}
