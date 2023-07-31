use std::{net::SocketAddr, time::Duration};

use cancellable::{Cancellable, CancellableHandle, CancellationToken};

use net::ListenerConfig;
use tokio::{
    net::TcpStream,
    sync::mpsc::{error::SendError, unbounded_channel, UnboundedReceiver},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, handshake::client::Request},
};

use crate::common::utils::get_peer_addr;

use super::{
    client_context::ClientContext,
    types::{Client, Listener},
    utils::{complete_cancellation_handle, get_config},
};

pub(crate) struct TestContext {
    config: ListenerConfig,
    cancellation_token: CancellationToken,
    listener_addr: SocketAddr,
    listener_handle: CancellableHandle<Listener>,
    client_receiver: UnboundedReceiver<Client>,
}

impl TestContext {
    pub(crate) async fn new() -> Result<Self, anyhow::Error> {
        let config = get_config();
        Self::from_config(config).await
    }

    pub(crate) async fn with_config_modification<F>(f: F) -> Result<Self, anyhow::Error>
    where
        F: FnOnce(ListenerConfig) -> ListenerConfig,
    {
        let config = get_config();
        let updated_config = f(config);
        Self::from_config(updated_config).await
    }

    async fn from_config(config: ListenerConfig) -> Result<Self, anyhow::Error> {
        let cancellation_token = CancellationToken::new();

        let listener_addr = SocketAddr::new(config.interface.parse()?, config.port);

        let (client_sender, client_receiver) = unbounded_channel();

        let listener = Listener::bind(config.clone(), cancellation_token.clone()).await?;
        let listener_handle = listener
            .spawn_with_callback(
                cancellation_token.clone(),
                move |client| match client_sender.send(client) {
                    Ok(()) => Ok(()),
                    Err(SendError(client)) => {
                        log::error!("Failed to queue up a new client connection.");
                        Err(client)
                    }
                },
            )
            .await;

        Ok(Self {
            config,
            cancellation_token,
            listener_addr,
            listener_handle,
            client_receiver,
        })
    }

    pub(crate) fn config(&self) -> &ListenerConfig {
        &self.config
    }

    pub(crate) async fn teardown(&mut self) -> Result<(), anyhow::Error> {
        self.cancellation_token.cancel();
        complete_cancellation_handle(Duration::from_secs(3), &mut self.listener_handle).await;

        Ok(())
    }

    pub(crate) fn cancellation_token(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub(crate) async fn new_client(&mut self) -> Result<ClientContext, anyhow::Error> {
        let request = self.create_basic_request();
        let uri = request.uri().clone();

        log::debug!("Creating a new WebSocket connection to {}...", uri);

        let (socket, _) = connect_async(request).await?;

        let client = self
            .client_receiver
            .recv()
            .await
            .ok_or(anyhow::anyhow!("Incoming client stream has been closed."))?;

        let peer_addr = get_peer_addr(&socket)?;

        log::debug!(
            "Successfully created a connection between server({}) and client({}).",
            self.listener_addr,
            peer_addr,
        );

        let client_context = ClientContext::new(socket, client, self.cancellation_token()).await;
        Ok(client_context)
    }

    pub(crate) async fn new_tcp_client(&self) -> Result<TcpStream, anyhow::Error> {
        let connection = TcpStream::connect(self.listener_addr).await?;
        Ok(connection)
    }

    fn create_basic_request(&self) -> Request {
        format!("ws://{}", self.listener_addr)
            .into_client_request()
            .unwrap()
    }
}
