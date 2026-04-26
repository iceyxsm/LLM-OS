use async_trait::async_trait;
use thiserror::Error;

use crate::message::Envelope;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("no subscribers for topic: {0}")]
    NoSubscribers(String),
    #[error("send failed: {0}")]
    SendFailed(String),
    #[error("receive failed: {0}")]
    ReceiveFailed(String),
    #[error("transport closed")]
    Closed,
}

/// The transport trait abstracts the underlying messaging mechanism.
///
/// Implementations may use in-process channels, gRPC streams, NATS, or
/// any other pub/sub system. Modules program against this trait so the
/// transport can be swapped without rewriting business logic.
#[async_trait]
pub trait Transport: Send + Sync {
    /// Publish a message to a topic.
    async fn publish(&self, envelope: Envelope) -> Result<(), TransportError>;

    /// Subscribe to a topic, returning a receiver for incoming messages.
    async fn subscribe(
        &self,
        topic: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<Envelope>, TransportError>;
}
