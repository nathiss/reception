use reception::{
    client::{self},
    connection::Connection,
};
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;

use super::mock_model::MockModel;

pub(super) type WebSocketStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

pub(super) type Client =
    client::Client<Connection<tokio_tungstenite::WebSocketStream<TcpStream>>, MockModel, MockModel>;

pub(crate) type Listener = reception::Listener<MockModel, MockModel>;
