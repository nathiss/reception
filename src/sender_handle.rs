use async_trait::async_trait;

/// Sender trait implemented for some service handles.
///
/// Useful for sending data into the service.
#[async_trait]
pub trait SenderHandle {
    /// Type of value send into the service.
    type Item;

    /// Type of error returned when the send has failed.
    type Error;

    /// Sends the given `item` into the service.
    ///
    /// # Arguments
    ///
    /// * `item` - value to the send into the service.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` when it successfully send data into the service,
    /// `Err(Self::Error)` otherwise.
    async fn send(&mut self, item: Self::Item) -> Result<(), Self::Error>;
}
