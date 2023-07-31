use thiserror::Error;

use tokio_tungstenite::tungstenite::Message;

#[derive(Error, Debug)]
pub enum ConnectionError<E> {
    #[error("received an error from websocket library")]
    WebSocket(#[source] tokio_tungstenite::tungstenite::Error),

    #[error("received an illegal message type. Message length: {}", .0.len())]
    IllegalMessageType(Message),

    #[error("failed to queue up a data package from server to peer")]
    QueueServerMessage(#[source] E),
}
