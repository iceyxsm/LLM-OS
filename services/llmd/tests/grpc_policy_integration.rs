use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use common_types::{ActionRequest, ActionStatus, LlmOsError};
use controlplane_api::{
    policy_service_server::{PolicyService, PolicyServiceServer},
    EvaluatePolicyRequest, EvaluatePolicyResponse,
};
use llmd::{
    process_action, ActionExecutor, GrpcPolicyDecisionClient, NoopExecutor, StdoutAuditSink,
};
use policy_engine::{
    grpc::PolicyGrpcService,
    model::{PolicyDocument, PolicyRule, RuleEffect},
};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{transport::Server, Request, Response, Status};

#[tokio::test]
async fn grpc_allow_executes_action() {
    let policy = PolicyDocument {
        version: "v1".to_string(),
        rules: vec![PolicyRule {
            id: "allow-network".to_string(),
            effect: RuleEffect::Allow,
            subject: "runtime/model-runtime".to_string(),
            actions: vec!["network:connect".to_string()],
            resources: vec!["api.openai.com".to_string()],
        }],
    };
    let (endpoint, shutdown) = spawn_policy_server(policy).await;

    let mut client = GrpcPolicyDecisionClient::connect(endpoint, std::time::Duration::from_secs(2))
        .await
        .expect("failed to connect policy client");
    let audit = StdoutAuditSink;
    let executor = CountingExecutor::new();

    let result = process_action(&mut client, test_request(), &executor, &audit)
        .await
        .expect("allow request should execute");

    assert_eq!(result.status, ActionStatus::Executed);
    assert_eq!(executor.calls(), 1);

    let _ = shutdown.send(());
}

#[tokio::test]
async fn grpc_explicit_deny_blocks_execution() {
    let policy = PolicyDocument {
        version: "v1".to_string(),
        rules: vec![PolicyRule {
            id: "deny-network".to_string(),
            effect: RuleEffect::Deny,
            subject: "runtime/model-runtime".to_string(),
            actions: vec!["network:connect".to_string()],
            resources: vec!["api.openai.com".to_string()],
        }],
    };
    let (endpoint, shutdown) = spawn_policy_server(policy).await;

    let mut client = GrpcPolicyDecisionClient::connect(endpoint, std::time::Duration::from_secs(2))
        .await
        .expect("failed to connect policy client");
    let audit = StdoutAuditSink;
    let executor = CountingExecutor::new();

    let err = process_action(&mut client, test_request(), &executor, &audit)
        .await
        .expect_err("deny rule should block request");

    assert!(matches!(err, LlmOsError::ActionDenied(_)));
    assert_eq!(executor.calls(), 0);

    let _ = shutdown.send(());
}

#[tokio::test]
async fn grpc_no_match_denies_by_default() {
    let policy = PolicyDocument {
        version: "v1".to_string(),
        rules: vec![PolicyRule {
            id: "allow-filesystem".to_string(),
            effect: RuleEffect::Allow,
            subject: "runtime/mcp-runtime".to_string(),
            actions: vec!["fs:write".to_string()],
            resources: vec!["/tmp/*".to_string()],
        }],
    };
    let (endpoint, shutdown) = spawn_policy_server(policy).await;

    let mut client = GrpcPolicyDecisionClient::connect(endpoint, std::time::Duration::from_secs(2))
        .await
        .expect("failed to connect policy client");
    let audit = StdoutAuditSink;
    let executor = CountingExecutor::new();

    let err = process_action(&mut client, test_request(), &executor, &audit)
        .await
        .expect_err("no-match should deny by default");

    assert!(matches!(err, LlmOsError::ActionDenied(_)));
    assert_eq!(executor.calls(), 0);

    let _ = shutdown.send(());
}

#[tokio::test]
async fn grpc_propagates_request_and_correlation_ids_in_metadata() {
    let captured = Arc::new(Mutex::new(None));
    let (endpoint, shutdown) = spawn_capture_server(captured.clone()).await;

    let mut client = GrpcPolicyDecisionClient::connect(endpoint, std::time::Duration::from_secs(2))
        .await
        .expect("failed to connect policy client");
    let audit = StdoutAuditSink;
    let executor = CountingExecutor::new();

    let request = ActionRequest {
        version: "v1".to_string(),
        request_id: "req-meta-1".to_string(),
        correlation_id: "corr-meta-1".to_string(),
        subject: "runtime/model-runtime".to_string(),
        action: "network:connect".to_string(),
        resource: "api.openai.com".to_string(),
    };

    let result = process_action(&mut client, request, &executor, &audit)
        .await
        .expect("capture service returns allow");

    assert_eq!(result.status, ActionStatus::Executed);

    let values = captured.lock().expect("capture lock poisoned").clone();
    assert_eq!(
        values,
        Some(("req-meta-1".to_string(), "corr-meta-1".to_string()))
    );

    let _ = shutdown.send(());
}

fn test_request() -> ActionRequest {
    ActionRequest {
        version: "v1".to_string(),
        request_id: "req-test-1".to_string(),
        correlation_id: "corr-test-1".to_string(),
        subject: "runtime/model-runtime".to_string(),
        action: "network:connect".to_string(),
        resource: "api.openai.com".to_string(),
    }
}

async fn spawn_policy_server(policy: PolicyDocument) -> (String, tokio::sync::oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind policy test listener");
    let addr = listener
        .local_addr()
        .expect("failed to read policy test listener addr");

    let service = PolicyGrpcService::new(Arc::new(policy));
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        let _ = Server::builder()
            .add_service(PolicyServiceServer::new(service))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    (format!("http://{}", addr), shutdown_tx)
}

async fn spawn_capture_server(
    captured: Arc<Mutex<Option<(String, String)>>>,
) -> (String, tokio::sync::oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind capture test listener");
    let addr = listener
        .local_addr()
        .expect("failed to read capture test listener addr");

    let service = MetadataCaptureService { captured };
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        let _ = Server::builder()
            .add_service(PolicyServiceServer::new(service))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    (format!("http://{}", addr), shutdown_tx)
}

#[derive(Clone)]
struct MetadataCaptureService {
    captured: Arc<Mutex<Option<(String, String)>>>,
}

#[tonic::async_trait]
impl PolicyService for MetadataCaptureService {
    async fn evaluate(
        &self,
        request: Request<EvaluatePolicyRequest>,
    ) -> Result<Response<EvaluatePolicyResponse>, Status> {
        let request_id = request
            .metadata()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let correlation_id = request
            .metadata()
            .get("x-correlation-id")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();

        *self.captured.lock().expect("capture lock poisoned") = Some((request_id, correlation_id));

        Ok(Response::new(EvaluatePolicyResponse {
            effect: "allow".to_string(),
            reason: "allowed by capture service".to_string(),
            rule_id: "capture-allow".to_string(),
        }))
    }
}

struct CountingExecutor {
    calls: AtomicUsize,
}

impl CountingExecutor {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ActionExecutor for CountingExecutor {
    fn execute(&self, request: &ActionRequest) -> Result<common_types::ActionResult, LlmOsError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let noop = NoopExecutor;
        noop.execute(request)
    }
}
