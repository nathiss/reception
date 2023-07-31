use thiserror::Error;

use super::{connection_status::ConnectionStatus, handle_status::HandleStatus};

#[derive(Error, Debug)]
pub(crate) enum SharedStateError {
    #[error("connection failed to share a new status update with the handle. Status: '{}'", .0)]
    ConnectionFailedToShareUpdate(ConnectionStatus),

    #[error("handle failed to share a new status update with the connection. Status: '{}'", .0)]
    HandleFailedToShareUpdate(HandleStatus),
}
