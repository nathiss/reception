mod client_half;
mod client_status;
mod error;
mod handle_half;

use tokio::sync::watch::channel;

pub(super) use self::{
    client_half::ClientHalf, client_status::ClientStatus, handle_half::HandleHalf,
};

#[derive(Debug)]
pub(super) struct SharedState {
    handle_half: HandleHalf,
    client_half: ClientHalf,
}

impl SharedState {
    pub(super) fn new() -> Self {
        let (handle_notifier, handle_receiver) = channel(Default::default());

        let handle_half = HandleHalf::new(handle_receiver);
        let client_half = ClientHalf::new(handle_notifier);

        Self {
            handle_half,
            client_half,
        }
    }

    pub(super) fn split(self) -> (ClientHalf, HandleHalf) {
        (self.client_half, self.handle_half)
    }
}
