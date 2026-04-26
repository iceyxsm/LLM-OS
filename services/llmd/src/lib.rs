use std::time::{SystemTime, UNIX_EPOCH};

use common_types::{
    ActionRequest, ActionResult, ActionStatus, AuditEvent, LlmOsError, PolicyDecisionRecord,
    PolicyEffect,
};
use policy_engine::model::{
    DecisionEffect, DecisionReason, PolicyDecision, PolicyDocument, PolicyRequest,
};
use tracing::info;

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

pub fn process_action(
    policy: &PolicyDocument,
    request: ActionRequest,
    executor: &dyn ActionExecutor,
    audit_sink: &dyn AuditSink,
) -> Result<ActionResult, LlmOsError> {
    let policy_request = PolicyRequest {
        subject: request.subject.clone(),
        action: request.action.clone(),
        resource: request.resource.clone(),
    };
    let policy_decision = policy_engine::engine::evaluate_policy(policy, &policy_request);
    let decision_record = map_decision(&policy_decision);

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

fn map_decision(decision: &PolicyDecision) -> PolicyDecisionRecord {
    match &decision.reason {
        DecisionReason::MatchedAllow { rule_id } => PolicyDecisionRecord {
            version: "v1".to_string(),
            effect: PolicyEffect::Allow,
            reason: "allowed by matching rule".to_string(),
            rule_id: Some(rule_id.clone()),
        },
        DecisionReason::MatchedDeny { rule_id } => PolicyDecisionRecord {
            version: "v1".to_string(),
            effect: PolicyEffect::Deny,
            reason: "denied by matching rule".to_string(),
            rule_id: Some(rule_id.clone()),
        },
        DecisionReason::NoMatch => PolicyDecisionRecord {
            version: "v1".to_string(),
            effect: match decision.effect {
                DecisionEffect::Allow => PolicyEffect::Allow,
                DecisionEffect::Deny => PolicyEffect::Deny,
            },
            reason: "denied by default because no matching rule was found".to_string(),
            rule_id: None,
        },
    }
}

fn build_audit_event(
    request: &ActionRequest,
    decision: PolicyDecisionRecord,
    outcome: ActionStatus,
) -> AuditEvent {
    AuditEvent {
        version: request.version.clone(),
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
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };

    use common_types::{ActionRequest, ActionStatus, AuditEvent, LlmOsError, PolicyEffect};
    use policy_engine::model::{PolicyDocument, PolicyRule, RuleEffect};

    use crate::{process_action, ActionExecutor, AuditSink};

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

    fn build_policy(rules: Vec<PolicyRule>) -> PolicyDocument {
        PolicyDocument {
            version: "v1".to_string(),
            rules,
        }
    }

    fn request() -> ActionRequest {
        ActionRequest {
            version: "v1".to_string(),
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
        }
    }

    #[test]
    fn allow_path_executes_action_and_emits_audit() {
        let policy = build_policy(vec![PolicyRule {
            id: "allow-network".to_string(),
            effect: RuleEffect::Allow,
            subject: "runtime/model-runtime".to_string(),
            actions: vec!["network:connect".to_string()],
            resources: vec!["api.openai.com".to_string()],
        }]);
        let executor = TestExecutor::new();
        let audit = RecordingAuditSink::default();

        let result = process_action(&policy, request(), &executor, &audit)
            .expect("action should be allowed");

        assert_eq!(result.status, ActionStatus::Executed);
        assert_eq!(executor.calls(), 1);
        let event = audit.last_event();
        assert_eq!(event.decision.effect, PolicyEffect::Allow);
        assert_eq!(event.outcome, ActionStatus::Executed);
    }

    #[test]
    fn explicit_deny_does_not_execute_action() {
        let policy = build_policy(vec![PolicyRule {
            id: "deny-network".to_string(),
            effect: RuleEffect::Deny,
            subject: "runtime/model-runtime".to_string(),
            actions: vec!["network:connect".to_string()],
            resources: vec!["api.openai.com".to_string()],
        }]);
        let executor = TestExecutor::new();
        let audit = RecordingAuditSink::default();

        let err = process_action(&policy, request(), &executor, &audit)
            .expect_err("action should be denied");

        assert!(matches!(err, LlmOsError::ActionDenied(_)));
        assert_eq!(executor.calls(), 0);
        let event = audit.last_event();
        assert_eq!(event.decision.effect, PolicyEffect::Deny);
        assert_eq!(event.outcome, ActionStatus::Denied);
    }

    #[test]
    fn no_match_is_denied_and_does_not_execute_action() {
        let policy = build_policy(vec![PolicyRule {
            id: "allow-filesystem".to_string(),
            effect: RuleEffect::Allow,
            subject: "runtime/mcp-runtime".to_string(),
            actions: vec!["fs:write".to_string()],
            resources: vec!["/tmp/*".to_string()],
        }]);
        let executor = TestExecutor::new();
        let audit = RecordingAuditSink::default();

        let err = process_action(&policy, request(), &executor, &audit)
            .expect_err("no-match should deny");

        assert!(matches!(err, LlmOsError::ActionDenied(_)));
        assert_eq!(executor.calls(), 0);
        let event = audit.last_event();
        assert_eq!(event.decision.effect, PolicyEffect::Deny);
        assert_eq!(event.decision.rule_id, None);
        assert_eq!(event.outcome, ActionStatus::Denied);
    }
}
