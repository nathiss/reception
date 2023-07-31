use std::net::{AddrParseError, SocketAddr};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ListenerError {
    #[error("invalid listener socket configuration")]
    SocketConfiguration(#[source] AddrParseError),

    #[error("failed to bind listener to {addr}")]
    BindingError {
        addr: SocketAddr,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to accept a new connection")]
    AcceptConnection(#[source] std::io::Error),

    #[error("client socket could not become readable")]
    Readable(#[source] std::io::Error),

    #[error("client socket could not become writable")]
    Writable(#[source] std::io::Error),

    #[error("failed to complete websocket handshake")]
    WebSocketHandshakeResolution(#[source] tokio_tungstenite::tungstenite::Error),
}
