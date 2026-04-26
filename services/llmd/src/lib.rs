use std::{
    fs,
    fs::OpenOptions,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use async_trait::async_trait;
use common_types::{
    ActionRequest, ActionResult, ActionStatus, AuditEvent, LlmOsError, PolicyDecisionRecord,
    PolicyEffect,
};
use controlplane_api::{policy_service_client::PolicyServiceClient, EvaluatePolicyRequest};
use hyper::{
    body::Body,
    service::{make_service_fn, service_fn},
    Method, Response, Server, StatusCode,
};
use tonic::{metadata::MetadataValue, transport::Channel, Request};
use tracing::{info, warn};

pub mod bus;
pub mod executor;
pub mod secrets;

pub use executor::ModelExecutor;

#[async_trait]
pub trait PolicyDecisionClient {
    async fn evaluate(
        &mut self,
        request: &ActionRequest,
    ) -> Result<PolicyDecisionRecord, LlmOsError>;
}

pub trait ActionExecutor {
    fn execute(&self, request: &ActionRequest) -> Result<ActionResult, LlmOsError>;
}

pub trait AuditSink {
    fn emit(&self, event: &AuditEvent);
}

const LATENCY_BUCKETS_MS: [u64; 10] = [5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000];

static RUNTIME_METRICS: OnceLock<Arc<RuntimeMetrics>> = OnceLock::new();

pub fn init_runtime_metrics(metrics: Arc<RuntimeMetrics>) {
    let _ = RUNTIME_METRICS.set(metrics);
}

pub fn runtime_metrics_handle() -> Arc<RuntimeMetrics> {
    RUNTIME_METRICS
        .get_or_init(|| Arc::new(RuntimeMetrics::default()))
        .clone()
}

#[derive(Debug)]
pub struct RuntimeMetrics {
    policy_requests_total: AtomicU64,
    policy_retries_total: AtomicU64,
    policy_denies_total: AtomicU64,
    policy_allows_total: AtomicU64,
    policy_breaker_open_total: AtomicU64,
    policy_breaker_open_state: AtomicU64,
    policy_latency_sum_ms: AtomicU64,
    policy_latency_count: AtomicU64,
    policy_latency_bucket_counts: [AtomicU64; LATENCY_BUCKETS_MS.len()],
    audit_write_failures_total: AtomicU64,
    audit_last_write_failed: AtomicU64,
}

impl Default for RuntimeMetrics {
    fn default() -> Self {
        Self {
            policy_requests_total: AtomicU64::new(0),
            policy_retries_total: AtomicU64::new(0),
            policy_denies_total: AtomicU64::new(0),
            policy_allows_total: AtomicU64::new(0),
            policy_breaker_open_total: AtomicU64::new(0),
            policy_breaker_open_state: AtomicU64::new(0),
            policy_latency_sum_ms: AtomicU64::new(0),
            policy_latency_count: AtomicU64::new(0),
            policy_latency_bucket_counts: std::array::from_fn(|_| AtomicU64::new(0)),
            audit_write_failures_total: AtomicU64::new(0),
            audit_last_write_failed: AtomicU64::new(0),
        }
    }
}

impl RuntimeMetrics {
    fn inc_policy_requests(&self) {
        self.policy_requests_total.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_policy_retries(&self) {
        self.policy_retries_total.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_policy_denies(&self) {
        self.policy_denies_total.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_policy_allows(&self) {
        self.policy_allows_total.fetch_add(1, Ordering::Relaxed);
    }

    fn observe_policy_latency(&self, elapsed: Duration) {
        let elapsed_ms = elapsed.as_millis().min(u64::MAX as u128) as u64;
        self.policy_latency_sum_ms
            .fetch_add(elapsed_ms, Ordering::Relaxed);
        self.policy_latency_count.fetch_add(1, Ordering::Relaxed);

        for (idx, boundary) in LATENCY_BUCKETS_MS.iter().enumerate() {
            if elapsed_ms <= *boundary {
                self.policy_latency_bucket_counts[idx].fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    }

    fn set_breaker_open(&self, open: bool) {
        self.policy_breaker_open_state
            .store(if open { 1 } else { 0 }, Ordering::Relaxed);
    }

    fn inc_breaker_open_total(&self) {
        self.policy_breaker_open_total
            .fetch_add(1, Ordering::Relaxed);
    }

    fn mark_audit_write_failure(&self) {
        self.audit_write_failures_total
            .fetch_add(1, Ordering::Relaxed);
        self.audit_last_write_failed.store(1, Ordering::Relaxed);
    }

    fn mark_audit_write_success(&self) {
        self.audit_last_write_failed.store(0, Ordering::Relaxed);
    }

    fn render_prometheus(&self) -> String {
        let mut out = String::new();
        out.push_str("# TYPE llmos_policy_requests_total counter\n");
        out.push_str(&format!(
            "llmos_policy_requests_total {}\n",
            self.policy_requests_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_retries_total counter\n");
        out.push_str(&format!(
            "llmos_policy_retries_total {}\n",
            self.policy_retries_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_denies_total counter\n");
        out.push_str(&format!(
            "llmos_policy_denies_total {}\n",
            self.policy_denies_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_allows_total counter\n");
        out.push_str(&format!(
            "llmos_policy_allows_total {}\n",
            self.policy_allows_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_breaker_open_total counter\n");
        out.push_str(&format!(
            "llmos_policy_breaker_open_total {}\n",
            self.policy_breaker_open_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_breaker_open gauge\n");
        out.push_str(&format!(
            "llmos_policy_breaker_open {}\n",
            self.policy_breaker_open_state.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_audit_queue_depth gauge\n");
        out.push_str("llmos_audit_queue_depth 0\n");
        out.push_str("# TYPE llmos_audit_write_failures_total counter\n");
        out.push_str(&format!(
            "llmos_audit_write_failures_total {}\n",
            self.audit_write_failures_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_audit_last_write_failed gauge\n");
        out.push_str(&format!(
            "llmos_audit_last_write_failed {}\n",
            self.audit_last_write_failed.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_latency_ms histogram\n");

        let mut cumulative = 0u64;
        for (idx, boundary) in LATENCY_BUCKETS_MS.iter().enumerate() {
            cumulative = cumulative
                .saturating_add(self.policy_latency_bucket_counts[idx].load(Ordering::Relaxed));
            out.push_str(&format!(
                "llmos_policy_latency_ms_bucket{{le=\"{}\"}} {}\n",
                boundary, cumulative
            ));
        }
        out.push_str(&format!(
            "llmos_policy_latency_ms_bucket{{le=\"+Inf\"}} {}\n",
            self.policy_latency_count.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "llmos_policy_latency_ms_sum {}\n",
            self.policy_latency_sum_ms.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "llmos_policy_latency_ms_count {}\n",
            self.policy_latency_count.load(Ordering::Relaxed)
        ));

        out
    }
}

pub async fn run_metrics_server(
    listen_addr: SocketAddr,
    metrics: Arc<RuntimeMetrics>,
) -> anyhow::Result<()> {
    let make_service = make_service_fn(move |_| {
        let metrics = metrics.clone();
        async move {
            Ok::<_, std::convert::Infallible>(service_fn(move |request| {
                let metrics = metrics.clone();
                async move {
                    if request.method() == Method::GET && request.uri().path() == "/metrics" {
                        Ok::<_, std::convert::Infallible>(Response::new(Body::from(
                            metrics.render_prometheus(),
                        )))
                    } else {
                        let mut response = Response::new(Body::from("not found"));
                        *response.status_mut() = StatusCode::NOT_FOUND;
                        Ok(response)
                    }
                }
            }))
        }
    });

    Server::bind(&listen_addr).serve(make_service).await?;
    Ok(())
}

pub struct StdoutAuditSink;

impl AuditSink for StdoutAuditSink {
    fn emit(&self, event: &AuditEvent) {
        info!(target: "llmd::audit", event = ?event, "audit event");
    }
}

pub struct JsonlFileAuditSink {
    path: PathBuf,
    max_bytes: u64,
    max_files: usize,
    state: Mutex<JsonlAuditState>,
}

struct JsonlAuditState {
    file: Option<std::fs::File>,
    bytes_written: u64,
}

impl JsonlFileAuditSink {
    pub fn new(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        Self::new_with_rotation(path, 10 * 1024 * 1024, 5)
    }

    pub fn new_with_rotation(
        path: impl AsRef<Path>,
        max_bytes: u64,
        max_files: usize,
    ) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let bytes_written = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            path: path.to_path_buf(),
            max_bytes,
            max_files: max_files.max(1),
            state: Mutex::new(JsonlAuditState {
                file: Some(file),
                bytes_written,
            }),
        })
    }

    fn rotate_files(&self, state: &mut JsonlAuditState) -> anyhow::Result<()> {
        if let Some(mut file) = state.file.take() {
            let _ = file.flush();
            drop(file);
        }

        for idx in (1..=self.max_files).rev() {
            let src = if idx == 1 {
                self.path.clone()
            } else {
                rotated_path(&self.path, idx - 1)
            };
            let dst = rotated_path(&self.path, idx);

            if !src.exists() {
                continue;
            }

            if dst.exists() {
                fs::remove_file(&dst)?;
            }
            fs::rename(src, dst)?;
        }

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;
        state.file = Some(file);
        state.bytes_written = 0;
        Ok(())
    }
}

impl AuditSink for JsonlFileAuditSink {
    fn emit(&self, event: &AuditEvent) {
        let metrics = runtime_metrics_handle();
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                warn!(target: "llmd::audit", "failed to acquire audit file lock");
                metrics.mark_audit_write_failure();
                return;
            }
        };

        let mut payload = match serde_json::to_vec(event) {
            Ok(payload) => payload,
            Err(err) => {
                warn!(target: "llmd::audit", error = %err, "failed to serialize audit event");
                metrics.mark_audit_write_failure();
                return;
            }
        };
        payload.push(b'\n');

        if state.bytes_written + payload.len() as u64 > self.max_bytes {
            if let Err(err) = self.rotate_files(&mut state) {
                warn!(target: "llmd::audit", error = %err, "failed to rotate audit log files");
                metrics.mark_audit_write_failure();
                return;
            }
        }

        if state.file.is_none() {
            match OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
            {
                Ok(file) => {
                    state.bytes_written = file.metadata().map(|m| m.len()).unwrap_or(0);
                    state.file = Some(file);
                }
                Err(err) => {
                    warn!(target: "llmd::audit", error = %err, "failed to open audit log file");
                    metrics.mark_audit_write_failure();
                    return;
                }
            }
        }

        {
            let file = state.file.as_mut().expect("file must be present");
            if let Err(err) = file.write_all(&payload) {
                warn!(target: "llmd::audit", error = %err, "failed to write audit event");
                metrics.mark_audit_write_failure();
                return;
            }
            if let Err(err) = file.flush() {
                warn!(target: "llmd::audit", error = %err, "failed to flush audit log file");
                metrics.mark_audit_write_failure();
                return;
            }
        }
        state.bytes_written = state.bytes_written.saturating_add(payload.len() as u64);
        metrics.mark_audit_write_success();
    }
}

fn rotated_path(path: &Path, index: usize) -> PathBuf {
    let display = path.as_os_str().to_string_lossy();
    PathBuf::from(format!("{display}.{index}"))
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

pub struct GrpcPolicyDecisionClient {
    inner: PolicyServiceClient<Channel>,
    timeout_per_attempt: Duration,
    max_attempts: usize,
    initial_backoff: Duration,
    max_backoff: Duration,
    circuit_breaker_threshold: u32,
    circuit_breaker_cooldown: Duration,
    consecutive_failures: u32,
    circuit_open_until: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct GrpcPolicyClientConfig {
    pub timeout_per_attempt: Duration,
    pub max_attempts: usize,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub circuit_breaker_threshold: u32,
    pub circuit_breaker_cooldown: Duration,
}

impl Default for GrpcPolicyClientConfig {
    fn default() -> Self {
        Self {
            timeout_per_attempt: Duration::from_secs(2),
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(1),
            circuit_breaker_threshold: 3,
            circuit_breaker_cooldown: Duration::from_secs(5),
        }
    }
}

impl GrpcPolicyDecisionClient {
    pub async fn connect(endpoint: String, timeout: Duration) -> anyhow::Result<Self> {
        let config = GrpcPolicyClientConfig {
            timeout_per_attempt: timeout,
            ..GrpcPolicyClientConfig::default()
        };
        Self::connect_with_config(endpoint, config).await
    }

    pub async fn connect_with_config(
        endpoint: String,
        config: GrpcPolicyClientConfig,
    ) -> anyhow::Result<Self> {
        let inner = PolicyServiceClient::connect(endpoint).await?;
        Ok(Self {
            inner,
            timeout_per_attempt: config.timeout_per_attempt,
            max_attempts: config.max_attempts.max(1),
            initial_backoff: config.initial_backoff,
            max_backoff: config.max_backoff,
            circuit_breaker_threshold: config.circuit_breaker_threshold.max(1),
            circuit_breaker_cooldown: config.circuit_breaker_cooldown,
            consecutive_failures: 0,
            circuit_open_until: None,
        })
    }
}

#[async_trait]
impl PolicyDecisionClient for GrpcPolicyDecisionClient {
    async fn evaluate(
        &mut self,
        request: &ActionRequest,
    ) -> Result<PolicyDecisionRecord, LlmOsError> {
        let metrics = runtime_metrics_handle();
        metrics.inc_policy_requests();
        let started = Instant::now();
        self.fail_if_circuit_open()?;

        for attempt in 1..=self.max_attempts {
            let grpc_request = build_grpc_request(request)?;
            let call_result =
                tokio::time::timeout(self.timeout_per_attempt, self.inner.evaluate(grpc_request))
                    .await;

            match call_result {
                Ok(Ok(response)) => {
                    self.mark_success();
                    metrics.observe_policy_latency(started.elapsed());
                    return Ok(map_grpc_decision(request, response.into_inner()));
                }
                Ok(Err(status)) => {
                    let retryable = is_retryable_status(&status);
                    let message = format!(
                        "policy service returned {}: {}",
                        status.code(),
                        status.message()
                    );

                    if retryable && attempt < self.max_attempts {
                        metrics.inc_policy_retries();
                        tokio::time::sleep(backoff_for_attempt(
                            attempt,
                            self.initial_backoff,
                            self.max_backoff,
                        ))
                        .await;
                        continue;
                    }

                    self.mark_failure();
                    metrics.observe_policy_latency(started.elapsed());
                    return Err(LlmOsError::PolicyUnavailable(message));
                }
                Err(_) => {
                    if attempt < self.max_attempts {
                        metrics.inc_policy_retries();
                        tokio::time::sleep(backoff_for_attempt(
                            attempt,
                            self.initial_backoff,
                            self.max_backoff,
                        ))
                        .await;
                        continue;
                    }

                    self.mark_failure();
                    metrics.observe_policy_latency(started.elapsed());
                    return Err(LlmOsError::PolicyUnavailable(
                        "policy evaluation timed out; denying request by default".to_string(),
                    ));
                }
            }
        }

        self.mark_failure();
        metrics.observe_policy_latency(started.elapsed());
        Err(LlmOsError::PolicyUnavailable(
            "policy service failed after retries".to_string(),
        ))
    }
}

fn build_grpc_request(
    request: &ActionRequest,
) -> Result<Request<EvaluatePolicyRequest>, LlmOsError> {
    let mut grpc_request = Request::new(EvaluatePolicyRequest {
        subject: request.subject.clone(),
        action: request.action.clone(),
        resource: request.resource.clone(),
    });

    let request_id = MetadataValue::try_from(request.request_id.as_str()).map_err(|_| {
        LlmOsError::PolicyUnavailable("request_id contains invalid metadata characters".to_string())
    })?;
    let correlation_id =
        MetadataValue::try_from(request.correlation_id.as_str()).map_err(|_| {
            LlmOsError::PolicyUnavailable(
                "correlation_id contains invalid metadata characters".to_string(),
            )
        })?;

    grpc_request
        .metadata_mut()
        .insert("x-request-id", request_id);
    grpc_request
        .metadata_mut()
        .insert("x-correlation-id", correlation_id);
    Ok(grpc_request)
}

fn map_grpc_decision(
    request: &ActionRequest,
    response: controlplane_api::EvaluatePolicyResponse,
) -> PolicyDecisionRecord {
    let effect = match response.effect.as_str() {
        "allow" => PolicyEffect::Allow,
        _ => PolicyEffect::Deny,
    };

    let rule_id = if response.rule_id.is_empty() {
        None
    } else {
        Some(response.rule_id)
    };

    PolicyDecisionRecord {
        version: request.version.clone(),
        effect,
        reason: response.reason,
        rule_id,
    }
}

fn is_retryable_status(status: &tonic::Status) -> bool {
    matches!(
        status.code(),
        tonic::Code::Unavailable | tonic::Code::DeadlineExceeded
    )
}

fn backoff_for_attempt(attempt: usize, initial: Duration, max: Duration) -> Duration {
    let multiplier = 1u32
        .checked_shl(attempt.saturating_sub(1).min(30) as u32)
        .unwrap_or(u32::MAX);
    let raw_ms = initial
        .as_millis()
        .saturating_mul(multiplier as u128)
        .min(max.as_millis());
    Duration::from_millis(raw_ms as u64)
}

impl GrpcPolicyDecisionClient {
    fn fail_if_circuit_open(&mut self) -> Result<(), LlmOsError> {
        let metrics = runtime_metrics_handle();
        if let Some(until) = self.circuit_open_until {
            if Instant::now() < until {
                metrics.set_breaker_open(true);
                return Err(LlmOsError::PolicyUnavailable(
                    "policy circuit breaker open; failing closed".to_string(),
                ));
            }
            self.circuit_open_until = None;
            metrics.set_breaker_open(false);
        }
        Ok(())
    }

    fn mark_success(&mut self) {
        self.consecutive_failures = 0;
        self.circuit_open_until = None;
        runtime_metrics_handle().set_breaker_open(false);
    }

    fn mark_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= self.circuit_breaker_threshold {
            self.circuit_open_until = Some(Instant::now() + self.circuit_breaker_cooldown);
            let metrics = runtime_metrics_handle();
            metrics.set_breaker_open(true);
            metrics.inc_breaker_open_total();
        }
    }
}

pub async fn process_action(
    policy_client: &mut dyn PolicyDecisionClient,
    request: ActionRequest,
    executor: &dyn ActionExecutor,
    audit_sink: &dyn AuditSink,
) -> Result<ActionResult, LlmOsError> {
    let metrics = runtime_metrics_handle();
    let decision_record = match policy_client.evaluate(&request).await {
        Ok(decision) => decision,
        Err(err) => {
            let message = format!("policy unavailable: {}; request denied", err);
            let denied = PolicyDecisionRecord {
                version: request.version.clone(),
                effect: PolicyEffect::Deny,
                reason: message.clone(),
                rule_id: None,
            };
            let result = ActionResult {
                version: request.version.clone(),
                status: ActionStatus::Denied,
                message,
            };
            audit_sink.emit(&build_audit_event(&request, denied, result.status));
            metrics.inc_policy_denies();
            return Err(LlmOsError::ActionDenied(result.message));
        }
    };

    if decision_record.effect == PolicyEffect::Deny {
        let result = ActionResult {
            version: request.version.clone(),
            status: ActionStatus::Denied,
            message: decision_record.reason.clone(),
        };
        audit_sink.emit(&build_audit_event(&request, decision_record, result.status));
        metrics.inc_policy_denies();
        return Err(LlmOsError::ActionDenied(result.message));
    }

    let execution_result = executor.execute(&request)?;
    metrics.inc_policy_allows();
    audit_sink.emit(&build_audit_event(
        &request,
        decision_record,
        execution_result.status,
    ));
    Ok(execution_result)
}

fn build_audit_event(
    request: &ActionRequest,
    decision: PolicyDecisionRecord,
    outcome: ActionStatus,
) -> AuditEvent {
    AuditEvent {
        version: request.version.clone(),
        request_id: request.request_id.clone(),
        correlation_id: request.correlation_id.clone(),
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
    use std::{
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
        time::Duration,
    };

    use common_types::{
        ActionRequest, ActionStatus, AuditEvent, LlmOsError, PolicyDecisionRecord, PolicyEffect,
    };

    use crate::{
        now_unix_millis, process_action, ActionExecutor, AuditSink, JsonlFileAuditSink,
        PolicyDecisionClient, RuntimeMetrics,
    };

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

    struct FakePolicyClient {
        decision: Option<PolicyDecisionRecord>,
        error: Option<LlmOsError>,
    }

    #[async_trait::async_trait]
    impl PolicyDecisionClient for FakePolicyClient {
        async fn evaluate(
            &mut self,
            _request: &ActionRequest,
        ) -> Result<PolicyDecisionRecord, LlmOsError> {
            if let Some(err) = &self.error {
                return Err(match err {
                    LlmOsError::PolicyUnavailable(msg) => {
                        LlmOsError::PolicyUnavailable(msg.clone())
                    }
                    LlmOsError::ActionDenied(msg) => LlmOsError::ActionDenied(msg.clone()),
                    LlmOsError::ModuleNotFound(msg) => LlmOsError::ModuleNotFound(msg.clone()),
                });
            }

            self.decision
                .clone()
                .ok_or_else(|| LlmOsError::PolicyUnavailable("missing fake decision".to_string()))
        }
    }

    fn request() -> ActionRequest {
        ActionRequest {
            version: "v1".to_string(),
            request_id: "req-123".to_string(),
            correlation_id: "corr-123".to_string(),
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
        }
    }

    #[tokio::test]
    async fn allow_path_executes_action_and_emits_audit() {
        let mut policy = FakePolicyClient {
            decision: Some(PolicyDecisionRecord {
                version: "v1".to_string(),
                effect: PolicyEffect::Allow,
                reason: "allowed by matching rule".to_string(),
                rule_id: Some("allow-network".to_string()),
            }),
            error: None,
        };
        let executor = TestExecutor::new();
        let audit = RecordingAuditSink::default();

        let result = process_action(&mut policy, request(), &executor, &audit)
            .await
            .expect("action should be allowed");

        assert_eq!(result.status, ActionStatus::Executed);
        assert_eq!(executor.calls(), 1);
        let event = audit.last_event();
        assert_eq!(event.request_id, "req-123");
        assert_eq!(event.correlation_id, "corr-123");
        assert_eq!(event.decision.effect, PolicyEffect::Allow);
        assert_eq!(event.outcome, ActionStatus::Executed);
    }

    #[tokio::test]
    async fn explicit_deny_does_not_execute_action() {
        let mut policy = FakePolicyClient {
            decision: Some(PolicyDecisionRecord {
                version: "v1".to_string(),
                effect: PolicyEffect::Deny,
                reason: "denied by matching rule".to_string(),
                rule_id: Some("deny-network".to_string()),
            }),
            error: None,
        };
        let executor = TestExecutor::new();
        let audit = RecordingAuditSink::default();

        let err = process_action(&mut policy, request(), &executor, &audit)
            .await
            .expect_err("action should be denied");

        assert!(matches!(err, LlmOsError::ActionDenied(_)));
        assert_eq!(executor.calls(), 0);
        let event = audit.last_event();
        assert_eq!(event.decision.effect, PolicyEffect::Deny);
        assert_eq!(event.outcome, ActionStatus::Denied);
    }

    #[tokio::test]
    async fn policy_error_fails_closed_and_does_not_execute_action() {
        let mut policy = FakePolicyClient {
            decision: None,
            error: Some(LlmOsError::PolicyUnavailable(
                "connection refused".to_string(),
            )),
        };
        let executor = TestExecutor::new();
        let audit = RecordingAuditSink::default();

        let err = process_action(&mut policy, request(), &executor, &audit)
            .await
            .expect_err("policy error should deny");

        assert!(matches!(err, LlmOsError::ActionDenied(_)));
        assert_eq!(executor.calls(), 0);
        let event = audit.last_event();
        assert_eq!(event.decision.effect, PolicyEffect::Deny);
        assert_eq!(event.decision.rule_id, None);
        assert_eq!(event.outcome, ActionStatus::Denied);
    }

    #[test]
    fn jsonl_audit_sink_writes_event() {
        let unique = format!(
            "llmos_audit_test_{}_{}.jsonl",
            std::process::id(),
            now_unix_millis()
        );
        let path: PathBuf = std::env::temp_dir().join(unique);

        let sink = JsonlFileAuditSink::new(&path).expect("failed to create jsonl sink");
        let event = AuditEvent {
            version: "v1".to_string(),
            request_id: "req-xyz".to_string(),
            correlation_id: "corr-xyz".to_string(),
            timestamp_unix_ms: now_unix_millis(),
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
            decision: PolicyDecisionRecord {
                version: "v1".to_string(),
                effect: PolicyEffect::Allow,
                reason: "allowed by matching rule".to_string(),
                rule_id: Some("allow-network".to_string()),
            },
            outcome: ActionStatus::Executed,
        };

        sink.emit(&event);

        let content = std::fs::read_to_string(&path).expect("failed to read audit jsonl");
        let line = content.lines().next().expect("expected jsonl line");
        let parsed: AuditEvent =
            serde_json::from_str(line).expect("failed to parse jsonl event line");

        assert_eq!(parsed.request_id, "req-xyz");
        assert_eq!(parsed.correlation_id, "corr-xyz");
        assert_eq!(parsed.outcome, ActionStatus::Executed);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn jsonl_audit_sink_rotates_by_size() {
        let unique = format!(
            "llmos_audit_rotate_test_{}_{}.jsonl",
            std::process::id(),
            now_unix_millis()
        );
        let path: PathBuf = std::env::temp_dir().join(unique);
        let rotated_1 = PathBuf::from(format!("{}.1", path.display()));
        let rotated_2 = PathBuf::from(format!("{}.2", path.display()));

        let sink = JsonlFileAuditSink::new_with_rotation(&path, 220, 2)
            .expect("failed to create rotating jsonl sink");

        for idx in 0..6 {
            let event = AuditEvent {
                version: "v1".to_string(),
                request_id: format!("req-rotate-{idx}"),
                correlation_id: "corr-rotate".to_string(),
                timestamp_unix_ms: now_unix_millis(),
                subject: "runtime/model-runtime".to_string(),
                action: "network:connect".to_string(),
                resource: "api.openai.com".to_string(),
                decision: PolicyDecisionRecord {
                    version: "v1".to_string(),
                    effect: PolicyEffect::Allow,
                    reason: "allowed by matching rule".to_string(),
                    rule_id: Some("allow-network".to_string()),
                },
                outcome: ActionStatus::Executed,
            };
            sink.emit(&event);
        }

        assert!(path.exists(), "expected active audit file");
        assert!(rotated_1.exists(), "expected first rotated file");
        assert!(rotated_2.exists(), "expected second rotated file");

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(rotated_1);
        let _ = std::fs::remove_file(rotated_2);
    }

    #[test]
    fn metrics_render_includes_policy_and_audit_fields() {
        let metrics = RuntimeMetrics::default();
        metrics.inc_policy_requests();
        metrics.inc_policy_retries();
        metrics.inc_policy_denies();
        metrics.inc_policy_allows();
        metrics.observe_policy_latency(Duration::from_millis(42));
        metrics.inc_breaker_open_total();
        metrics.set_breaker_open(true);
        metrics.mark_audit_write_failure();

        let rendered = metrics.render_prometheus();

        assert!(rendered.contains("llmos_policy_requests_total 1"));
        assert!(rendered.contains("llmos_policy_retries_total 1"));
        assert!(rendered.contains("llmos_policy_denies_total 1"));
        assert!(rendered.contains("llmos_policy_allows_total 1"));
        assert!(rendered.contains("llmos_policy_breaker_open 1"));
        assert!(rendered.contains("llmos_audit_write_failures_total 1"));
        assert!(rendered.contains("llmos_policy_latency_ms_count 1"));
    }
}
