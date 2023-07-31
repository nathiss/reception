use tokio::sync::watch::Receiver;

use super::client_status::ClientStatus;

#[derive(Debug)]
pub(crate) struct HandleHalf {
    handle_receiver: Receiver<ClientStatus>,
}

impl HandleHalf {
    pub(super) fn new(handle_receiver: Receiver<ClientStatus>) -> Self {
        Self { handle_receiver }
    }

    pub(crate) fn receiver_mut(&mut self) -> &mut Receiver<ClientStatus> {
        &mut self.handle_receiver
    }
}
