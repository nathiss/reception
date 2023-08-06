# reception

[![ci-master](https://github.com/nathiss/reception/actions/workflows/ci-master.yaml/badge.svg)](https://github.com/nathiss/reception/actions/workflows/ci-master.yaml)
[![Crates.io](https://img.shields.io/crates/v/reception)](https://crates.io/crates/reception)
[![docs.rs](https://docs.rs/reception/badge.svg)](https://docs.rs/reception/)
![Crates.io](https://img.shields.io/crates/l/reception)

This crate provides a way of binding a TCP listener that will accept incoming
[WebSocket](https://en.wikipedia.org/wiki/WebSocket) connections. Additionally it provides an abstraction layer (see
[`Client`](crate::client::Client)) for serializing and deserializing well-defined models.

## Examples

```rust
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
```

## License

See [LICENSE.txt](./LICENSE.txt) file.
