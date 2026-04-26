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
            reason: "contract test".to_string(),
            rule_id: "contract.rule".to_string(),
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
async fn all_json_commands_emit_valid_json() {
    let (endpoint, shutdown_tx) = spawn_controlplane_server().await;

    let modules = run_cli(["modules", "--output", "json", "--timeout-secs", "1"]);
    assert!(modules.status.success(), "modules command failed");
    let modules_value: serde_json::Value =
        serde_json::from_str(&modules.stdout).expect("modules should return valid json");
    assert!(
        modules_value.is_array(),
        "modules output should be a json array"
    );

    let policy_check = run_cli([
        "policy".to_string(),
        "check".to_string(),
        "--subject".to_string(),
        "runtime/model-runtime".to_string(),
        "--action".to_string(),
        "network:connect".to_string(),
        "--resource".to_string(),
        "api.openai.com".to_string(),
        "--endpoint".to_string(),
        endpoint.clone(),
        "--timeout-secs".to_string(),
        "1".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]);
    assert!(policy_check.status.success(), "policy check command failed");
    let policy_check_value: serde_json::Value =
        serde_json::from_str(&policy_check.stdout).expect("policy check should return valid json");
    assert!(
        policy_check_value.is_object(),
        "policy check output should be a json object"
    );

    let policy_health = run_cli([
        "policy".to_string(),
        "health".to_string(),
        "--endpoint".to_string(),
        endpoint.clone(),
        "--timeout-secs".to_string(),
        "1".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ]);
    assert!(
        policy_health.status.success(),
        "policy health command failed"
    );
    let policy_health_value: serde_json::Value = serde_json::from_str(&policy_health.stdout)
        .expect("policy health should return valid json");
    assert!(
        policy_health_value.is_object(),
        "policy health output should be a json object"
    );

    let _ = shutdown_tx.send(());
}

struct CliRunResult {
    status: std::process::ExitStatus,
    stdout: String,
}

fn run_cli(args: impl IntoIterator<Item = impl AsRef<str>>) -> CliRunResult {
    let output = Command::new(env!("CARGO_BIN_EXE_llmos-cli"))
        .args(args.into_iter().map(|arg| arg.as_ref().to_string()))
        .output()
        .expect("failed to run llmos-cli");

    if !output.status.success() {
        panic!(
            "command failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    CliRunResult {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
    }
}

async fn spawn_controlplane_server() -> (String, tokio::sync::oneshot::Sender<()>) {
    let addr = format!("127.0.0.1:{}", unused_local_port())
        .parse()
        .expect("valid socket address");
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = TonicServer::builder()
            .add_service(PolicyServiceServer::new(TestPolicyService))
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
