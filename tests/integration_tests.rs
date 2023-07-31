mod common;

use std::time::Duration;

use net::ListenerConfig;
use tokio::{io::AsyncWriteExt, time::timeout};
use tokio_tungstenite::tungstenite::Message;

use crate::common::{MockModel, TestContext};

#[cfg(test)]
#[ctor::ctor]
fn setup() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();
}

#[tokio::test]
async fn should_handle_valid_websocket_connection() -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;

    // Act & Assert
    let client_context = context.new_client().await?;

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_process_client_message() -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;
    let mut client_context = context.new_client().await?;

    let model = MockModel::valid();

    // Act
    client_context.client_send(model.clone()).await.unwrap();

    // Assert
    let received_model = client_context.server_recv().await.unwrap();
    assert_eq!(model, received_model);

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_pass_messages_to_client() -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;
    let mut client_context = context.new_client().await?;

    let model = MockModel::valid();

    // Act
    client_context.sever_send(model.clone()).await.unwrap();
    log::trace!("{}:{}", file!(), line!());

    // Assert
    let received_model = client_context.client_recv().await.unwrap();
    log::trace!("{}:{}", file!(), line!());
    assert_eq!(model, received_model);

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_accept_multiple_connection_at_once() -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;

    // Act
    let mut client_contexts = Vec::with_capacity(10);
    for _ in 0..10 {
        client_contexts.push(context.new_client().await?);
    }

    // Assert
    for client_context in client_contexts.into_iter() {
        client_context.teardown().await?;
    }

    // Teardown
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn client_should_receive_close_frame_send_by_server() -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;
    let mut client_context = context.new_client().await?;

    // Act
    client_context.close_server_send().await;

    // Assert
    let mut message = client_context.client_recv_raw().await.unwrap();
    if message.is_ping() {
        message = client_context.client_recv_raw().await.unwrap();
    }

    let Message::Close(_) = message else {
        panic!("Expected a close frame.");
    };
    assert!(
        client_context.client_recv_raw().await.is_none(),
        "Expected client receiving half to be closed after the close frame."
    );

    // Teardown
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn server_should_receive_close_frame_send_by_client() -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;
    let mut client_context = context.new_client().await?;

    // Act
    client_context.close_client_send().await;

    // Assert
    // There's no way of receiving a "raw" message from client on the server side. All WebSocket internals are being
    // handled by the `net` module. However, once the client closed it's sending half of the connection we should be
    // able to complete on server's receiving side.
    assert!(
        client_context.server_recv().await.is_none(),
        "Expected server receiving half to be closed after the close frame."
    );

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn server_should_respond_with_its_close_frame_when_receives_close_frame_from_client(
) -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;
    let mut client_context = context.new_client().await?;

    // Act
    client_context
        .client_send_raw(Message::Close(None))
        .await
        .unwrap();
    client_context.close_client_send().await;

    // Assert
    let mut message = client_context.client_recv_raw().await.unwrap();
    if message.is_ping() {
        message = client_context.client_recv_raw().await.unwrap();
    }

    assert!(message.is_close(), "Expected a close frame");

    assert!(
        client_context.client_recv_raw().await.is_none(),
        "Expected client receiving half to be closed after the close frame."
    );

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn client_should_receive_ping_after_configured_interval() -> Result<(), anyhow::Error> {
    // Arrange
    let update_delays = |mut config: ListenerConfig| {
        config.connection_config.ping_interval = Duration::from_millis(300);
        config
    };

    let mut context = TestContext::with_config_modification(update_delays).await?;
    let mut client_context = context.new_client().await?;

    // Act
    // We do nothing to trigger the effect. The server _should_ periodically send a ping message to the client and if
    // the client fails to respond within the time window, then the connection should be terminated.

    // Assert
    let Ok(Some(message)) = timeout(
        Duration::from_millis(300 + 200),
        client_context.client_recv_raw(),
    )
    .await else {
        panic!("Expected a message to be a ping message.");
    };
    assert!(message.is_ping());

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn server_should_not_terminate_the_connection_if_client_responds_to_ping(
) -> Result<(), anyhow::Error> {
    // Arrange
    let update_delays = |mut config: ListenerConfig| {
        config.connection_config.ping_interval = Duration::from_secs(2);
        config.connection_config.pong_timeout = Duration::from_millis(500);
        config
    };

    let mut context = TestContext::with_config_modification(update_delays).await?;
    let mut client_context = context.new_client().await?;

    // Act
    assert!(
        client_context.client_recv_raw().await.unwrap().is_ping(),
        "Expected a ping message."
    );

    log::info!("[TEST] Got a ping message. Responding with a pong.");

    // Tungstenite library handles pong message for us. However, the future still needs to be awaited.
    // We _flush_ the sink to make sure that the pong gets send to the server.
    client_context.client_send_flush().await;

    // Assert
    assert!(
        client_context.client_recv_raw().await.unwrap().is_ping(),
        "expected the next server message to be ping"
    );

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_handle_multiple_ping_pong_exchanges_and_graceful_shutdown(
) -> Result<(), anyhow::Error> {
    // Arrange
    let update_delays = |mut config: ListenerConfig| {
        config.connection_config.ping_interval = Duration::from_millis(300);
        config.connection_config.pong_timeout = Duration::from_millis(100);
        config
    };

    let mut context = TestContext::with_config_modification(update_delays).await?;
    let mut client_context = context.new_client().await?;

    // Act
    for i in 0..10 {
        let message = client_context.client_recv_raw().await.unwrap();
        assert!(message.is_ping(), "Expected a ping message from server");

        log::info!(
            "[TEST #{}] Got a pong message from server: {:?}",
            i,
            message
        );
    }

    client_context
        .client_send_raw(Message::Close(None))
        .await
        .unwrap();
    client_context.close_client_send().await;

    // Assert
    assert!(
        client_context.client_recv_raw().await.unwrap().is_close(),
        "expected a close frame from the server"
    );

    assert!(
        client_context.client_recv_raw().await.is_none(),
        "expected connection to be closed"
    );

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
#[ignore = "Not sure if it's possible to detect that peer's closed connection."]
async fn server_should_respond_with_its_close_frame_when_client_closes_connection_without_handshake(
) -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;
    let mut client_context = context.new_client().await?;

    assert!(
        client_context.client_recv_raw().await.unwrap().is_ping(),
        "Expected first message to be a ping"
    );

    // Act
    client_context.close_client_send().await;

    // Assert
    assert!(
        client_context.client_recv_raw().await.unwrap().is_close(),
        "Expected a close frame from the server"
    );

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_process_messages_from_multiple_connections_at_once() -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;

    let mut client_contexts = Vec::with_capacity(10);
    for _ in 0..10 {
        client_contexts.push(context.new_client().await?);
    }

    // Act
    let tasks: Vec<_> = client_contexts
        .into_iter()
        .map(|mut client_context| {
            tokio::spawn(async move {
                let model = MockModel::valid();

                client_context.client_send(model.clone()).await.unwrap();
                let received_model = client_context.server_recv().await.unwrap();

                assert_eq!(model, received_model);
                client_context
            })
        })
        .collect();

    // Assert
    for task in tasks.into_iter() {
        let client_context = task.await?;
        client_context.teardown().await?;
    }

    // Teardown
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_accept_new_connection_when_previous_client_completes_handshake(
) -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;

    // Act
    let mut tcp_client = context.new_tcp_client().await?;
    tcp_client.write_all(&[42u8]).await?;

    // Assert
    log::info!("aa");
    let client_context = timeout(Duration::from_millis(100), context.new_client()).await?;
    assert!(
        client_context.is_ok(),
        "Expected WebSocket connection to connect successfully"
    );

    // Teardown
    client_context.unwrap().teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_respond_with_pong_when_client_sends_ping() -> Result<(), anyhow::Error> {
    // Arrange
    let payload = vec![42u8, 42u8];

    let mut context = TestContext::new().await?;
    let mut client_context = context.new_client().await?;

    // Act
    client_context
        .client_send_raw(Message::Ping(payload.clone()))
        .await
        .unwrap();

    // Assert
    let mut message = client_context.client_recv_raw().await.unwrap();
    if message.is_ping() {
        // Ignore ping sent by the server.
        message = client_context.client_recv_raw().await.unwrap();
    }

    let Message::Pong(received_payload) = message else {
        panic!("Expected to receive a pong frame.");
    };

    assert_eq!(payload, received_payload);

    // Teardown
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_complete_gracefully_when_client_terminates_with_close_frame(
) -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;
    let mut client_context = context.new_client().await?;

    assert!(
        client_context.client_recv_raw().await.unwrap().is_ping(),
        "Expected first message to be ping."
    );

    // Act
    client_context.close_client_send().await;

    // Assert
    assert!(
        client_context.client_recv_raw().await.unwrap().is_close(),
        "Expected a close frame from the server"
    );
    assert!(
        client_context.client_recv_raw().await.is_none(),
        "Expected server to terminate its connection"
    );

    // Teardown
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_handle_error_when_client_terminates_without_close_frame(
) -> Result<(), anyhow::Error> {
    // Arrange
    let update_delays = |mut config: ListenerConfig| {
        config.connection_config.ping_interval = Duration::from_millis(300);
        config.connection_config.pong_timeout = Duration::from_millis(100);
        config
    };

    let mut context = TestContext::with_config_modification(update_delays).await?;
    let mut client_context = context.new_client().await?;

    client_context.close_client_send().await;
    loop {
        if client_context.client_recv_raw().await.is_none() {
            break;
        }
    }

    // Act
    let mut new_client = context.new_client().await?;

    // Assert
    assert!(
        new_client.client_recv_raw().await.is_some(),
        "Expected server to send some message"
    );

    // Teardown
    new_client.teardown().await?;
    client_context.teardown().await?;
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_unbind_when_cancelled() -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;
    let port = context.config().port;

    let port_mod = move |mut config: ListenerConfig| {
        config.port = port;
        config
    };

    // Act
    context.teardown().await?;

    // Assert

    let mut new_context = TestContext::with_config_modification(port_mod).await?;
    let new_client = new_context.new_client().await?;

    // Teardown
    new_client.teardown().await?;
    new_context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_terminate_client_connection_if_message_too_big() -> Result<(), anyhow::Error> {
    // Arrange
    let config_mod = |mut config: ListenerConfig| {
        config.websocket_config.max_frame_size = 512;
        config.websocket_config.max_message_size = 1024;
        config
    };

    let mut context = TestContext::with_config_modification(config_mod).await?;
    let mut client_context = context.new_client().await?;

    assert!(
        client_context.client_recv_raw().await.unwrap().is_ping(),
        "Expected first message to be ping."
    );

    // Act
    client_context
        .client_send(MockModel::valid_with_size(2 * 1024))
        .await
        .unwrap();

    // Assert
    assert!(
        client_context.client_recv_raw().await.unwrap().is_close(),
        "Expected a close frame from the server"
    );
    assert!(
        client_context.client_recv_raw().await.is_none(),
        "Expected server to terminate its connection"
    );

    // Teardown
    context.teardown().await?;

    Ok(())
}

#[tokio::test]
async fn should_await_processing_when_client_sends_too_many_messages() -> Result<(), anyhow::Error>
{
    // Arrange

    // Act

    // Assert

    // Teardown

    Ok(())
}

#[tokio::test]
async fn should_terminate_when_client_sends_invalid_message() -> Result<(), anyhow::Error> {
    // Arrange
    let mut context = TestContext::new().await?;
    let mut client_context = context.new_client().await?;

    assert!(
        client_context.client_recv_raw().await.unwrap().is_ping(),
        "Expected first message to be ping."
    );

    // Act
    client_context
        .client_send(MockModel::invalid())
        .await
        .unwrap();

    // Assert
    assert!(
        client_context.client_recv_raw().await.unwrap().is_close(),
        "Expected a close frame from the server"
    );
    assert!(
        client_context.client_recv_raw().await.is_none(),
        "Expected server to terminate its connection"
    );

    // Teardown
    context.teardown().await?;

    Ok(())
}
