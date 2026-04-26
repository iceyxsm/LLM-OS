use std::{path::PathBuf, time::Duration};

use crate::output::{render_json, OutputFormat};
use clap::Args;
use common_types::ModuleDescriptor;
use controlplane_api::{health_service_client::HealthServiceClient, HealthCheckRequest};
use hyper::{body::to_bytes, Client, Uri};
use mcp_runtime::load_manifests;
use tonic::metadata::MetadataValue;

#[derive(Args, Debug)]
pub struct ModulesArgs {
    #[arg(long, default_value = "http://127.0.0.1:9090/metrics")]
    llmd_metrics_endpoint: String,
    #[arg(long, default_value = "http://127.0.0.1:50051")]
    policy_endpoint: String,
    #[arg(long, default_value = "sdk/plugin-api/manifests")]
    mcp_manifest_dir: PathBuf,
    #[arg(long, default_value_t = 2)]
    timeout_secs: u64,
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    output: OutputFormat,
}

pub async fn run_modules(args: ModulesArgs) -> anyhow::Result<()> {
    let modules = collect_modules(&args).await;
    render_modules(&modules, args.output)?;
    Ok(())
}

async fn collect_modules(args: &ModulesArgs) -> Vec<ModuleDescriptor> {
    let timeout = Duration::from_secs(args.timeout_secs);
    let llmd = probe_llmd_metrics(&args.llmd_metrics_endpoint, timeout).await;
    let policy = probe_policy_health(&args.policy_endpoint, timeout).await;
    let mcp = probe_mcp_runtime(&args.mcp_manifest_dir);

    vec![
        ModuleDescriptor {
            id: "services/llmd".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            status: llmd,
        },
        ModuleDescriptor {
            id: "services/mcp-runtime".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            status: mcp,
        },
        ModuleDescriptor {
            id: "services/policy-engine".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            status: policy,
        },
    ]
}

fn render_modules(modules: &[ModuleDescriptor], output: OutputFormat) -> anyhow::Result<()> {
    match output {
        OutputFormat::Text => {
            for m in modules {
                println!("{}@{} [{}]", m.id, m.version, m.status);
            }
        }
        OutputFormat::Json => {
            let json = modules_as_json(modules)?;
            println!("{}", json);
        }
    }
    Ok(())
}

fn modules_as_json(modules: &[ModuleDescriptor]) -> anyhow::Result<String> {
    render_json(modules)
}

async fn probe_llmd_metrics(endpoint: &str, timeout: Duration) -> String {
    let uri: Uri = match endpoint.parse() {
        Ok(uri) => uri,
        Err(err) => return format!("invalid-endpoint({})", err),
    };
    let client = Client::new();
    let req = client.get(uri);
    let response = match tokio::time::timeout(timeout, req).await {
        Ok(Ok(response)) => response,
        Ok(Err(err)) => return format!("unreachable({})", err),
        Err(_) => return "timeout".to_string(),
    };

    if !response.status().is_success() {
        return format!("unhealthy(http:{})", response.status());
    }

    let body_bytes = match to_bytes(response.into_body()).await {
        Ok(bytes) => bytes,
        Err(err) => return format!("unhealthy(body-read:{})", err),
    };
    let body = String::from_utf8_lossy(&body_bytes);
    if body.contains("llmos_policy_requests_total") {
        "healthy".to_string()
    } else {
        "unhealthy(metrics-missing)".to_string()
    }
}

async fn probe_policy_health(endpoint: &str, timeout: Duration) -> String {
    match fetch_policy_health(endpoint, timeout, "policy-engine").await {
        Ok((status, _, _, _, _)) if status.eq_ignore_ascii_case("SERVING") => "healthy".to_string(),
        Ok((status, detail, _, _, _)) => format!("unhealthy(status:{} detail:{})", status, detail),
        Err(err) => format!("unreachable({})", err),
    }
}

fn probe_mcp_runtime(manifest_dir: &PathBuf) -> String {
    match load_manifests(manifest_dir) {
        Ok(manifests) if manifests.is_empty() => "no-manifests".to_string(),
        Ok(manifests) => format!("configured(manifests:{})", manifests.len()),
        Err(err) => format!("invalid-manifests({})", err),
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
        modules_as_json, probe_llmd_metrics, probe_mcp_runtime, probe_policy_health,
        ModuleDescriptor,
    };
    use controlplane_api::{
        health_service_server::{HealthService, HealthServiceServer},
        HealthCheckRequest, HealthCheckResponse,
    };
    use hyper::{
        body::Body,
        service::{make_service_fn, service_fn},
        Method, Response, Server, StatusCode,
    };
    use std::{
        net::SocketAddr,
        path::PathBuf,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tonic::{transport::Server as TonicServer, Request, Response as TonicResponse, Status};

    struct TestHealthService {
        status: String,
        detail: String,
        delay: Duration,
    }

    #[tonic::async_trait]
    impl HealthService for TestHealthService {
        async fn check(
            &self,
            _request: Request<HealthCheckRequest>,
        ) -> Result<TonicResponse<HealthCheckResponse>, Status> {
            if self.delay > Duration::ZERO {
                tokio::time::sleep(self.delay).await;
            }
            Ok(TonicResponse::new(HealthCheckResponse {
                status: self.status.clone(),
                detail: self.detail.clone(),
            }))
        }
    }

    #[tokio::test]
    async fn llmd_probe_reports_healthy_for_expected_metric_payload() {
        let addr =
            spawn_metrics_server(200, "llmos_policy_requests_total 1\n", Duration::ZERO).await;
        let endpoint = format!("http://{}/metrics", addr);
        let status = probe_llmd_metrics(&endpoint, Duration::from_secs(1)).await;
        assert_eq!(status, "healthy");
    }

    #[tokio::test]
    async fn llmd_probe_reports_timeout() {
        let addr = spawn_metrics_server(
            200,
            "llmos_policy_requests_total 1\n",
            Duration::from_millis(100),
        )
        .await;
        let endpoint = format!("http://{}/metrics", addr);
        let status = probe_llmd_metrics(&endpoint, Duration::from_millis(20)).await;
        assert_eq!(status, "timeout");
    }

    #[tokio::test]
    async fn llmd_probe_reports_http_failure() {
        let addr = spawn_metrics_server(503, "unavailable", Duration::ZERO).await;
        let endpoint = format!("http://{}/metrics", addr);
        let status = probe_llmd_metrics(&endpoint, Duration::from_secs(1)).await;
        assert!(status.starts_with("unhealthy(http:503"));
    }

    #[tokio::test]
    async fn llmd_probe_reports_metrics_missing() {
        let addr = spawn_metrics_server(200, "other_metric 7\n", Duration::ZERO).await;
        let endpoint = format!("http://{}/metrics", addr);
        let status = probe_llmd_metrics(&endpoint, Duration::from_secs(1)).await;
        assert_eq!(status, "unhealthy(metrics-missing)");
    }

    #[tokio::test]
    async fn policy_probe_reports_healthy_for_serving_status() {
        let (addr, tx) = spawn_policy_server("SERVING", "ok", Duration::ZERO).await;
        let endpoint = format!("http://{}", addr);
        let status = probe_policy_health(&endpoint, Duration::from_secs(1)).await;
        assert_eq!(status, "healthy");
        let _ = tx.send(());
    }

    #[tokio::test]
    async fn policy_probe_reports_unhealthy_for_non_serving_status() {
        let (addr, tx) = spawn_policy_server("NOT_SERVING", "degraded", Duration::ZERO).await;
        let endpoint = format!("http://{}", addr);
        let status = probe_policy_health(&endpoint, Duration::from_secs(1)).await;
        assert_eq!(status, "unhealthy(status:NOT_SERVING detail:degraded)");
        let _ = tx.send(());
    }

    #[tokio::test]
    async fn policy_probe_reports_unreachable_when_server_missing() {
        let port = unused_local_port();
        let endpoint = format!("http://127.0.0.1:{}", port);
        let status = probe_policy_health(&endpoint, Duration::from_millis(100)).await;
        assert!(status.starts_with("unreachable("));
    }

    #[tokio::test]
    async fn mcp_probe_reports_configured() {
        let dir = temp_test_dir("mcp_probe_configured");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("sample.json"),
            r#"{
              "id":"test.plugin",
              "version":"0.1.0",
              "entrypoint":"echo hi",
              "capabilities":["mcp:spawn"]
            }"#,
        )
        .expect("write manifest");

        let status = probe_mcp_runtime(&dir);
        assert_eq!(status, "configured(manifests:1)");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn mcp_probe_reports_no_manifests() {
        let dir = temp_test_dir("mcp_probe_empty");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let status = probe_mcp_runtime(&dir);
        assert_eq!(status, "no-manifests");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn mcp_probe_reports_invalid_manifests() {
        let dir = temp_test_dir("mcp_probe_invalid");
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(
            dir.join("broken.json"),
            r#"{
              "id":"INVALID.ID",
              "version":"0.1.0",
              "entrypoint":"echo hi",
              "capabilities":["mcp:spawn"]
            }"#,
        )
        .expect("write invalid manifest");

        let status = probe_mcp_runtime(&dir);
        assert!(status.starts_with("invalid-manifests("));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn render_modules_json_includes_descriptor_fields() {
        let modules = vec![ModuleDescriptor {
            id: "services/llmd".to_string(),
            version: "0.1.0".to_string(),
            status: "healthy".to_string(),
        }];
        let json = modules_as_json(&modules).expect("json render should succeed");
        assert!(json.contains("\"id\": \"services/llmd\""));
        assert!(json.contains("\"status\": \"healthy\""));
    }

    async fn spawn_metrics_server(
        status_code: u16,
        body: &'static str,
        delay: Duration,
    ) -> SocketAddr {
        let addr: SocketAddr = format!("127.0.0.1:{}", unused_local_port())
            .parse()
            .expect("valid socket address");
        let make = make_service_fn(move |_| async move {
            Ok::<_, std::convert::Infallible>(service_fn(move |request| async move {
                if request.method() == Method::GET && request.uri().path() == "/metrics" {
                    if delay > Duration::ZERO {
                        tokio::time::sleep(delay).await;
                    }
                    let mut response = Response::new(Body::from(body));
                    *response.status_mut() =
                        StatusCode::from_u16(status_code).expect("valid status code");
                    Ok::<_, std::convert::Infallible>(response)
                } else {
                    let mut response = Response::new(Body::from("not found"));
                    *response.status_mut() = StatusCode::NOT_FOUND;
                    Ok::<_, std::convert::Infallible>(response)
                }
            }))
        });

        tokio::spawn(async move {
            let _ = Server::bind(&addr).serve(make).await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        addr
    }

    async fn spawn_policy_server(
        status: &str,
        detail: &str,
        delay: Duration,
    ) -> (SocketAddr, tokio::sync::oneshot::Sender<()>) {
        let addr: SocketAddr = format!("127.0.0.1:{}", unused_local_port())
            .parse()
            .expect("valid socket address");
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let service = TestHealthService {
            status: status.to_string(),
            detail: detail.to_string(),
            delay,
        };
        tokio::spawn(async move {
            let _ = TonicServer::builder()
                .add_service(HealthServiceServer::new(service))
                .serve_with_shutdown(addr, async move {
                    let _ = rx.await;
                })
                .await;
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        (addr, tx)
    }

    fn unused_local_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .expect("bind should succeed")
            .local_addr()
            .expect("local addr should exist")
            .port()
    }

    fn temp_test_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("llmos_cli_{}_{}", label, nanos))
    }
}
