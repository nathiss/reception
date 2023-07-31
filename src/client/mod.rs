mod client_handle;
mod error;
mod shared_state;

use std::{fmt::Display, marker::PhantomData};

use cancellable::{Cancellable, CancellableHandle, CancellationResult, CancellationToken};
use tokio::sync::mpsc::{error::SendError, unbounded_channel, UnboundedReceiver};

use crate::SenderHandle;

pub use self::error::ClientError;
use self::{
    client_handle::ClientHandle,
    shared_state::{ClientHalf, ClientStatus, HandleHalf, SharedState},
};

#[derive(Debug)]
pub struct Client<C, CM, SM>
where
    C: Cancellable,
{
    incoming_receiver: UnboundedReceiver<<C as Cancellable>::Result>,
    connection_handle: Option<CancellableHandle<C>>,
    client_half: ClientHalf,
    handle_half: Option<HandleHalf>,

    _cm_marker: PhantomData<CM>,
    _sm_marker: PhantomData<SM>,
}

impl<C, CM, SM> Client<C, CM, SM>
where
    C: Cancellable + Send + 'static,
    <C as Cancellable>::Result: Send,
    <C as Cancellable>::Handle: SenderHandle,
{
    pub(crate) async fn new(connection: C, cancellation_token: CancellationToken) -> Self {
        let (incoming_sender, incoming_receiver) = unbounded_channel();

        let connection_handle = connection
            .spawn_with_callback(
                cancellation_token.clone(),
                move |data| match incoming_sender.send(data) {
                    Ok(()) => Ok(()),
                    Err(SendError(data)) => Err(data),
                },
            )
            .await;

        let (client_half, handle_half) = SharedState::new().split();

        Self {
            incoming_receiver,
            connection_handle: Some(connection_handle),
            client_half,
            handle_half: Some(handle_half),

            _cm_marker: PhantomData,
            _sm_marker: PhantomData,
        }
    }

    fn notify_handle(&self, status: ClientStatus) {
        if let Err(e) = self.client_half.notify(status) {
            log::error!("Error: {}", e);
        }
    }
}

#[cancellable::async_trait]
impl<C, CM, SM> Cancellable for Client<C, CM, SM>
where
    C: std::fmt::Debug + Send + Cancellable + 'static,
    <C as Cancellable>::Result: Send,
    <C as Cancellable>::Handle: Send + SenderHandle,
    Vec<u8>: From<<C as Cancellable>::Result>,
    CM: TryFrom<Vec<u8>> + Send,
    <CM as TryFrom<Vec<u8>>>::Error: Display,
    SM: std::fmt::Debug + Send,
{
    type Result = CM;
    type Handle = ClientHandle<C, SM>;
    type Error = ClientError;

    async fn new_handle(&mut self) -> Self::Handle {
        let handle_half = self.handle_half.take().expect("HandleHalf to be present.");

        self.connection_handle
            .take()
            .map(|handle| ClientHandle::new(handle, handle_half))
            .expect("SendingServiceHandle to be present.")
    }

    async fn run(&mut self) -> Result<CancellationResult<Self::Result>, Self::Error> {
        match self.incoming_receiver.recv().await {
            Some(msg) => match CM::try_from(msg.into()) {
                Ok(model) => Ok(CancellationResult::Item(model)),
                Err(e) => {
                    log::warn!("Received ill-formed message from client: '{e}'. Terminating the connection...");
                    self.notify_handle(ClientStatus::InvalidModel);
                    Err(ClientError::ModelDeserialization)
                }
            },
            None => {
                log::trace!(
                    "{}:{} Connection channel has been closed.",
                    file!(),
                    line!()
                );
                Ok(CancellationResult::Break)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use cancellable::{Cancellable, CancellationResult, CancellationToken};
    use tokio::{
        sync::mpsc::{error::SendError, unbounded_channel, UnboundedReceiver, UnboundedSender},
        time::timeout,
    };

    use crate::{client::ClientError, SenderHandle};

    use super::Client;

    #[derive(Debug)]
    struct MockConnectionHandle {
        server_sender: UnboundedSender<Vec<u8>>,
    }

    impl MockConnectionHandle {
        fn new(server_sender: UnboundedSender<Vec<u8>>) -> Self {
            Self { server_sender }
        }
    }

    #[async_trait::async_trait]
    impl SenderHandle for MockConnectionHandle {
        type Item = Vec<u8>;
        type Error = Self::Item;

        async fn send(&mut self, item: Self::Item) -> Result<(), Self::Item> {
            match self.server_sender.send(item) {
                Ok(()) => Ok(()),
                Err(SendError(item)) => Err(item),
            }
        }
    }

    #[derive(Debug)]
    struct MockConnection {
        server_sender: Option<UnboundedSender<Vec<u8>>>,
        server_receiver: UnboundedReceiver<Vec<u8>>,
    }

    impl MockConnection {
        fn new(
            server_sender: UnboundedSender<Vec<u8>>,
            server_receiver: UnboundedReceiver<Vec<u8>>,
        ) -> Self {
            Self {
                server_sender: Some(server_sender),
                server_receiver,
            }
        }
    }

    #[cancellable::async_trait]
    impl Cancellable for MockConnection {
        type Result = Vec<u8>;
        type Handle = MockConnectionHandle;
        type Error = anyhow::Error;

        async fn new_handle(&mut self) -> Self::Handle {
            self.server_sender
                .take()
                .map(MockConnectionHandle::new)
                .expect("server_sender to be present.")
        }

        async fn run(&mut self) -> Result<CancellationResult<Self::Result>, Self::Error> {
            match self.server_receiver.recv().await {
                Some(msg) => Ok(CancellationResult::Item(msg)),
                None => Ok(CancellationResult::Break),
            }
        }
    }

    const INVALID_DATA: &[u8] = &[13u8, 37u8];

    #[derive(Debug)]
    struct MockModel {
        data: Vec<u8>,
    }

    impl MockModel {
        fn new(data: Vec<u8>) -> Self {
            Self { data }
        }
    }

    impl Into<Vec<u8>> for MockModel {
        fn into(self) -> Vec<u8> {
            self.data
        }
    }

    impl TryFrom<Vec<u8>> for MockModel {
        type Error = ClientError;

        fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
            if INVALID_DATA == value.as_slice() {
                return Err(ClientError::ModelDeserialization);
            }

            Ok(Self::new(value))
        }
    }

    async fn construct_connection_medium() -> (
        Client<MockConnection, MockModel, MockModel>,
        UnboundedSender<Vec<u8>>,
        UnboundedReceiver<Vec<u8>>,
    ) {
        let (server_sender, client_receiver) = unbounded_channel();
        let (client_sender, server_receiver) = unbounded_channel();

        let connection = MockConnection::new(server_sender, server_receiver);
        let client = Client::new(connection, CancellationToken::new()).await;

        (client, client_sender, client_receiver)
    }

    #[tokio::test]
    async fn should_serialize_server_model_and_send_it_to_peer() -> Result<(), anyhow::Error> {
        // Arrange
        let serialized_model = vec![42u8, 42u8];

        let model = MockModel::new(serialized_model.clone());

        let (client, _client_sender, mut client_receiver) = construct_connection_medium().await;
        let mut handle = client.spawn(CancellationToken::new()).await;

        // Act
        handle.send(model).await.unwrap();

        // Assert
        let received_model = client_receiver.recv().await.unwrap();
        assert_eq!(serialized_model, received_model);

        Ok(())
    }

    #[tokio::test]
    async fn should_deserialize_valid_model_from_peer_and_await_next_one(
    ) -> Result<(), anyhow::Error> {
        // Arrange
        let serialized_model = vec![42u8, 42u8];

        let (client, client_sender, _client_receiver) = construct_connection_medium().await;
        let handle = client.spawn(CancellationToken::new()).await;

        // Act
        client_sender.send(serialized_model.clone()).unwrap();

        // Assert
        let t = timeout(Duration::from_millis(100), handle).await;
        assert!(t.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn should_deserialize_valid_model_from_peer_and_callback() -> Result<(), anyhow::Error> {
        // Arrange
        let (incoming_sender, mut incoming_receiver) = unbounded_channel();

        let serialized_model = vec![42u8, 42u8];

        let (client, client_sender, _client_receiver) = construct_connection_medium().await;
        let _ = client
            .spawn_with_callback(CancellationToken::new(), move |model| match incoming_sender
                .send(model)
            {
                Ok(()) => Ok(()),
                Err(SendError(model)) => Err(model),
            })
            .await;

        // Act
        client_sender.send(serialized_model.clone()).unwrap();

        // Assert
        let received: Vec<u8> = incoming_receiver.recv().await.unwrap().into();
        assert_eq!(serialized_model, received);

        Ok(())
    }

    #[tokio::test]
    async fn should_complete_with_error_on_invalid_model() -> Result<(), anyhow::Error> {
        // Arrange
        let (client, client_sender, _client_receiver) = construct_connection_medium().await;
        let handle = client.spawn(CancellationToken::new()).await;

        // Act
        client_sender.send(Vec::from(INVALID_DATA)).unwrap();

        // Assert
        let Err(ClientError::ModelDeserialization) = handle.await? else {
            panic!("Expected `ModelsError::Deserialization` error.");
        };

        Ok(())
    }

    #[tokio::test]
    async fn should_not_callback_on_invalid_model() -> Result<(), anyhow::Error> {
        // Arrange
        let (incoming_sender, mut incoming_receiver) = unbounded_channel();

        let (client, client_sender, _client_receiver) = construct_connection_medium().await;
        let _ = client
            .spawn_with_callback(CancellationToken::new(), move |model| match incoming_sender
                .send(model)
            {
                Ok(()) => Ok(()),
                Err(SendError(model)) => Err(model),
            })
            .await;

        // Act
        client_sender.send(Vec::from(INVALID_DATA)).unwrap();

        // Assert
        assert!(incoming_receiver.recv().await.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn should_complete_without_error_on_exhausted_receiver() -> Result<(), anyhow::Error> {
        // Arrange
        let (client, client_sender, _client_receiver) = construct_connection_medium().await;
        let handle = client.spawn(CancellationToken::new()).await;

        drop(client_sender);

        // Act && Assert
        handle.await??;

        Ok(())
    }

    #[tokio::test]
    async fn should_close_sending_channel_when_completed() -> Result<(), anyhow::Error> {
        // Arrange
        let (client, client_sender, mut client_receiver) = construct_connection_medium().await;
        let handle = client.spawn(CancellationToken::new()).await;

        drop(client_sender);

        // Act
        handle.await??;

        // Assert
        assert!(client_receiver.recv().await.is_none());

        Ok(())
    }
}
