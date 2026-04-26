use std::sync::{Arc, Mutex};

use common_types::{ActionRequest, ActionStatus, AuditEvent, LlmOsError};
use controlplane_api::{
    health_service_client::HealthServiceClient,
    health_service_server::HealthServiceServer,
    policy_service_server::PolicyServiceServer,
    HealthCheckRequest,
};
use llmd::{
    process_action, GrpcPolicyDecisionClient, NoopExecutor, AuditSink,
};
use policy_engine::{
    grpc::{HealthGrpcService, PolicyGrpcService},
    model::{PolicyDocument, PolicyRule, RuleEffect},
};
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

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
    fn event_count(&self) -> usize {
        self.events.lock().expect("audit lock poisoned").len()
    }

    fn events(&self) -> Vec<AuditEvent> {
        self.events.lock().expect("audit lock poisoned").clone()
    }
}

fn example_policy() -> PolicyDocument {
    PolicyDocument {
        version: "v1".to_string(),
        rules: vec![
            PolicyRule {
                id: "allow-model-api".to_string(),
                effect: RuleEffect::Allow,
                subject: "runtime/model-runtime".to_string(),
                actions: vec!["network:connect".to_string()],
                resources: vec![
                    "api.openai.com".to_string(),
                    "api.anthropic.com".to_string(),
                ],
            },
            PolicyRule {
                id: "deny-host-fs".to_string(),
                effect: RuleEffect::Deny,
                subject: "runtime/mcp-runtime".to_string(),
                actions: vec!["fs:write".to_string()],
                resources: vec!["/".to_string()],
            },
        ],
    }
}

fn make_request(subject: &str, action: &str, resource: &str) -> ActionRequest {
    ActionRequest {
        version: "v1".to_string(),
        request_id: format!("req-e2e-{}", subject.replace('/', "-")),
        correlation_id: "corr-e2e-1".to_string(),
        subject: subject.to_string(),
        action: action.to_string(),
        resource: resource.to_string(),
    }
}

#[tokio::test]
async fn e2e_full_pipeline_with_health_and_policy() {
    let policy = example_policy();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind");
    let addr = listener.local_addr().expect("failed to get addr");
    let endpoint = format!("http://{}", addr);

    let policy_service = PolicyGrpcService::new(Arc::new(policy));
    let health_service = HealthGrpcService;
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    tokio::spawn(async move {
        let _ = Server::builder()
            .add_service(PolicyServiceServer::new(policy_service))
            .add_service(HealthServiceServer::new(health_service))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    // Health check
    let mut health_client = HealthServiceClient::connect(endpoint.clone())
        .await
        .expect("failed to connect health client");
    let health_response = health_client
        .check(tonic::Request::new(HealthCheckRequest {
            service: "policy-engine".to_string(),
        }))
        .await
        .expect("health check failed")
        .into_inner();
    assert_eq!(health_response.status, "SERVING");

    // Policy client
    let mut policy_client =
        GrpcPolicyDecisionClient::connect(endpoint, std::time::Duration::from_secs(2))
            .await
            .expect("failed to connect policy client");
    let executor = NoopExecutor;
    let audit = RecordingAuditSink::default();

    // Allowed: model runtime connecting to API
    let result = process_action(
        &mut policy_client,
        make_request("runtime/model-runtime", "network:connect", "api.openai.com"),
        &executor,
        &audit,
    )
    .await
    .expect("should be allowed");
    assert_eq!(result.status, ActionStatus::Executed);

    // Denied by explicit rule: mcp-runtime writing to /
    let err = process_action(
        &mut policy_client,
        make_request("runtime/mcp-runtime", "fs:write", "/"),
        &executor,
        &audit,
    )
    .await
    .expect_err("should be denied by rule");
    assert!(matches!(err, LlmOsError::ActionDenied(_)));

    // Denied by default: no matching rule
    let err = process_action(
        &mut policy_client,
        make_request("security/audit", "fs:read", "/etc/passwd"),
        &executor,
        &audit,
    )
    .await
    .expect_err("should be denied by default");
    assert!(matches!(err, LlmOsError::ActionDenied(_)));

    // Verify audit events were emitted for all three actions
    assert_eq!(audit.event_count(), 3);
    let events = audit.events();
    assert_eq!(events[0].outcome, ActionStatus::Executed);
    assert_eq!(events[1].outcome, ActionStatus::Denied);
    assert_eq!(events[2].outcome, ActionStatus::Denied);

    // All events share the same correlation id
    assert!(events.iter().all(|e| e.correlation_id == "corr-e2e-1"));

    let _ = shutdown_tx.send(());
}
