use thiserror::Error;

/// Errors that can be raised by [`Client`](`crate::client::Client`).
#[derive(Error, Debug)]
pub enum ClientError {
    /// Server has failed to deserialize client's model.
    ///
    /// It is assumed that the peer's connection has been broken. The server gracefully terminates the connection to the
    /// peer. After receiving this error, any sends to the client will fail.
    #[error("failed to deserialize a message received from peer")]
    ModelDeserialization,

    /// Server has failed to send a serialized model to the client.
    ///
    /// It _probably_ means that the peer has dropped its half of the connection. It's not possible to revive a closed
    /// connection.
    #[error("failed to send serialized model to the client")]
    SendModel,

    /// Client's connection has already been dropped.
    ///
    /// Either the peer has sent an ill-formed model or
    /// [`ClientHandle::close()`](`crate::client::ClientHandle::close`) has been called.
    #[error("connection to client has been dropped")]
    DroppedConnection,
}
