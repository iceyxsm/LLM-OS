use std::sync::Arc;

use controlplane_api::{
    policy_service_server::PolicyService, EvaluatePolicyRequest, EvaluatePolicyResponse,
};
use tonic::{Request, Response, Status};

use crate::{
    engine::evaluate_policy,
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
        let request = request.into_inner();
        let input = PolicyRequest {
            subject: request.subject,
            action: request.action,
            resource: request.resource,
        };
        let decision = evaluate_policy(&self.policy, &input);
        let (effect, reason, rule_id) = map_decision_fields(&decision);

        Ok(Response::new(EvaluatePolicyResponse {
            effect,
            reason,
            rule_id,
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
