use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::warn;

use crate::message::Envelope;
use crate::transport::{Transport, TransportError};

const DEFAULT_CHANNEL_CAPACITY: usize = 256;

/// An in-process transport backed by tokio mpsc channels.
///
/// Suitable for single-process deployments and testing. Each topic gets
/// its own set of subscriber channels.
pub struct LocalChannel {
    subscribers: Mutex<HashMap<String, Vec<mpsc::Sender<Envelope>>>>,
    capacity: usize,
}

impl LocalChannel {
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(HashMap::new()),
            capacity: DEFAULT_CHANNEL_CAPACITY,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            subscribers: Mutex::new(HashMap::new()),
            capacity,
        }
    }
}

impl Default for LocalChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for LocalChannel {
    async fn publish(&self, envelope: Envelope) -> Result<(), TransportError> {
        let senders = {
            let map = self
                .subscribers
                .lock()
                .map_err(|_| TransportError::SendFailed("lock poisoned".to_string()))?;
            match map.get(&envelope.topic) {
                Some(senders) => senders.clone(),
                None => return Err(TransportError::NoSubscribers(envelope.topic.clone())),
            }
        };

        if senders.is_empty() {
            return Err(TransportError::NoSubscribers(envelope.topic.clone()));
        }

        for sender in &senders {
            if let Err(err) = sender.try_send(envelope.clone()) {
                warn!(
                    target: "service-bus",
                    topic = %envelope.topic,
                    error = %err,
                    "failed to deliver message to subscriber"
                );
            }
        }

        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<mpsc::Receiver<Envelope>, TransportError> {
        let (tx, rx) = mpsc::channel(self.capacity);
        let mut map = self
            .subscribers
            .lock()
            .map_err(|_| TransportError::SendFailed("lock poisoned".to_string()))?;
        map.entry(topic.to_string()).or_default().push(tx);
        Ok(rx)
    }
}
