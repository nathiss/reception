mod websocket_config;

use serde::Deserialize;

use crate::connection::ConnectionConfig;

pub use self::websocket_config::WebSocketConfig;

/// Configures the listener.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ListenerConfig {
    /// Interface to which the listener will bind.
    ///
    /// Default: `"127.0.0.1"`
    pub interface: String,

    /// Port to which the listener will bind.
    ///
    /// Default: `8080`
    pub port: u16,

    /// Configuration for WebSockets.
    pub websocket_config: WebSocketConfig,

    /// Configuration for peer connections.
    pub connection_config: ConnectionConfig,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            interface: "127.0.0.1".to_owned(),
            port: 8080,
            websocket_config: Default::default(),
            connection_config: Default::default(),
        }
    }
}
