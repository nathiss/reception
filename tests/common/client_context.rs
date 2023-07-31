use cancellable::{Cancellable, CancellableHandle, CancellationToken};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use net::SenderHandle;
use tokio::sync::mpsc::{error::SendError, unbounded_channel, UnboundedReceiver};
use tokio_tungstenite::tungstenite::Message;

use super::{
    mock_model::MockModel,
    types::{Client, WebSocketStream},
};

pub(crate) struct ClientContext {
    // Client sends to Server
    sink: Option<SplitSink<WebSocketStream, Message>>,

    // Client receives from Server
    stream: Option<SplitStream<WebSocketStream>>,

    // Server sends to Client
    client_handle: CancellableHandle<Client>,

    // Server receives from Client
    cm_receiver: Option<UnboundedReceiver<MockModel>>,
}

impl ClientContext {
    pub(super) async fn new(
        client_socket: WebSocketStream,
        listener_client: Client,
        cancellation_token: CancellationToken,
    ) -> Self {
        let (cm_sender, cm_receiver) = unbounded_channel();

        let client_handle = listener_client
            .spawn_with_callback(cancellation_token, move |cm| match cm_sender.send(cm) {
                Ok(()) => Ok(()),
                Err(SendError(cm)) => Err(cm),
            })
            .await;

        let (sink, stream) = client_socket.split();

        Self {
            sink: Some(sink),
            stream: Some(stream),
            client_handle,
            cm_receiver: Some(cm_receiver),
        }
    }

    pub(crate) async fn teardown(mut self) -> Result<(), anyhow::Error> {
        if self.sink.is_some() {
            self.client_send_raw(Message::Close(None))
                .await
                .map_err(|message| {
                    log::error!("Failed to send a close message to the server.");
                    anyhow::anyhow!("Failed to send {:?} to the server.", message)
                })?;
        }

        Ok(())
    }

    pub(crate) async fn sever_send(&mut self, model: MockModel) -> Result<(), MockModel> {
        self.client_handle
            .send(model.clone())
            .await
            .map_err(|_| model)
    }

    pub(crate) async fn server_recv(&mut self) -> Option<MockModel> {
        self.cm_receiver
            .as_mut()
            .expect("cm_receiver to be present")
            .recv()
            .await
    }

    pub(crate) async fn client_send_flush(&mut self) {
        self.sink
            .as_mut()
            .expect("sink to be present")
            .flush()
            .await
            .expect("sink to not be closed");
    }

    pub(crate) async fn client_send_raw(&mut self, message: Message) -> Result<(), Message> {
        match self
            .sink
            .as_mut()
            .expect("sink to be present")
            .send(message.clone())
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => {
                log::error!("Failed to send a message to the server. Reason: {}.", e);
                Err(message)
            }
        }
    }

    pub(crate) async fn client_send(&mut self, model: MockModel) -> Result<(), MockModel> {
        let message = Message::Binary(model.clone().into());
        self.client_send_raw(message).await.map_err(|_| model)
    }

    pub(crate) async fn client_recv_raw(&mut self) -> Option<Message> {
        log::trace!(
            "{}:{} Awaiting new raw message from the server.",
            file!(),
            line!()
        );
        self.stream
            .as_mut()
            .expect("stream to be present")
            .next()
            .await
            .map(|result| match result {
                Ok(msg) => {
                    log::trace!(
                        "{}:{} Received raw message from server: {}.",
                        file!(),
                        line!(),
                        msg
                    );
                    Some(msg)
                }
                Err(e) => {
                    log::error!(
                        "Failed to receiver a message from the server. Reason: {}.",
                        e
                    );
                    None
                }
            })
            .unwrap_or_default()
    }

    pub(crate) async fn client_recv(&mut self) -> Option<MockModel> {
        match self.client_recv_raw().await {
            Some(Message::Binary(data)) => MockModel::try_from(data)
                .or_else(|e| {
                    log::error!(
                        "Failed to deserialize a model from the server. Reason: {}.",
                        e
                    );
                    Err(e)
                })
                .ok(),
            _ => None,
        }
    }

    pub(crate) async fn close_server_send(&mut self) {
        self.client_handle.close().await;
    }

    pub(crate) async fn close_client_send(&mut self) {
        if let Some(mut sink) = self.sink.take() {
            sink.close().await.unwrap();
        }
    }
}
