use std::time::Duration;

use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct PingContext {
    payload: Vec<u8>,
    deadline: Instant,
}

impl PingContext {
    pub(super) fn new(payload: Vec<u8>, timeout: Duration) -> Self {
        Self::with_time(payload, timeout, Instant::now())
    }

    fn with_time(payload: Vec<u8>, timeout: Duration, now: Instant) -> Self {
        Self {
            payload,
            deadline: now + timeout,
        }
    }

    pub(crate) fn payload(&self) -> &Vec<u8> {
        &self.payload
    }

    pub(crate) fn deadline(&self) -> &Instant {
        &self.deadline
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::PingContext;

    #[test]
    fn should_return_correct_payload() {
        // Arrange
        let payload = vec![42u8, 42u8];
        let timeout = Duration::from_secs(5);
        let now = Instant::now();

        // Act
        let ping_context = PingContext::with_time(payload.clone(), timeout, now);

        // Assert
        assert_eq!(payload, *ping_context.payload());
    }

    #[test]
    fn should_return_correct_deadline() {
        // Arrange
        let payload = vec![42u8, 42u8];
        let timeout = Duration::from_secs(5);
        let now = Instant::now();

        // Act
        let ping_context = PingContext::with_time(payload, timeout, now);

        // Assert
        assert_eq!(now + timeout, *ping_context.deadline());
    }
}
