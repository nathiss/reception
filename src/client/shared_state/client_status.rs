use std::fmt::Display;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientStatus {
    #[default]
    Normal,

    InvalidModel,
}

impl Display for ClientStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientStatus::Normal => write!(f, "Normal"),
            ClientStatus::InvalidModel => write!(f, "InvalidModel"),
        }
    }
}
