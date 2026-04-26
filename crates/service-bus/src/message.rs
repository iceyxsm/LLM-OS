use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

static SEQ: AtomicU64 = AtomicU64::new(1);

/// A unique message identifier combining a timestamp and sequence number.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(String);

impl MessageId {
    pub fn generate() -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default();
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        Self(format!("msg-{ts}-{seq}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A transport-agnostic message envelope.
///
/// The payload is an opaque JSON value. The `topic` field determines routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub id: MessageId,
    pub topic: String,
    pub source: String,
    pub payload: serde_json::Value,
    pub timestamp_unix_ms: u128,
}

impl Envelope {
    pub fn new(
        topic: impl Into<String>,
        source: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: MessageId::generate(),
            topic: topic.into(),
            source: source.into(),
            payload,
            timestamp_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default(),
        }
    }
}
