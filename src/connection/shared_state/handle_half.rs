use tokio::sync::watch::{error::SendError, Receiver, Sender};

use crate::connection::ConnectionConfig;

use super::{
    connection_status::ConnectionStatus, error::SharedStateError, handle_status::HandleStatus,
};

#[derive(Debug)]
pub(crate) struct HandleHalf {
    config: ConnectionConfig,
    connection_notifier: Sender<HandleStatus>,
    handle_receiver: Receiver<ConnectionStatus>,
}

impl HandleHalf {
    pub(super) fn new(
        config: ConnectionConfig,
        connection_notifier: Sender<HandleStatus>,
        handle_receiver: Receiver<ConnectionStatus>,
    ) -> Self {
        Self {
            config,
            connection_notifier,
            handle_receiver,
        }
    }

    pub(crate) fn notify(&self, status: HandleStatus) -> Result<(), SharedStateError> {
        self.connection_notifier
            .send(status)
            .map_err(|SendError(status)| SharedStateError::HandleFailedToShareUpdate(status))?;

        Ok(())
    }

    pub(crate) fn receiver(&self) -> Receiver<ConnectionStatus> {
        self.handle_receiver.clone()
    }

    pub(crate) fn config(&self) -> &ConnectionConfig {
        &self.config
    }
}
