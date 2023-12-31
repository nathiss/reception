use serde::Deserialize;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig as InternalWebSocketConfig;

const MEGA: usize = 1024 * 1024;

/// Configuration for WebSockets.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WebSocketConfig {
    /// Maximum message size.
    ///
    /// Default: `16MB`
    pub max_message_size: usize,

    /// Maximum frame size.
    ///
    /// Default: `4MB`
    pub max_frame_size: usize,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_message_size: 16 * MEGA,
            max_frame_size: 4 * MEGA,
        }
    }
}

impl From<WebSocketConfig> for InternalWebSocketConfig {
    fn from(val: WebSocketConfig) -> Self {
        InternalWebSocketConfig {
            max_message_size: Some(val.max_message_size),
            max_frame_size: Some(val.max_frame_size),
            ..Default::default()
        }
    }
}
