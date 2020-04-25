use actix_http::ws::handshake;
use actix_web::{web, HttpRequest, HttpResponse};
use tokio::sync::mpsc::channel;

pub use actix_http::ws::Message;

mod fut;
mod session;

pub use self::{
    fut::{MessageStream, StreamingBody},
    session::{Closed, Session},
};

pub fn handle(
    req: &HttpRequest,
    body: web::Payload,
) -> Result<(HttpResponse, Session, MessageStream), actix_web::Error> {
    let mut response = handshake(req.head())?;
    let (tx, rx) = channel(32);

    Ok((
        response.streaming(StreamingBody::new(rx)),
        Session::new(tx),
        MessageStream::new(body.into_inner()),
    ))
}
