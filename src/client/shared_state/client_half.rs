use tokio::sync::watch::Sender;

use super::{client_status::ClientStatus, error::SharedStateError};

#[derive(Debug)]
pub(crate) struct ClientHalf {
    handle_notifier: Sender<ClientStatus>,
}

impl ClientHalf {
    pub(super) fn new(handle_notifier: Sender<ClientStatus>) -> Self {
        Self { handle_notifier }
    }

    pub(crate) fn notify(&self, status: ClientStatus) -> Result<(), SharedStateError> {
        self.handle_notifier.send(status).map_err(|_| {
            log::error!(
                "{}:{} Tried to send a new `ClientStatus` over the channel and failed. Status: {}.",
                file!(),
                line!(),
                status
            );
            SharedStateError::new(status)
        })?;

        Ok(())
    }
}
