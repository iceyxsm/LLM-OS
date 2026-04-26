use std::process::Command;

use controlplane_api::{
    health_service_server::{HealthService, HealthServiceServer},
    policy_service_server::{PolicyService, PolicyServiceServer},
    EvaluatePolicyRequest, EvaluatePolicyResponse, HealthCheckRequest, HealthCheckResponse,
};
use tonic::{transport::Server as TonicServer, Request, Response, Status};

struct TestPolicyService;

#[tonic::async_trait]
impl PolicyService for TestPolicyService {
    async fn evaluate(
        &self,
        _request: Request<EvaluatePolicyRequest>,
    ) -> Result<Response<EvaluatePolicyResponse>, Status> {
        Ok(Response::new(EvaluatePolicyResponse {
            effect: "allow".to_string(),
            reason: "matched test rule".to_string(),
            rule_id: "test.rule.allow".to_string(),
        }))
    }
}

struct TestHealthService;

#[tonic::async_trait]
impl HealthService for TestHealthService {
    async fn check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Ok(Response::new(HealthCheckResponse {
            status: "SERVING".to_string(),
            detail: "ok".to_string(),
        }))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn policy_check_json_output_matches_contract() {
    let (endpoint, tx) = spawn_policy_server().await;
    let output = Command::new(env!("CARGO_BIN_EXE_llmos-cli"))
        .arg("policy")
        .arg("check")
        .arg("--subject")
        .arg("runtime/model-runtime")
        .arg("--action")
        .arg("network:connect")
        .arg("--resource")
        .arg("api.openai.com")
        .arg("--endpoint")
        .arg(&endpoint)
        .arg("--timeout-secs")
        .arg("1")
        .arg("--output")
        .arg("json")
        .output()
        .expect("failed to run llmos-cli policy check");
    let _ = tx.send(());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "policy check command failed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );

    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("policy check should emit valid JSON");
    let object = value
        .as_object()
        .expect("policy check output should be a JSON object");
    assert!(
        object.get("decision").and_then(|v| v.as_str()).is_some(),
        "missing string field 'decision': {}",
        value
    );
    assert!(
        object.get("reason").and_then(|v| v.as_str()).is_some(),
        "missing string field 'reason': {}",
        value
    );
    assert!(
        object.get("request_id").and_then(|v| v.as_str()).is_some(),
        "missing string field 'request_id': {}",
        value
    );
    assert!(
        object
            .get("correlation_id")
            .and_then(|v| v.as_str())
            .is_some(),
        "missing string field 'correlation_id': {}",
        value
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn policy_health_json_output_matches_contract() {
    let (endpoint, tx) = spawn_health_server().await;
    let output = Command::new(env!("CARGO_BIN_EXE_llmos-cli"))
        .arg("policy")
        .arg("health")
        .arg("--endpoint")
        .arg(&endpoint)
        .arg("--timeout-secs")
        .arg("1")
        .arg("--output")
        .arg("json")
        .output()
        .expect("failed to run llmos-cli policy health");
    let _ = tx.send(());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "policy health command failed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );

    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("policy health should emit valid JSON");
    let object = value
        .as_object()
        .expect("policy health output should be a JSON object");
    assert!(
        object.get("status").and_then(|v| v.as_str()).is_some(),
        "missing string field 'status': {}",
        value
    );
    assert!(
        object.get("detail").and_then(|v| v.as_str()).is_some(),
        "missing string field 'detail': {}",
        value
    );
    assert!(
        object.get("latency_ms").and_then(|v| v.as_u64()).is_some(),
        "missing numeric field 'latency_ms': {}",
        value
    );
    assert!(
        object.get("request_id").and_then(|v| v.as_str()).is_some(),
        "missing string field 'request_id': {}",
        value
    );
    assert!(
        object
            .get("correlation_id")
            .and_then(|v| v.as_str())
            .is_some(),
        "missing string field 'correlation_id': {}",
        value
    );
}

#[test]
fn policy_check_unreachable_endpoint_fails_with_stable_error_shape() {
    let endpoint = format!("http://127.0.0.1:{}", unused_local_port());
    let output = Command::new(env!("CARGO_BIN_EXE_llmos-cli"))
        .arg("policy")
        .arg("check")
        .arg("--subject")
        .arg("runtime/model-runtime")
        .arg("--action")
        .arg("network:connect")
        .arg("--resource")
        .arg("api.openai.com")
        .arg("--endpoint")
        .arg(&endpoint)
        .arg("--timeout-secs")
        .arg("1")
        .arg("--output")
        .arg("json")
        .output()
        .expect("failed to run llmos-cli policy check");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected command to fail");
    assert!(
        stderr.contains("Error:"),
        "expected stderr to include Rust main error prefix, got:\n{}",
        stderr
    );
}

#[test]
fn policy_health_unreachable_endpoint_fails_with_stable_error_shape() {
    let endpoint = format!("http://127.0.0.1:{}", unused_local_port());
    let output = Command::new(env!("CARGO_BIN_EXE_llmos-cli"))
        .arg("policy")
        .arg("health")
        .arg("--endpoint")
        .arg(&endpoint)
        .arg("--timeout-secs")
        .arg("1")
        .arg("--output")
        .arg("json")
        .output()
        .expect("failed to run llmos-cli policy health");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "expected command to fail");
    assert!(
        stderr.contains("Error:"),
        "expected stderr to include Rust main error prefix, got:\n{}",
        stderr
    );
}

async fn spawn_policy_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let addr = format!("127.0.0.1:{}", unused_local_port())
        .parse()
        .expect("valid socket address");
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = TonicServer::builder()
            .add_service(PolicyServiceServer::new(TestPolicyService))
            .serve_with_shutdown(addr, async move {
                let _ = rx.await;
            })
            .await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    (format!("http://{}", addr), tx)
}

async fn spawn_health_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let addr = format!("127.0.0.1:{}", unused_local_port())
        .parse()
        .expect("valid socket address");
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = TonicServer::builder()
            .add_service(HealthServiceServer::new(TestHealthService))
            .serve_with_shutdown(addr, async move {
                let _ = rx.await;
            })
            .await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    (format!("http://{}", addr), tx)
}

fn unused_local_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind should succeed")
        .local_addr()
        .expect("local addr should exist")
        .port()
}
