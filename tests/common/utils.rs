use std::{
    net::SocketAddr,
    sync::atomic::{AtomicU16, Ordering},
    time::Duration,
};

use cancellable::{Cancellable, CancellableHandle};
use reception::ListenerConfig;
use tokio::time::timeout;
use tokio_tungstenite::MaybeTlsStream;

use super::types::WebSocketStream;

lazy_static::lazy_static! {
    static ref NEXT_PORT: AtomicU16 = AtomicU16::new(5300);
}

pub(super) fn get_config() -> ListenerConfig {
    ListenerConfig {
        interface: "0.0.0.0".to_owned(),
        port: NEXT_PORT.fetch_add(1, Ordering::SeqCst),
        ..Default::default()
    }
}

pub(super) fn get_peer_addr(socket: &WebSocketStream) -> Result<SocketAddr, anyhow::Error> {
    let addr = match socket.get_ref() {
        MaybeTlsStream::Plain(s) => s.peer_addr(),
        MaybeTlsStream::NativeTls(s) => s.get_ref().get_ref().get_ref().peer_addr(),
        _ => unreachable!(),
    }?;

    Ok(addr)
}

pub(super) async fn complete_cancellation_handle<C>(
    duration: Duration,
    handle: &mut CancellableHandle<C>,
) where
    C: Cancellable,
{
    match timeout(duration, handle).await {
        Ok(Ok(Ok(_))) => {}
        Ok(Ok(Err(e))) => {
            log::error!("Service completed due to error: {}.", e);
        }
        Ok(Err(e)) => {
            log::error!("Failed to join service's handle. Reason: {}.", e);
        }
        Err(e) => {
            log::error!("Service failed to complete in {}.", e)
        }
    }
}
