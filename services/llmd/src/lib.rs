use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use common_types::{
    ActionRequest, ActionResult, ActionStatus, AuditEvent, LlmOsError, PolicyDecisionRecord,
    PolicyEffect,
};
use controlplane_api::{policy_service_client::PolicyServiceClient, EvaluatePolicyRequest};
use tonic::{metadata::MetadataValue, transport::Channel, Request};
use tracing::{info, warn};

#[async_trait]
pub trait PolicyDecisionClient {
    async fn evaluate(
        &mut self,
        request: &ActionRequest,
    ) -> Result<PolicyDecisionRecord, LlmOsError>;
}

pub trait ActionExecutor {
    fn execute(&self, request: &ActionRequest) -> Result<ActionResult, LlmOsError>;
}

pub trait AuditSink {
    fn emit(&self, event: &AuditEvent);
}

pub struct StdoutAuditSink;

impl AuditSink for StdoutAuditSink {
    fn emit(&self, event: &AuditEvent) {
        info!(target: "llmd::audit", event = ?event, "audit event");
    }
}

pub struct JsonlFileAuditSink {
    file: Mutex<std::fs::File>,
}

impl JsonlFileAuditSink {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }
}

impl AuditSink for JsonlFileAuditSink {
    fn emit(&self, event: &AuditEvent) {
        let mut file = match self.file.lock() {
            Ok(file) => file,
            Err(_) => {
                warn!(target: "llmd::audit", "failed to acquire audit file lock");
                return;
            }
        };

        if let Err(err) = serde_json::to_writer(&mut *file, event) {
            warn!(target: "llmd::audit", error = %err, "failed to serialize audit event");
            return;
        }

        if let Err(err) = writeln!(file) {
            warn!(target: "llmd::audit", error = %err, "failed to write audit newline");
            return;
        }

        if let Err(err) = file.flush() {
            warn!(target: "llmd::audit", error = %err, "failed to flush audit log file");
        }
    }
}

pub struct NoopExecutor;

impl ActionExecutor for NoopExecutor {
    fn execute(&self, request: &ActionRequest) -> Result<ActionResult, LlmOsError> {
        Ok(ActionResult {
            version: request.version.clone(),
            status: ActionStatus::Executed,
            message: format!(
                "executed {} for subject {} on {}",
                request.action, request.subject, request.resource
            ),
        })
    }
}

pub struct GrpcPolicyDecisionClient {
    inner: PolicyServiceClient<Channel>,
    timeout: Duration,
}

impl GrpcPolicyDecisionClient {
    pub async fn connect(endpoint: String, timeout: Duration) -> anyhow::Result<Self> {
        let inner = PolicyServiceClient::connect(endpoint).await?;
        Ok(Self { inner, timeout })
    }
}

#[async_trait]
impl PolicyDecisionClient for GrpcPolicyDecisionClient {
    async fn evaluate(
        &mut self,
        request: &ActionRequest,
    ) -> Result<PolicyDecisionRecord, LlmOsError> {
        let mut grpc_request = Request::new(EvaluatePolicyRequest {
            subject: request.subject.clone(),
            action: request.action.clone(),
            resource: request.resource.clone(),
        });

        let request_id = MetadataValue::try_from(request.request_id.as_str()).map_err(|_| {
            LlmOsError::PolicyUnavailable(
                "request_id contains invalid metadata characters".to_string(),
            )
        })?;
        let correlation_id =
            MetadataValue::try_from(request.correlation_id.as_str()).map_err(|_| {
                LlmOsError::PolicyUnavailable(
                    "correlation_id contains invalid metadata characters".to_string(),
                )
            })?;

        grpc_request
            .metadata_mut()
            .insert("x-request-id", request_id);
        grpc_request
            .metadata_mut()
            .insert("x-correlation-id", correlation_id);

        let rpc_result = tokio::time::timeout(self.timeout, self.inner.evaluate(grpc_request))
            .await
            .map_err(|_| {
                LlmOsError::PolicyUnavailable(
                    "policy evaluation timed out; denying request by default".to_string(),
                )
            })?
            .map_err(|status| {
                LlmOsError::PolicyUnavailable(format!(
                    "policy service returned error: {}",
                    status.message()
                ))
            })?
            .into_inner();

        let effect = match rpc_result.effect.as_str() {
            "allow" => PolicyEffect::Allow,
            _ => PolicyEffect::Deny,
        };

        let rule_id = if rpc_result.rule_id.is_empty() {
            None
        } else {
            Some(rpc_result.rule_id)
        };

        Ok(PolicyDecisionRecord {
            version: request.version.clone(),
            effect,
            reason: rpc_result.reason,
            rule_id,
        })
    }
}

pub async fn process_action(
    policy_client: &mut dyn PolicyDecisionClient,
    request: ActionRequest,
    executor: &dyn ActionExecutor,
    audit_sink: &dyn AuditSink,
) -> Result<ActionResult, LlmOsError> {
    let decision_record = match policy_client.evaluate(&request).await {
        Ok(decision) => decision,
        Err(err) => {
            let message = format!("policy unavailable: {}; request denied", err);
            let denied = PolicyDecisionRecord {
                version: request.version.clone(),
                effect: PolicyEffect::Deny,
                reason: message.clone(),
                rule_id: None,
            };
            let result = ActionResult {
                version: request.version.clone(),
                status: ActionStatus::Denied,
                message,
            };
            audit_sink.emit(&build_audit_event(&request, denied, result.status));
            return Err(LlmOsError::ActionDenied(result.message));
        }
    };

    if decision_record.effect == PolicyEffect::Deny {
        let result = ActionResult {
            version: request.version.clone(),
            status: ActionStatus::Denied,
            message: decision_record.reason.clone(),
        };
        audit_sink.emit(&build_audit_event(&request, decision_record, result.status));
        return Err(LlmOsError::ActionDenied(result.message));
    }

    let execution_result = executor.execute(&request)?;
    audit_sink.emit(&build_audit_event(
        &request,
        decision_record,
        execution_result.status,
    ));
    Ok(execution_result)
}

fn build_audit_event(
    request: &ActionRequest,
    decision: PolicyDecisionRecord,
    outcome: ActionStatus,
) -> AuditEvent {
    AuditEvent {
        version: request.version.clone(),
        request_id: request.request_id.clone(),
        correlation_id: request.correlation_id.clone(),
        timestamp_unix_ms: now_unix_millis(),
        subject: request.subject.clone(),
        action: request.action.clone(),
        resource: request.resource.clone(),
        decision,
        outcome,
    }
}

fn now_unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
    };

    use common_types::{
        ActionRequest, ActionStatus, AuditEvent, LlmOsError, PolicyDecisionRecord, PolicyEffect,
    };

    use crate::{
        now_unix_millis, process_action, ActionExecutor, AuditSink, JsonlFileAuditSink,
        PolicyDecisionClient,
    };

    struct TestExecutor {
        calls: AtomicUsize,
    }

    impl TestExecutor {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
            }
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl ActionExecutor for TestExecutor {
        fn execute(
            &self,
            request: &ActionRequest,
        ) -> Result<common_types::ActionResult, LlmOsError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(common_types::ActionResult {
                version: request.version.clone(),
                status: ActionStatus::Executed,
                message: "ok".to_string(),
            })
        }
    }

    #[derive(Default)]
    struct RecordingAuditSink {
        events: Mutex<Vec<AuditEvent>>,
    }

    impl AuditSink for RecordingAuditSink {
        fn emit(&self, event: &AuditEvent) {
            self.events
                .lock()
                .expect("audit lock poisoned")
                .push(event.clone());
        }
    }

    impl RecordingAuditSink {
        fn last_event(&self) -> AuditEvent {
            self.events
                .lock()
                .expect("audit lock poisoned")
                .last()
                .expect("expected at least one event")
                .clone()
        }
    }

    struct FakePolicyClient {
        decision: Option<PolicyDecisionRecord>,
        error: Option<LlmOsError>,
    }

    #[async_trait::async_trait]
    impl PolicyDecisionClient for FakePolicyClient {
        async fn evaluate(
            &mut self,
            _request: &ActionRequest,
        ) -> Result<PolicyDecisionRecord, LlmOsError> {
            if let Some(err) = &self.error {
                return Err(match err {
                    LlmOsError::PolicyUnavailable(msg) => {
                        LlmOsError::PolicyUnavailable(msg.clone())
                    }
                    LlmOsError::ActionDenied(msg) => LlmOsError::ActionDenied(msg.clone()),
                    LlmOsError::ModuleNotFound(msg) => LlmOsError::ModuleNotFound(msg.clone()),
                });
            }

            self.decision
                .clone()
                .ok_or_else(|| LlmOsError::PolicyUnavailable("missing fake decision".to_string()))
        }
    }

    fn request() -> ActionRequest {
        ActionRequest {
            version: "v1".to_string(),
            request_id: "req-123".to_string(),
            correlation_id: "corr-123".to_string(),
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
        }
    }

    #[tokio::test]
    async fn allow_path_executes_action_and_emits_audit() {
        let mut policy = FakePolicyClient {
            decision: Some(PolicyDecisionRecord {
                version: "v1".to_string(),
                effect: PolicyEffect::Allow,
                reason: "allowed by matching rule".to_string(),
                rule_id: Some("allow-network".to_string()),
            }),
            error: None,
        };
        let executor = TestExecutor::new();
        let audit = RecordingAuditSink::default();

        let result = process_action(&mut policy, request(), &executor, &audit)
            .await
            .expect("action should be allowed");

        assert_eq!(result.status, ActionStatus::Executed);
        assert_eq!(executor.calls(), 1);
        let event = audit.last_event();
        assert_eq!(event.request_id, "req-123");
        assert_eq!(event.correlation_id, "corr-123");
        assert_eq!(event.decision.effect, PolicyEffect::Allow);
        assert_eq!(event.outcome, ActionStatus::Executed);
    }

    #[tokio::test]
    async fn explicit_deny_does_not_execute_action() {
        let mut policy = FakePolicyClient {
            decision: Some(PolicyDecisionRecord {
                version: "v1".to_string(),
                effect: PolicyEffect::Deny,
                reason: "denied by matching rule".to_string(),
                rule_id: Some("deny-network".to_string()),
            }),
            error: None,
        };
        let executor = TestExecutor::new();
        let audit = RecordingAuditSink::default();

        let err = process_action(&mut policy, request(), &executor, &audit)
            .await
            .expect_err("action should be denied");

        assert!(matches!(err, LlmOsError::ActionDenied(_)));
        assert_eq!(executor.calls(), 0);
        let event = audit.last_event();
        assert_eq!(event.decision.effect, PolicyEffect::Deny);
        assert_eq!(event.outcome, ActionStatus::Denied);
    }

    #[tokio::test]
    async fn policy_error_fails_closed_and_does_not_execute_action() {
        let mut policy = FakePolicyClient {
            decision: None,
            error: Some(LlmOsError::PolicyUnavailable(
                "connection refused".to_string(),
            )),
        };
        let executor = TestExecutor::new();
        let audit = RecordingAuditSink::default();

        let err = process_action(&mut policy, request(), &executor, &audit)
            .await
            .expect_err("policy error should deny");

        assert!(matches!(err, LlmOsError::ActionDenied(_)));
        assert_eq!(executor.calls(), 0);
        let event = audit.last_event();
        assert_eq!(event.decision.effect, PolicyEffect::Deny);
        assert_eq!(event.decision.rule_id, None);
        assert_eq!(event.outcome, ActionStatus::Denied);
    }

    #[test]
    fn jsonl_audit_sink_writes_event() {
        let unique = format!(
            "llmos_audit_test_{}_{}.jsonl",
            std::process::id(),
            now_unix_millis()
        );
        let path: PathBuf = std::env::temp_dir().join(unique);

        let sink = JsonlFileAuditSink::new(&path).expect("failed to create jsonl sink");
        let event = AuditEvent {
            version: "v1".to_string(),
            request_id: "req-xyz".to_string(),
            correlation_id: "corr-xyz".to_string(),
            timestamp_unix_ms: now_unix_millis(),
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
            decision: PolicyDecisionRecord {
                version: "v1".to_string(),
                effect: PolicyEffect::Allow,
                reason: "allowed by matching rule".to_string(),
                rule_id: Some("allow-network".to_string()),
            },
            outcome: ActionStatus::Executed,
        };

        sink.emit(&event);

        let content = std::fs::read_to_string(&path).expect("failed to read audit jsonl");
        let line = content.lines().next().expect("expected jsonl line");
        let parsed: AuditEvent =
            serde_json::from_str(line).expect("failed to parse jsonl event line");

        assert_eq!(parsed.request_id, "req-xyz");
        assert_eq!(parsed.correlation_id, "corr-xyz");
        assert_eq!(parsed.outcome, ActionStatus::Executed);

        let _ = std::fs::remove_file(path);
    }
}
