use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("failed to deserialize a message received from peer")]
    ModelDeserialization,

    #[error("failed to send serialized model to the client")]
    SendModel,

    #[error("connection to client has been dropped")]
    DroppedConnection,
}
