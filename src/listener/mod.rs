pub(crate) mod config;
pub(crate) mod error;

use std::{marker::PhantomData, net::SocketAddr};

use cancellable::{Cancellable, CancellationResult, CancellationToken};

use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
};
use tokio_tungstenite::{accept_async_with_config, WebSocketStream};

use crate::{client::Client, connection::Connection, ListenerConfig, ListenerError};

type ClientConnection = Connection<WebSocketStream<TcpStream>>;

/// Binds to a configured port and accepts incoming connections.
#[derive(Debug)]
pub struct Listener<CM, SM>
where
    CM: Send + Sync + 'static,
    SM: Send + Sync + 'static,
{
    config: ListenerConfig,
    tcp_listener: TcpListener,
    cancellation_token: CancellationToken,
    client_sender: UnboundedSender<<Listener<CM, SM> as Cancellable>::Result>,
    client_receiver: UnboundedReceiver<<Listener<CM, SM> as Cancellable>::Result>,

    _cm_marker: PhantomData<CM>,
    _sm_marker: PhantomData<SM>,
}

impl<CM, SM> Listener<CM, SM>
where
    CM: Send + Sync + 'static,
    SM: Send + Sync + 'static,
{
    /// Constructs the listener and binds it to the configured socket.
    ///
    /// # Arguments
    ///
    /// * `config` - configuration for the listener and all its clients.
    /// * `cancellation_token` - cancels the listener, unbinding it from the socket and sends a termination signal to
    /// all clients spawned from this listener.
    ///
    /// # Returns
    pub async fn bind(
        config: ListenerConfig,
        cancellation_token: CancellationToken,
    ) -> Result<Self, ListenerError> {
        let interface = &config.interface;
        let port = config.port;
        let addr: SocketAddr = format!("{}:{}", interface, port)
            .parse()
            .map_err(ListenerError::SocketConfiguration)?;
        let tcp_listener = TcpListener::bind(addr)
            .await
            .map_err(|e| ListenerError::BindingError { addr, source: e })?;

        log::info!("Awaiting new connections on {}.", addr);

        let (client_sender, client_receiver) = unbounded_channel();

        Ok(Self {
            config,
            tcp_listener,
            cancellation_token,
            client_sender,
            client_receiver,
            _cm_marker: PhantomData,
            _sm_marker: PhantomData,
        })
    }

    async fn handle_new_connection(&self, stream: TcpStream, addr: SocketAddr) {
        let config = self.config.clone();
        let cancellation_token = self.cancellation_token.clone();
        let client_sender = self.client_sender.clone();

        tokio::spawn(async move {
            if let Err(e) = stream.readable().await {
                log::error!("Peer connection {addr} has failed to become readable. Error: {e}");
                return;
            };
            if let Err(e) = stream.writable().await {
                log::error!("Peer connection {addr} has failed to become writable. Error: {e}");
                return;
            };

            log::debug!("New connection from: {}.", addr);

            match accept_async_with_config(stream, Some(config.websocket_config.into())).await {
                Ok(websocket) => {
                    let connection_config = config.connection_config.clone();
                    let connection = Connection::new(connection_config, websocket).await;

                    let client =
                        Client::<ClientConnection, CM, SM>::new(connection, cancellation_token)
                            .await;

                    log::debug!("Completed handshake from {addr}");

                    let _ = client_sender.send(client);
                }
                Err(e) => {
                    log::error!("Failed to complete handshake with a client. Error: {e}");
                }
            }
        });
    }

    #[cfg(test)]
    pub fn get_local_addr(&self) -> SocketAddr {
        self.tcp_listener.local_addr().unwrap()
    }
}

#[cancellable::async_trait]
impl<CM, SM> Cancellable for Listener<CM, SM>
where
    CM: Send + Sync + 'static,
    SM: Send + Sync + 'static,
{
    type Result = Client<ClientConnection, CM, SM>;
    type Handle = ();
    type Error = ListenerError;

    async fn new_handle(&mut self) -> Self::Handle {}

    async fn run(&mut self) -> Result<CancellationResult<Self::Result>, Self::Error> {
        log::trace!(
            "{}:{} Awaiting to accept a new TCP connection.",
            file!(),
            line!()
        );

        tokio::select! {
            maybe_connection = self.tcp_listener.accept() => {
                match maybe_connection {
                    Ok((stream, addr)) => {
                        self.handle_new_connection(stream, addr).await;
                        Ok(CancellationResult::Continue)
                    },
                    Err(e) => {
                        log::error!("Failed to accept a new connection. Reason: {}.", e);
                        Err(ListenerError::AcceptConnection(e))
                    }
                }
            }
            maybe_client = self.client_receiver.recv() => {
                match maybe_client {
                    Some(client) => Ok(CancellationResult::Item(client)),
                    None => {
                        log::debug!("Client channel has been exhausted.");
                        Ok(CancellationResult::Break)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, time::Duration};

    use cancellable::{Cancellable, CancellationToken};

    use tokio::{
        net::TcpStream,
        sync::mpsc::{error::SendError, unbounded_channel},
        time::timeout,
    };
    use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

    use crate::listener::ListenerConfig;

    struct MockModel {}

    type Listener = crate::listener::Listener<MockModel, MockModel>;

    async fn connect(
        addr: SocketAddr,
    ) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, anyhow::Error> {
        let (socket, _) = connect_async(format!("ws://{}", addr.to_string())).await?;
        Ok(socket)
    }

    #[tokio::test]
    async fn should_return_error_on_invalid_interface() {
        // Arrange
        let config = ListenerConfig {
            interface: "127.0.0".to_owned(),
            port: 8080,
            ..Default::default()
        };

        // Act
        let result = Listener::bind(config, CancellationToken::new()).await;

        // Assert
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn should_return_listener_on_valid_config() {
        // Arrange
        let config = ListenerConfig {
            interface: "127.0.0.1".to_owned(),
            port: 0,
            ..Default::default()
        };

        // Act
        let result = Listener::bind(config, CancellationToken::new()).await;

        // Assert
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn should_accept_incoming_connection() -> Result<(), anyhow::Error> {
        // Arrange
        let cancellation_token = CancellationToken::new();
        let (client_sender, mut client_receiver) = unbounded_channel();

        let config = ListenerConfig {
            interface: "127.0.0.1".to_owned(),
            port: 0,
            ..Default::default()
        };
        let listener = Listener::bind(config, CancellationToken::new())
            .await
            .unwrap();
        let local_addr = listener.get_local_addr();
        let handle = listener
            .spawn_with_callback(
                cancellation_token.clone(),
                move |client| match client_sender.send(client) {
                    Ok(()) => Ok(()),
                    Err(SendError(client)) => Err(client),
                },
            )
            .await;

        // Act
        let connection = connect(local_addr).await?;

        // Assert
        let client_connection = client_receiver.recv().await;
        assert!(client_connection.is_some());

        let t = timeout(Duration::from_millis(100), handle).await;
        assert!(t.is_err());

        drop(connection);

        Ok(())
    }

    #[tokio::test]
    async fn should_unbind_when_cancelled() -> Result<(), anyhow::Error> {
        // Arrange
        let cancellation_token = CancellationToken::new();
        let (client_sender, mut client_receiver) = unbounded_channel();

        let config = ListenerConfig {
            interface: "127.0.0.1".to_owned(),
            port: 0,
            ..Default::default()
        };
        let listener = Listener::bind(config, CancellationToken::new()).await?;
        let local_addr = listener.get_local_addr();
        let handle = listener
            .spawn_with_callback(
                cancellation_token.clone(),
                move |client| match client_sender.send(client) {
                    Ok(()) => Ok(()),
                    Err(SendError(client)) => Err(client),
                },
            )
            .await;

        // Act
        cancellation_token.cancel();
        handle.await??;

        // Assert
        let connection = connect(local_addr).await;
        assert!(connection.is_err());

        assert!(client_receiver.recv().await.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn should_break_and_complete_when_receiver_is_dropped() -> Result<(), anyhow::Error> {
        // Arrange
        let cancellation_token = CancellationToken::new();
        let (client_sender, client_receiver) = unbounded_channel();

        let config = ListenerConfig {
            interface: "127.0.0.1".to_owned(),
            port: 0,
            ..Default::default()
        };
        let listener = Listener::bind(config, CancellationToken::new()).await?;
        let local_addr = listener.get_local_addr();
        let handle = listener
            .spawn_with_callback(cancellation_token, move |client| {
                match client_sender.send(client) {
                    Ok(()) => Ok(()),
                    Err(SendError(client)) => Err(client),
                }
            })
            .await;

        // Act
        drop(client_receiver);
        // Currently the listener is still awaiting a new connection, since it was not cancelled. We connect one to
        // continue its task.
        let _ = connect(local_addr).await?;

        // Assert
        handle.await??;

        Ok(())
    }
}
