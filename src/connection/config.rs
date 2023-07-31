use std::time::Duration;

use serde::Deserialize;

/// Configures connection to a peer.
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnectionConfig {
    /// An interval used for delaying ping message send from the server to its clients.
    ///
    /// Default: `20s`
    #[serde(with = "humantime_serde")]
    pub ping_interval: Duration,

    /// An interval during which the client must respond to a ping message.
    ///
    /// If a client fails to deliver a pong message with the matching payload, the server will assume the connection to
    /// be ill-formed and will disconnect the peer.
    ///
    /// Default: `4s`
    #[serde(with = "humantime_serde")]
    pub pong_timeout: Duration,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(20),
            pong_timeout: Duration::from_secs(4),
        }
    }
}
