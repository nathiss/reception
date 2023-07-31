mod config;
mod connection_handle;
mod error;
mod ping_context;
mod shared_state;

use std::collections::VecDeque;

use cancellable::{Cancellable, CancellationResult};
use futures_util::{
    stream::{SplitSink, SplitStream},
    Sink, Stream, StreamExt,
};
use tokio::time::sleep_until;
use tokio_tungstenite::tungstenite::{self, error::ProtocolError, Message};

use crate::connection::shared_state::HandleStatus;

pub(crate) use self::error::ConnectionError;
use self::{
    connection_handle::ConnectionHandle,
    ping_context::PingContext,
    shared_state::{ConnectionHalf, ConnectionStatus, HandleHalf, SharedState},
};

pub use self::config::ConnectionConfig;

/// Type used for transferring data in and out of the connection.
pub(crate) type Data = Vec<u8>;

/// Receiving layer of peer's connection.
///
/// Handles protocol internals and keeps connection alive.
#[derive(Debug)]
pub struct Connection<S>
where
    S: Sink<Message> + Send,
    <S as Sink<Message>>::Error: std::fmt::Debug + std::fmt::Display + Send,
{
    stream: SplitStream<S>,
    sink: Option<SplitSink<S, Message>>,
    handle_half: Option<HandleHalf>,
    connection_half: ConnectionHalf,
    ping_queue: VecDeque<PingContext>,
    pong_payload_queue: VecDeque<Vec<u8>>,
    config: ConnectionConfig,
}

impl<S> Connection<S>
where
    S: Stream + Sink<Message> + Send + 'static,
    <S as Sink<Message>>::Error: std::fmt::Debug + std::fmt::Display + Send,
{
    pub(crate) async fn new(config: ConnectionConfig, stream: S) -> Self {
        let (sink, stream) = stream.split();

        let (connection_half, handle_half) = SharedState::new(config.clone()).split();

        Self {
            stream,
            sink: Some(sink),
            handle_half: Some(handle_half),
            connection_half,
            ping_queue: VecDeque::with_capacity(2),
            pong_payload_queue: VecDeque::with_capacity(2),
            config,
        }
    }

    fn notify_handle(&self, status: ConnectionStatus) {
        if let Err(e) = self.connection_half.notify(status) {
            log::error!("Error: {}", e);
        }
    }

    fn process_peer_message(
        &mut self,
        item: Option<Result<Message, tungstenite::Error>>,
    ) -> Result<CancellationResult<Data>, ConnectionError<tungstenite::Error>> {
        match item {
            Some(Ok(message)) => {
                match message {
                    Message::Close(frame) => {
                        log::trace!(
                            "A peer closed the connection. Received close frame: {:?}.",
                            frame
                        );

                        self.notify_handle(ConnectionStatus::ClientClosedNormally);
                        Ok(CancellationResult::Break)
                    }
                    Message::Ping(ping) => {
                        log::trace!("Received ping message: {:?}.", ping);
                        Ok(CancellationResult::Continue)
                    }
                    Message::Pong(payload) => {
                        log::debug!("Received a pong message: {:?}.", payload);
                        self.pong_payload_queue.push_back(payload);
                        Ok(CancellationResult::Continue)
                    }
                    Message::Binary(bin) => Ok(CancellationResult::Item(bin)),
                    msg => {
                        // It's either `Message::Text` or `Message::Frame`. According to the docs `Message::Frame` is
                        // impossible and we do not allow text messages.
                        // Terminate the connection.
                        // See: https://docs.rs/tungstenite/latest/tungstenite/protocol/enum.Message.html
                        log::debug!("Received unknown message: {}.", msg);

                        self.notify_handle(ConnectionStatus::ClientSentIllegalData);
                        Err(ConnectionError::IllegalMessageType(msg))
                    }
                }
            }
            Some(Err(tungstenite::Error::ConnectionClosed)) => {
                log::debug!("Connection has been closed by the peer");

                self.notify_handle(ConnectionStatus::ClientClosedNormally);
                Ok(CancellationResult::Break)
            }
            Some(Err(tungstenite::Error::Protocol(
                ProtocolError::ResetWithoutClosingHandshake,
            ))) => {
                log::warn!("Client closed connection without closing handshake.");

                self.notify_handle(ConnectionStatus::ClientClosedWithoutHandshake);
                Ok(CancellationResult::Break)
            }
            Some(Err(websocket_error)) => {
                log::error!(
                    "Received an error while awaiting for a new websocket message: {}.",
                    websocket_error
                );

                self.notify_handle(ConnectionStatus::Unknown);
                Err(ConnectionError::WebSocket(websocket_error))
            }
            None => {
                log::debug!("Connection stream has been exhausted.");
                Ok(CancellationResult::Break)
            }
        }
    }

    async fn construct_pong_deadline(ping_queue: &VecDeque<PingContext>) -> PingContext {
        let front = ping_queue.front();
        match front {
            Some(ping_context) => {
                log::trace!(
                    "{}:{}, Awaiting for deadline: {:?}.",
                    file!(),
                    line!(),
                    ping_context.deadline()
                );
                sleep_until(*ping_context.deadline()).await;
                ping_context.clone()
            }
            None => {
                log::trace!("{}:{} No pings in the queue.", file!(), line!());
                std::future::pending().await
            }
        }
    }
}

#[cancellable::async_trait]
impl<S> Cancellable for Connection<S>
where
    S: Stream<Item = Result<Message, tungstenite::Error>>
        + Sink<Message>
        + std::fmt::Debug
        + Send
        + 'static,
    <S as futures_util::Sink<Message>>::Error: std::fmt::Debug + std::fmt::Display + Send,
{
    type Result = Data;
    type Handle = ConnectionHandle<SplitSink<S, Message>>;
    type Error = ConnectionError<tungstenite::Error>;

    async fn new_handle(&mut self) -> Self::Handle {
        let handle_half = self.handle_half.take().expect("Handle half to be present.");

        self.sink
            .take()
            .map(|sink| ConnectionHandle::new(sink, handle_half))
            .expect("Sending service handle to be present.")
    }

    async fn run(&mut self) -> Result<CancellationResult<Self::Result>, Self::Error> {
        let pong_future = Self::construct_pong_deadline(&self.ping_queue);
        let receiver = self.connection_half.receiver_mut();

        tokio::select! {
            biased;

            ping_context = pong_future => {
                match self.pong_payload_queue.pop_front() {
                    Some(ping_payload) if ping_context.payload() == &ping_payload => {
                        self.ping_queue.pop_front();
                        Ok(CancellationResult::Continue)
                    }
                    Some(ping_payload) => {
                        log::warn!("Client has failed to respond with a correct pong payload.");
                        log::debug!("Received payload: {:?}.", ping_payload);
                        self.notify_handle(ConnectionStatus::ConnectionTerminated);
                        Ok(CancellationResult::Break)
                    }
                    None => {
                        log::warn!("Client failed to send a pong message within the given deadline.");
                        self.notify_handle(ConnectionStatus::ConnectionTerminated);
                        Ok(CancellationResult::Break)
                    }
                }
            }
            handle_status_update = receiver.changed() => {
                match handle_status_update {
                    Ok(()) => {
                        let status = receiver.borrow_and_update().clone();
                        match status {
                            HandleStatus::Normal => {
                                log::trace!("{}:{} Handle has changed its status to Normal.", file!(), line!());
                                Ok(CancellationResult::Continue)
                            },
                            HandleStatus::ServerClosedNormally => {
                                // TODO: await for client's close frame, but only for a limited time.
                                Ok(CancellationResult::Break)
                            },
                            HandleStatus::PingTaskTerminated => {
                                // Ping task has terminated due to an error.
                                Ok(CancellationResult::Break)
                            },
                            HandleStatus::SendPing(payload) => {
                                let pong_timeout = self.config.pong_timeout;
                                self.ping_queue.push_back(PingContext::new(payload, pong_timeout));
                                Ok(CancellationResult::Continue)
                            },
                        }
                    }
                    Err(e) => {
                        log::warn!("Failed to receive a message from the connection handle. Error: {}.", e);
                        Ok(CancellationResult::Break)
                    }
                }
            }
            item = self.stream.next() => {
                self.process_peer_message(item)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::Poll,
        time::Duration,
    };

    use cancellable::{Cancellable, CancellationToken};
    use futures_util::{Sink, Stream};
    use pin_project::pin_project;
    use tokio::{
        sync::mpsc::{
            self, error::SendError, unbounded_channel, UnboundedReceiver, UnboundedSender,
        },
        time::timeout,
    };
    use tokio_tungstenite::tungstenite::{self, protocol::frame::Frame, Message};

    use crate::{
        connection::{Connection, ConnectionError},
        SenderHandle,
    };

    use super::ConnectionConfig;

    async fn create_communication_medium_with_config_mod<F>(
        f: F,
    ) -> (
        Connection<MockStreamSink>,
        UnboundedSender<Message>,
        UnboundedReceiver<Message>,
    )
    where
        F: FnOnce(ConnectionConfig) -> ConnectionConfig,
    {
        let config = f(ConnectionConfig::default());

        let (out_send, out_recv) = unbounded_channel();
        let (in_send, in_recv) = unbounded_channel();
        let stream_sink = MockStreamSink::new(in_send, out_recv, Arc::new(AtomicBool::new(false)));

        let connection = Connection::new(config, stream_sink).await;

        (connection, out_send, in_recv)
    }

    async fn create_communication_medium() -> (
        Connection<MockStreamSink>,
        UnboundedSender<Message>,
        UnboundedReceiver<Message>,
    ) {
        create_communication_medium_with_config_mod(|config| config).await
    }

    #[derive(Debug)]
    #[pin_project]
    struct MockStreamSink {
        #[pin]
        sender: mpsc::UnboundedSender<Message>,
        #[pin]
        receiver: mpsc::UnboundedReceiver<Message>,
        #[pin]
        is_closed: Arc<AtomicBool>,
    }

    impl MockStreamSink {
        fn new(
            sender: mpsc::UnboundedSender<Message>,
            receiver: mpsc::UnboundedReceiver<Message>,
            is_closed: Arc<AtomicBool>,
        ) -> Self {
            Self {
                sender,
                receiver,
                is_closed,
            }
        }
    }

    impl Stream for MockStreamSink {
        type Item = Result<Message, tungstenite::Error>;

        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            let mut this = self.project();
            match this.receiver.poll_recv(cx) {
                Poll::Ready(msg) => {
                    Poll::Ready(Some(msg.ok_or(tungstenite::Error::ConnectionClosed)))
                }
                Poll::Pending => Poll::Pending,
            }
        }
    }

    impl Sink<Message> for MockStreamSink {
        type Error = tungstenite::Error;

        fn poll_ready(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn start_send(self: std::pin::Pin<&mut Self>, item: Message) -> Result<(), Self::Error> {
            let this = self.project();
            this.sender
                .send(item)
                .map_err(|_e| tungstenite::Error::AlreadyClosed)
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_close(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            let this = self.project();
            this.is_closed.store(true, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }
    }

    unsafe impl Send for MockStreamSink {}

    #[tokio::test]
    async fn should_return_binary_message_sent_by_peer() -> Result<(), anyhow::Error> {
        // Arrange
        let (msg_sender, mut msg_recv) = unbounded_channel();

        let (connection, out_send, _out_recv) = create_communication_medium().await;
        let handle = connection
            .spawn_with_callback(CancellationToken::new(), move |msg| {
                match msg_sender.send(msg) {
                    Ok(_) => Ok(()),
                    Err(SendError(msg)) => Err(msg),
                }
            })
            .await;

        let data = vec![1u8, 3u8, 3u8, 7u8];

        // Act
        out_send.send(Message::Binary(data.clone()))?;

        // Assert
        let received_data = msg_recv.recv().await.unwrap();
        assert_eq!(data, received_data);

        let t = timeout(Duration::from_millis(100), handle).await;
        assert!(t.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn should_send_binary_message_to_peer_when_received_new_data() -> Result<(), anyhow::Error>
    {
        // Arrange
        let (connection, _out_send, mut out_recv) = create_communication_medium().await;
        let mut handle = connection.spawn(CancellationToken::new()).await;

        let data = vec![1u8, 3u8, 3u8, 7u8];

        // Act
        handle.send(data.clone()).await.unwrap();

        // Assert
        let message = timeout(Duration::from_millis(200), out_recv.recv())
            .await?
            .unwrap();
        let Message::Binary(raw_message) = message else {
            panic!("Message must be of binary type.");
        };
        assert_eq!(data, raw_message);

        Ok(())
    }

    #[tokio::test]
    async fn should_complete_task_when_peer_closed_connection() -> Result<(), anyhow::Error> {
        // Arrange
        let (connection, out_send, _) = create_communication_medium().await;
        let handle = connection.spawn(CancellationToken::new()).await;

        // Act
        out_send.send(Message::Close(None))?;

        // Assert
        handle.await??;

        Ok(())
    }

    #[tokio::test]
    async fn should_complete_task_when_received_text_message() -> Result<(), anyhow::Error> {
        // Arrange
        let msg = Message::Text("1337".to_owned());

        let (sender, mut receiver) = unbounded_channel();

        let (connection, out_send, _out_recv) = create_communication_medium().await;
        let handle = connection
            .spawn_with_callback(CancellationToken::new(), move |package| {
                match sender.send(package) {
                    Ok(()) => Ok(()),
                    Err(SendError(package)) => Err(package),
                }
            })
            .await;

        // Act
        out_send.send(msg.clone())?;

        // Assert
        assert_eq!(None, receiver.recv().await);
        let Err(ConnectionError::IllegalMessageType(error_msg)) = handle.await? else {
            panic!("Expected a `ConnectionError::IllegalMessageType` error.");
        };
        assert_eq!(msg, error_msg);

        Ok(())
    }

    #[tokio::test]
    async fn should_complete_task_when_received_frame_message() -> Result<(), anyhow::Error> {
        // Arrange
        let msg = Message::Frame(Frame::ping(vec![]));
        let (sender, mut receiver) = unbounded_channel();

        let (connection, out_send, _out_recv) = create_communication_medium().await;
        let handle = connection
            .spawn_with_callback(CancellationToken::new(), move |package| {
                match sender.send(package) {
                    Ok(()) => Ok(()),
                    Err(SendError(package)) => Err(package),
                }
            })
            .await;

        // Act
        out_send.send(msg.clone())?;

        // Assert
        assert_eq!(None, receiver.recv().await);
        let Err(ConnectionError::IllegalMessageType(error_msg)) = handle.await? else {
            panic!("Expected a `ConnectionError::IllegalMessageType` error.");
        };
        assert_eq!(msg, error_msg);

        Ok(())
    }

    #[tokio::test]
    async fn should_complete_task_when_peer_closes_without_close_message(
    ) -> Result<(), anyhow::Error> {
        // Arrange
        let (connection, out_send, _) = create_communication_medium().await;
        let handle = connection.spawn(CancellationToken::new()).await;

        // Act
        drop(out_send);

        // Assert
        handle.await??;

        Ok(())
    }

    #[tokio::test]
    async fn should_complete_task_when_fails_to_send_new_message() -> Result<(), anyhow::Error> {
        // Arrange
        let data = vec![42u8];

        let (connection, out_send, _) = create_communication_medium().await;
        let handle = connection
            .spawn_with_callback(CancellationToken::new(), |package| Err(package))
            .await;

        // Act
        out_send.send(Message::Binary(data.clone()))?;

        // Assert
        handle.await??;

        Ok(())
    }

    #[tokio::test]
    async fn server_should_terminate_if_client_fails_to_respond_to_ping(
    ) -> Result<(), anyhow::Error> {
        // Arrange
        let pong_timeout = Duration::from_millis(100);

        let (connection, out_send, mut out_recv) =
            create_communication_medium_with_config_mod(|mut config| {
                config.ping_interval = Duration::from_millis(400);
                config.pong_timeout = pong_timeout.clone();
                config
            })
            .await;
        let handle = connection
            .spawn_with_callback(CancellationToken::new(), |package| Err(package))
            .await;

        assert!(
            out_recv
                .recv()
                .await
                .expect("message to be present")
                .is_ping(),
            "Expected message to be a ping message"
        );

        // Act
        // We do nothing to trigger the effect.

        // Assert
        // We do not assert for a close frame, as it's not being sent by us. Tungstenite handles
        // closing handshake for us.
        assert!(
            out_recv.recv().await.is_none(),
            "Expected stream to be closed"
        );

        assert!(
            out_send.send(Message::Ping(vec![42u8])).is_err(),
            "Expected out_send to be closed"
        );

        handle.await??;

        Ok(())
    }
}
