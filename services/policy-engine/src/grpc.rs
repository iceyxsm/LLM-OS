use std::sync::Arc;
use std::time::Instant;

use controlplane_api::{
    health_service_server::HealthService, policy_service_server::PolicyService,
    EvaluatePolicyRequest, EvaluatePolicyResponse, HealthCheckRequest, HealthCheckResponse,
};
use tonic::{Request, Response, Status};
use tracing::info;

use crate::{
    engine::evaluate_policy,
    metrics::policy_metrics_handle,
    model::{DecisionEffect, DecisionReason, PolicyDocument, PolicyRequest},
};

#[derive(Clone)]
pub struct PolicyGrpcService {
    policy: Arc<PolicyDocument>,
}

impl PolicyGrpcService {
    pub fn new(policy: Arc<PolicyDocument>) -> Self {
        Self { policy }
    }
}

#[tonic::async_trait]
impl PolicyService for PolicyGrpcService {
    async fn evaluate(
        &self,
        request: Request<EvaluatePolicyRequest>,
    ) -> Result<Response<EvaluatePolicyResponse>, Status> {
        let metrics = policy_metrics_handle();
        metrics.inc_evaluate_requests();
        let started = Instant::now();
        let request_id = metadata_value(&request, "x-request-id");
        let correlation_id = metadata_value(&request, "x-correlation-id");
        let request = request.into_inner();
        let input = PolicyRequest {
            subject: request.subject,
            action: request.action,
            resource: request.resource,
        };
        let decision = evaluate_policy(&self.policy, &input);
        let (effect, reason, rule_id) = map_decision_fields(&decision);
        if effect == "allow" {
            metrics.inc_allow();
        } else {
            metrics.inc_deny();
        }
        metrics.observe_evaluate_latency(started.elapsed());
        info!(
            target: "policy-engine::grpc",
            request_id = request_id.as_deref().unwrap_or("missing"),
            correlation_id = correlation_id.as_deref().unwrap_or("missing"),
            subject = %input.subject,
            action = %input.action,
            resource = %input.resource,
            effect = %effect,
            rule_id = %rule_id,
            "policy request evaluated"
        );

        Ok(Response::new(EvaluatePolicyResponse {
            effect,
            reason,
            rule_id,
        }))
    }
}

#[derive(Clone, Default)]
pub struct HealthGrpcService;

#[tonic::async_trait]
impl HealthService for HealthGrpcService {
    async fn check(
        &self,
        request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let metrics = policy_metrics_handle();
        metrics.inc_health_checks();
        let request_id = metadata_value(&request, "x-request-id");
        let correlation_id = metadata_value(&request, "x-correlation-id");
        let request = request.into_inner();
        let service = if request.service.is_empty() {
            "policy-engine".to_string()
        } else {
            request.service
        };

        info!(
            target: "policy-engine::health",
            request_id = request_id.as_deref().unwrap_or("missing"),
            correlation_id = correlation_id.as_deref().unwrap_or("missing"),
            service = %service,
            "health check responded"
        );

        Ok(Response::new(HealthCheckResponse {
            status: "SERVING".to_string(),
            detail: format!("{service} ready"),
        }))
    }
}

fn map_decision_fields(decision: &crate::model::PolicyDecision) -> (String, String, String) {
    let effect = match decision.effect {
        DecisionEffect::Allow => "allow".to_string(),
        DecisionEffect::Deny => "deny".to_string(),
    };

    match &decision.reason {
        DecisionReason::MatchedAllow { rule_id } => (
            effect,
            "allowed by matching rule".to_string(),
            rule_id.clone(),
        ),
        DecisionReason::MatchedDeny { rule_id } => (
            effect,
            "denied by matching rule".to_string(),
            rule_id.clone(),
        ),
        DecisionReason::NoMatch => (
            effect,
            "denied by default because no matching rule was found".to_string(),
            String::new(),
        ),
    }
}

fn metadata_value<T>(request: &Request<T>, key: &str) -> Option<String> {
    request
        .metadata()
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use tonic::Request;

    use crate::grpc::metadata_value;
    use controlplane_api::EvaluatePolicyRequest;

    #[test]
    fn metadata_value_reads_ascii_header() {
        let mut request = Request::new(EvaluatePolicyRequest {
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
        });
        request
            .metadata_mut()
            .insert("x-request-id", "req-123".parse().expect("valid metadata"));

        let read = metadata_value(&request, "x-request-id");
        assert_eq!(read.as_deref(), Some("req-123"));
    }
}
