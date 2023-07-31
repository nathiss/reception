use tokio::sync::watch::{Receiver, Sender};

use super::{
    connection_status::ConnectionStatus, error::SharedStateError, handle_status::HandleStatus,
};

#[derive(Debug)]
pub(crate) struct ConnectionHalf {
    handle_notifier: Sender<ConnectionStatus>,
    connection_receiver: Receiver<HandleStatus>,
}

impl ConnectionHalf {
    pub(super) fn new(
        handle_notifier: Sender<ConnectionStatus>,
        connection_receiver: Receiver<HandleStatus>,
    ) -> Self {
        Self {
            handle_notifier,
            connection_receiver,
        }
    }

    pub(crate) fn notify(&self, status: ConnectionStatus) -> Result<(), SharedStateError> {
        self.handle_notifier.send(status).map_err(|_| {
            log::error!(
                "{}:{} Tried to send a new `ConnectionStatus` over the channel and failed. Status: {}.",
                file!(),
                line!(),
                status
            );
            SharedStateError::ConnectionFailedToShareUpdate(status)
        })?;

        Ok(())
    }

    pub(crate) fn receiver_mut(&mut self) -> &mut Receiver<HandleStatus> {
        &mut self.connection_receiver
    }
}
