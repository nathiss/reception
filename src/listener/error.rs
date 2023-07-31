use std::net::{AddrParseError, SocketAddr};

use thiserror::Error;

/// Errors that can be raised by [`Listener`](`crate::Listener`).
#[derive(Error, Debug)]
pub enum ListenerError {
    /// Listener's configuration is invalid.
    #[error("invalid listener socket configuration")]
    SocketConfiguration(#[source] AddrParseError),

    /// Failed to bind listener to the configured socket.
    #[error("failed to bind listener to {addr}")]
    BindingError {
        /// Socket address it tried to bind to.
        addr: SocketAddr,

        /// Original error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to accept a new connection.
    ///
    /// The listener is considered to be broken. It stops listening on the configured socket and unbinds itself.
    #[error("failed to accept a new connection")]
    AcceptConnection(#[source] std::io::Error),
}
