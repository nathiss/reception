#![warn(missing_docs)]

//! This crate provides a way of binding a TCP listener that will accept incoming
//! [WebSocket](https://en.wikipedia.org/wiki/WebSocket) connections. Additionally it provides an abstraction layer (see
//! [`Client`](crate::client::Client)) for serializing and deserializing well-defined models.

/// Module containing types associated with client-level abstraction (sending and receiving models).
pub mod client;

/// Module containing types associated with connection-level abstraction (sending raw bytes and handling protocol
/// details).
pub mod connection;
mod listener;
mod sender_handle;

pub use listener::config::ListenerConfig;
pub use listener::error::ListenerError;
pub use listener::Listener;
pub use sender_handle::SenderHandle;
pub use tokio_util::sync::CancellationToken;
