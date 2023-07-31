use std::fmt::Display;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) enum HandleStatus {
    #[default]
    Normal,

    ServerClosedNormally,

    PingTaskTerminated,

    SendPing(Vec<u8>),
}

impl Display for HandleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandleStatus::Normal => write!(f, "Normal"),
            HandleStatus::ServerClosedNormally => write!(f, "ServerClosedNormally"),
            HandleStatus::PingTaskTerminated => write!(f, "PingTaskTerminated"),
            HandleStatus::SendPing(_) => write!(f, "SendPing"),
        }
    }
}
