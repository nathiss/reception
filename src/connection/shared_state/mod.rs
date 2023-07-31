mod connection_half;
mod connection_status;
mod error;
mod handle_half;
mod handle_status;

use tokio::sync::watch::channel;

pub(super) use self::{
    connection_half::ConnectionHalf, connection_status::ConnectionStatus, handle_half::HandleHalf,
    handle_status::HandleStatus,
};

use super::ConnectionConfig;

#[derive(Debug)]
pub(super) struct SharedState {
    handle_half: HandleHalf,
    connection_half: ConnectionHalf,
}

impl SharedState {
    pub(super) fn new(config: ConnectionConfig) -> Self {
        let (connection_notifier, connection_receiver) = channel(Default::default());
        let (handle_notifier, handle_receiver) = channel(Default::default());

        let handle_half = HandleHalf::new(config.clone(), connection_notifier, handle_receiver);
        let connection_half = ConnectionHalf::new(handle_notifier, connection_receiver);

        Self {
            handle_half,
            connection_half,
        }
    }

    pub(super) fn split(self) -> (ConnectionHalf, HandleHalf) {
        (self.connection_half, self.handle_half)
    }
}
