use std::path::PathBuf;

use policy_engine::{
    engine::evaluate_policy,
    loader::load_policy_document,
    model::{DecisionEffect, PolicyRequest},
};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .compact()
        .init();

    let policy_path = std::env::var("LLMOS_POLICY_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("config/policy.example.yaml"));
    let policy = load_policy_document(&policy_path)?;

    info!(
        target: "policy-engine",
        path = %policy_path.display(),
        version = %policy.version,
        rules = policy.rules.len(),
        "policy engine online"
    );
    info!(target: "policy-engine", "decision mode: deny overrides allow, default deny");

    let probe_requests = [
        PolicyRequest {
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
        },
        PolicyRequest {
            subject: "runtime/mcp-runtime".to_string(),
            action: "fs:write".to_string(),
            resource: "/".to_string(),
        },
        PolicyRequest {
            subject: "runtime/mcp-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "example.com".to_string(),
        },
    ];

    for request in probe_requests {
        let decision = evaluate_policy(&policy, &request);
        let effect = match decision.effect {
            DecisionEffect::Allow => "allow",
            DecisionEffect::Deny => "deny",
        };

        info!(
            target: "policy-engine",
            subject = %request.subject,
            action = %request.action,
            resource = %request.resource,
            decision = effect,
            reason = ?decision.reason,
            "evaluated policy request"
        );
    }

    tokio::signal::ctrl_c().await?;
    info!(target: "policy-engine", "shutdown");
    Ok(())
}
