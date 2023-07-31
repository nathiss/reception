mod websocket_config;

use serde::Deserialize;

use crate::connection::ConnectionConfig;

use self::websocket_config::WebSocketConfig;

#[derive(Debug, Deserialize, Clone)]
pub struct ListenerConfig {
    pub interface: String,
    pub port: u16,
    pub websocket_config: WebSocketConfig,
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
