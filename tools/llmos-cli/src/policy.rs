use std::time::Duration;

use crate::output::{
    render_json, render_lines, OutputFormat, PolicyCheckOutput, PolicyHealthOutput,
};
use clap::{Args, Subcommand};
use controlplane_api::{
    health_service_client::HealthServiceClient, policy_service_client::PolicyServiceClient,
    EvaluatePolicyRequest, HealthCheckRequest,
};
use tonic::metadata::MetadataValue;

#[derive(Subcommand, Debug)]
pub enum PolicyCommand {
    Check(PolicyCheckArgs),
    Health(PolicyHealthArgs),
}

#[derive(Args, Debug)]
pub struct PolicyCheckArgs {
    #[arg(long)]
    subject: String,
    #[arg(long)]
    action: String,
    #[arg(long)]
    resource: String,
    #[arg(long, default_value = "http://127.0.0.1:50051")]
    endpoint: String,
    #[arg(long, default_value_t = 2)]
    timeout_secs: u64,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    output: OutputFormat,
}

#[derive(Args, Debug)]
pub struct PolicyHealthArgs {
    #[arg(long, default_value = "http://127.0.0.1:50051")]
    endpoint: String,
    #[arg(long, default_value_t = 2)]
    timeout_secs: u64,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    output: OutputFormat,
}

pub async fn run_policy(command: PolicyCommand) -> anyhow::Result<()> {
    match command {
        PolicyCommand::Check(args) => run_policy_check(args).await,
        PolicyCommand::Health(args) => run_policy_health(args).await,
    }
}

async fn run_policy_check(args: PolicyCheckArgs) -> anyhow::Result<()> {
    let request_id = generate_id("cli-req");
    let correlation_id = generate_id("cli-corr");
    let mut client = PolicyServiceClient::connect(args.endpoint.clone()).await?;

    let mut request = tonic::Request::new(EvaluatePolicyRequest {
        subject: args.subject,
        action: args.action,
        resource: args.resource,
    });

    request.metadata_mut().insert(
        "x-request-id",
        MetadataValue::try_from(request_id.as_str())?,
    );
    request.metadata_mut().insert(
        "x-correlation-id",
        MetadataValue::try_from(correlation_id.as_str())?,
    );

    let response = tokio::time::timeout(
        Duration::from_secs(args.timeout_secs),
        client.evaluate(request),
    )
    .await??
    .into_inner();

    let result = PolicyCheckOutput {
        decision: response.effect,
        reason: response.reason,
        rule_id: if response.rule_id.is_empty() {
            None
        } else {
            Some(response.rule_id)
        },
        request_id,
        correlation_id,
    };
    println!("{}", render_policy_check(&result, args.output)?);

    Ok(())
}

async fn run_policy_health(args: PolicyHealthArgs) -> anyhow::Result<()> {
    let (status, detail, latency_ms, request_id, correlation_id) = fetch_policy_health(
        &args.endpoint,
        Duration::from_secs(args.timeout_secs),
        "policy-engine",
    )
    .await?;

    let result = PolicyHealthOutput {
        status,
        detail,
        latency_ms,
        request_id,
        correlation_id,
    };
    println!("{}", render_policy_health(&result, args.output)?);
    Ok(())
}

fn render_policy_check(result: &PolicyCheckOutput, output: OutputFormat) -> anyhow::Result<String> {
    match output {
        OutputFormat::Text => {
            let mut lines = Vec::with_capacity(5);
            lines.push(format!("decision: {}", result.decision));
            lines.push(format!("reason: {}", result.reason));
            lines.push(format!(
                "rule_id: {}",
                result.rule_id.as_deref().unwrap_or("<none>")
            ));
            lines.push(format!("request_id: {}", result.request_id));
            lines.push(format!("correlation_id: {}", result.correlation_id));
            Ok(render_lines(&lines))
        }
        OutputFormat::Json => render_json(result),
    }
}

fn render_policy_health(
    result: &PolicyHealthOutput,
    output: OutputFormat,
) -> anyhow::Result<String> {
    match output {
        OutputFormat::Text => {
            let mut lines = Vec::with_capacity(5);
            lines.push(format!("status: {}", result.status));
            lines.push(format!("detail: {}", result.detail));
            lines.push(format!("latency_ms: {}", result.latency_ms));
            lines.push(format!("request_id: {}", result.request_id));
            lines.push(format!("correlation_id: {}", result.correlation_id));
            Ok(render_lines(&lines))
        }
        OutputFormat::Json => render_json(result),
    }
}

async fn fetch_policy_health(
    endpoint: &str,
    timeout: Duration,
    service_name: &str,
) -> anyhow::Result<(String, String, u128, String, String)> {
    let request_id = generate_id("cli-health-req");
    let correlation_id = generate_id("cli-health-corr");
    let mut client = HealthServiceClient::connect(endpoint.to_string()).await?;

    let mut request = tonic::Request::new(HealthCheckRequest {
        service: service_name.to_string(),
    });
    request.metadata_mut().insert(
        "x-request-id",
        MetadataValue::try_from(request_id.as_str())?,
    );
    request.metadata_mut().insert(
        "x-correlation-id",
        MetadataValue::try_from(correlation_id.as_str())?,
    );

    let started = std::time::Instant::now();
    let response = tokio::time::timeout(timeout, client.check(request))
        .await??
        .into_inner();
    let latency_ms = started.elapsed().as_millis();

    Ok((
        response.status,
        response.detail,
        latency_ms,
        request_id,
        correlation_id,
    ))
}

fn generate_id(prefix: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let sequence = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{}-{}", prefix, ts, sequence)
}

#[cfg(test)]
mod tests {
    use super::{
        render_policy_check, render_policy_health, OutputFormat, PolicyCheckOutput,
        PolicyHealthOutput,
    };

    #[test]
    fn render_policy_check_text_handles_missing_rule_id() {
        let result = PolicyCheckOutput {
            decision: "allow".to_string(),
            reason: "matched rule".to_string(),
            rule_id: None,
            request_id: "req-1".to_string(),
            correlation_id: "corr-1".to_string(),
        };
        let rendered =
            render_policy_check(&result, OutputFormat::Text).expect("render should succeed");
        assert!(rendered.contains("decision: allow"));
        assert!(rendered.contains("rule_id: <none>"));
    }

    #[test]
    fn render_policy_check_json_includes_expected_keys() {
        let result = PolicyCheckOutput {
            decision: "deny".to_string(),
            reason: "blocked".to_string(),
            rule_id: Some("rule-1".to_string()),
            request_id: "req-2".to_string(),
            correlation_id: "corr-2".to_string(),
        };
        let rendered =
            render_policy_check(&result, OutputFormat::Json).expect("render should succeed");
        assert!(rendered.contains("\"decision\": \"deny\""));
        assert!(rendered.contains("\"rule_id\": \"rule-1\""));
    }

    #[test]
    fn render_policy_health_json_includes_expected_keys() {
        let result = PolicyHealthOutput {
            status: "SERVING".to_string(),
            detail: "ok".to_string(),
            latency_ms: 10,
            request_id: "req-3".to_string(),
            correlation_id: "corr-3".to_string(),
        };
        let rendered =
            render_policy_health(&result, OutputFormat::Json).expect("render should succeed");
        assert!(rendered.contains("\"status\": \"SERVING\""));
        assert!(rendered.contains("\"latency_ms\": 10"));
    }
}
