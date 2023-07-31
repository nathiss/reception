use std::{error::Error, fmt::Display};

use super::client_status::ClientStatus;

#[derive(Debug)]
pub(crate) struct SharedStateError {
    status: ClientStatus,
}

impl SharedStateError {
    pub(super) fn new(status: ClientStatus) -> Self {
        Self { status }
    }
}

impl Error for SharedStateError {}

impl Display for SharedStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "connection failed to share a new status update with the handle. Status: '{}'",
            self.status
        )
    }
}
