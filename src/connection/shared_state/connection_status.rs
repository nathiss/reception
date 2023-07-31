use std::fmt::Display;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectionStatus {
    #[default]
    Normal,

    ClientClosedNormally,

    ClientSentIllegalData,

    ClientClosedWithoutHandshake,

    ConnectionTerminated,

    Unknown,
}

impl Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionStatus::Normal => write!(f, "Normal"),
            ConnectionStatus::ClientClosedNormally => write!(f, "ClientClosedNormally"),
            ConnectionStatus::ClientSentIllegalData => write!(f, "ClientSentIllegalData"),
            ConnectionStatus::ClientClosedWithoutHandshake => {
                write!(f, "ClientClosedWithoutHandshake")
            }
            ConnectionStatus::ConnectionTerminated => write!(f, "ConnectionTerminated"),
            ConnectionStatus::Unknown => write!(f, "Unknown"),
        }
    }
}
