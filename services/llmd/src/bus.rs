use common_types::AuditEvent;
use llmos_service_bus::{Envelope, Transport};
use std::sync::Arc;
use tracing::warn;

use crate::AuditSink;

/// An audit sink that publishes events to the service bus on the "audit.events" topic.
pub struct BusAuditSink {
    transport: Arc<dyn Transport>,
    source: String,
}

impl BusAuditSink {
    pub fn new(transport: Arc<dyn Transport>, source: impl Into<String>) -> Self {
        Self {
            transport,
            source: source.into(),
        }
    }
}

impl AuditSink for BusAuditSink {
    fn emit(&self, event: &AuditEvent) {
        let payload = match serde_json::to_value(event) {
            Ok(v) => v,
            Err(err) => {
                warn!(target: "llmd::bus-audit", error = %err, "failed to serialize audit event");
                return;
            }
        };

        let envelope = Envelope::new("audit.events", &self.source, payload);
        let transport = self.transport.clone();

        // Fire-and-forget publish. The transport is async but the AuditSink trait is sync,
        // so we spawn a task. In a production system this would use a bounded channel.
        tokio::spawn(async move {
            if let Err(err) = transport.publish(envelope).await {
                warn!(target: "llmd::bus-audit", error = %err, "failed to publish audit event to bus");
            }
        });
    }
}
