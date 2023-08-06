use cancellable::Cancellable;
use reception::{client::Client, connection::Connection, SenderHandle};
use tokio::{net::TcpStream, sync::mpsc::unbounded_channel};
use tokio_tungstenite::WebSocketStream;
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
struct Model {
    payload: Vec<u8>,
}

impl Into<Vec<u8>> for Model {
    fn into(self) -> Vec<u8> {
        self.payload
    }
}

impl TryFrom<Vec<u8>> for Model {
    type Error = anyhow::Error;

    fn try_from(payload: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(Self { payload })
    }
}

fn handle_client(
    client: Client<Connection<WebSocketStream<TcpStream>>, Model, Model>,
    cancellation_token: CancellationToken,
) -> Result<(), Client<Connection<WebSocketStream<TcpStream>>, Model, Model>> {
    tokio::spawn(async move {
        let (tx, mut rx) = unbounded_channel();
        let mut handle = client
            .spawn_with_callback(cancellation_token, move |msg| {
                tx.send(msg).unwrap();
                Ok(())
            })
            .await;

        while let Some(msg) = rx.recv().await {
            handle.send(msg).await.unwrap();
        }
    });
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), anyhow::Error> {
    let cancellation_token = CancellationToken::new();

    let listener =
        reception::Listener::<Model, Model>::bind(Default::default(), cancellation_token.clone())
            .await?;

    let handle = listener
        .spawn_with_callback(cancellation_token.clone(), move |client| {
            handle_client(client, cancellation_token.clone())
        })
        .await;

    handle.await??;

    Ok(())
}
