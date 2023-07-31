use std::{
    iter::repeat_with,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    vec,
};

use async_trait::async_trait;
use futures_util::{Sink, SinkExt};
use tokio::{
    sync::Mutex,
    time::{interval, MissedTickBehavior},
};
use tokio_tungstenite::tungstenite::Message;

use crate::SenderHandle;

use super::{
    shared_state::{ConnectionStatus, HandleHalf, HandleStatus},
    Data,
};

#[derive(Debug)]
pub struct ConnectionHandle<S>
where
    S: Sink<Message> + Send + Unpin + 'static,
    <S as Sink<Message>>::Error: std::fmt::Display,
{
    weak_sink: Weak<Mutex<S>>,
    shared_state: Arc<HandleHalf>,
    has_sent_close_frame: Arc<AtomicBool>,
}

impl<S> ConnectionHandle<S>
where
    S: Sink<Message> + Send + Unpin + 'static,
    <S as Sink<Message>>::Error: std::fmt::Display,
{
    pub(super) fn new(sink: S, shared_state: HandleHalf) -> Self {
        let sink = Arc::new(Mutex::const_new(sink));
        let shared_state = Arc::new(shared_state);
        let has_sent_close_frame = Arc::new(AtomicBool::new(false));

        let weak_sink = Arc::downgrade(&sink);

        Self::spawn_shared_state_task(shared_state.clone(), sink, has_sent_close_frame.clone());
        Self::spawn_ping_task(shared_state.clone(), weak_sink.clone());

        Self {
            weak_sink,
            shared_state,
            has_sent_close_frame,
        }
    }

    fn spawn_shared_state_task(
        shared_state: Arc<HandleHalf>,
        sink: Arc<Mutex<S>>,
        has_sent_close_frame: Arc<AtomicBool>,
    ) {
        let mut handle_receiver = shared_state.receiver();
        tokio::spawn(async move {
            loop {
                match handle_receiver.changed().await {
                    Ok(()) => {
                        let status = *handle_receiver.borrow_and_update();
                        match status {
                            ConnectionStatus::Normal => {
                                log::trace!(
                                    "{}:{} Connection has changed its status to `Normal`.",
                                    file!(),
                                    line!()
                                );
                            }
                            ConnectionStatus::ClientClosedNormally
                            | ConnectionStatus::ClientSentIllegalData
                            | ConnectionStatus::ClientClosedWithoutHandshake
                            | ConnectionStatus::ConnectionTerminated
                            | ConnectionStatus::Unknown => {
                                Self::send_close_frame_if(
                                    sink.clone(),
                                    shared_state.clone(),
                                    has_sent_close_frame.clone(),
                                );
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::trace!(
                            "{}:{} handle_receiver channel has closed. Error: {}.",
                            file!(),
                            line!(),
                            e
                        );
                        break;
                    }
                }
            }
        });
    }

    fn spawn_ping_task(shared_state: Arc<HandleHalf>, sink: Weak<Mutex<S>>) {
        let mut interval = interval(shared_state.config().ping_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                let Some(sink) = sink.upgrade() else {
                    log::warn!("Failed to upgrade client's sink. Ping task is terminating.");
                    Self::notify_connection(shared_state.clone(), HandleStatus::PingTaskTerminated);
                    break;
                };

                let payload: Vec<_> = repeat_with(|| fastrand::u8(0..255)).take(10).collect();

                if let Err(e) = sink
                    .lock_owned()
                    .await
                    .send(Message::Ping(payload.clone()))
                    .await
                {
                    log::warn!(
                        "Failed to send a ping message to the client. Ping task is terminating. Error: {}.",
                        e
                    );
                    Self::notify_connection(shared_state.clone(), HandleStatus::PingTaskTerminated);
                    break;
                }

                Self::notify_connection(shared_state.clone(), HandleStatus::SendPing(payload));
            }
        });
    }

    fn send_close_frame_if(
        sink: Arc<Mutex<S>>,
        shared_state: Arc<HandleHalf>,
        has_sent_close_frame: Arc<AtomicBool>,
    ) {
        if has_sent_close_frame.load(Ordering::SeqCst) {
            return;
        }
        has_sent_close_frame.store(true, Ordering::SeqCst);

        if let Err(e) = futures::executor::block_on(async move {
            let mut sink = sink.lock_owned().await;

            sink.flush().await?;
            sink.close().await?;

            Ok::<(), <S as Sink<Message>>::Error>(())
        }) {
            log::error!(
                "{}:{} Failed to send close frame to a client. Reason: {}.",
                file!(),
                line!(),
                e
            );
        }

        Self::notify_connection(shared_state, HandleStatus::ServerClosedNormally);
    }

    fn notify_connection(shared_state: Arc<HandleHalf>, status: HandleStatus) {
        if let Err(e) = shared_state.notify(status) {
            log::error!("Error: {}", e);
        }
    }
}

impl<S> Drop for ConnectionHandle<S>
where
    S: Sink<Message> + Send + Unpin + 'static,
    <S as Sink<Message>>::Error: std::fmt::Display,
{
    fn drop(&mut self) {
        let Some(sink) = self.weak_sink.upgrade() else {
            return;
        };

        Self::send_close_frame_if(
            sink,
            self.shared_state.clone(),
            self.has_sent_close_frame.clone(),
        );
    }
}

#[async_trait]
impl<S> SenderHandle for ConnectionHandle<S>
where
    S: Sink<Message> + Send + Unpin + 'static,
    <S as Sink<Message>>::Error: std::fmt::Display,
{
    type Item = Data;
    type Error = Self::Item;

    async fn send(&mut self, item: Self::Item) -> Result<(), Self::Item> {
        let Some(sink) = self.weak_sink.upgrade() else {
            log::warn!("Failed to send an item to the client, because its sink has already been dropped.");
            return Err(item);
        };

        sink.lock_owned()
            .await
            .send(Message::binary(item.clone()))
            .await
            .map_err(|_| item)
    }
}
