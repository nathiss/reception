use std::{
    marker::PhantomData,
    sync::{Arc, Weak},
};

use cancellable::{Cancellable, CancellableHandle};
use tokio::sync::Mutex;

use crate::SenderHandle;

use super::{
    shared_state::{ClientStatus, HandleHalf},
    ClientError,
};

/// Sending half of the peer's connection.
#[derive(Debug)]
pub struct ClientHandle<C, SM>
where
    C: Cancellable,
{
    inner: Arc<Mutex<Option<CancellableHandle<C>>>>,
    _sm_marker: PhantomData<SM>,
}

impl<C, SM> ClientHandle<C, SM>
where
    C: Cancellable + 'static,
    <C as Cancellable>::Handle: Send,
{
    pub(super) fn new(inner: CancellableHandle<C>, handle_half: HandleHalf) -> Self {
        let inner = Arc::new(Mutex::const_new(Some(inner)));
        let weak_inner = Arc::downgrade(&inner);
        Self::spawn_handle_task(weak_inner, handle_half);

        Self {
            inner,
            _sm_marker: PhantomData,
        }
    }

    /// Closes both sending and receiving halves of the peer's connection.
    ///
    /// This method _should_ only be called once. Consecutive calls to of this method have no effect.
    pub async fn close(&self) {
        drop(self.inner.lock().await.take());
    }

    fn spawn_handle_task(
        inner: Weak<Mutex<Option<CancellableHandle<C>>>>,
        mut handle_half: HandleHalf,
    ) {
        tokio::spawn(async move {
            let receiver = handle_half.receiver_mut();
            loop {
                match receiver.changed().await {
                    Ok(()) => {
                        let status = *receiver.borrow_and_update();
                        match status {
                            ClientStatus::Normal => {}
                            ClientStatus::InvalidModel => {
                                if let Some(inner) = inner.upgrade() {
                                    drop(inner.lock_owned().await.take());
                                }
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::trace!(
                            "{}:{} handle_receiver channel has closed. Error: {}.",
                            file!(),
                            line!(),
                            e
                        );
                        break;
                    }
                }
            }
        });
    }
}

#[async_trait::async_trait]
impl<C, SM> SenderHandle for ClientHandle<C, SM>
where
    C: Cancellable,
    <C as Cancellable>::Handle: SenderHandle<Item = Vec<u8>> + Send + Sync,
    SM: Into<Vec<u8>> + Send,
{
    type Item = SM;
    type Error = ClientError;

    async fn send(&mut self, item: Self::Item) -> Result<(), Self::Error> {
        match self.inner.lock().await.as_mut() {
            Some(inner) => {
                inner
                    .send(item.into())
                    .await
                    .map_err(|_| ClientError::SendModel)?;
                Ok(())
            }
            None => {
                log::error!(
                    "Failed to send a message to the client. The connection has been dropped."
                );
                Err(ClientError::DroppedConnection)
            }
        }
    }
}
