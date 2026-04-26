use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};

use hyper::{
    body::Body,
    service::{make_service_fn, service_fn},
    Method, Response, Server, StatusCode,
};

const LATENCY_BUCKETS_US: [u64; 10] = [
    100, 250, 500, 1_000, 2_500, 5_000, 10_000, 25_000, 50_000, 100_000,
];

static POLICY_METRICS: OnceLock<Arc<PolicyEngineMetrics>> = OnceLock::new();

pub fn init_policy_metrics(metrics: Arc<PolicyEngineMetrics>) {
    let _ = POLICY_METRICS.set(metrics);
}

pub fn policy_metrics_handle() -> Arc<PolicyEngineMetrics> {
    POLICY_METRICS
        .get_or_init(|| Arc::new(PolicyEngineMetrics::default()))
        .clone()
}

/// Try to get the metrics handle without initializing a default.
/// Returns None if metrics have not been initialized yet.
pub fn try_policy_metrics_handle() -> Option<Arc<PolicyEngineMetrics>> {
    POLICY_METRICS.get().cloned()
}

#[derive(Debug)]
pub struct PolicyEngineMetrics {
    evaluate_requests_total: AtomicU64,
    evaluate_allow_total: AtomicU64,
    evaluate_deny_total: AtomicU64,
    health_checks_total: AtomicU64,
    rules_loaded: AtomicU64,
    evaluate_latency_sum_us: AtomicU64,
    evaluate_latency_count: AtomicU64,
    evaluate_latency_bucket_counts: [AtomicU64; LATENCY_BUCKETS_US.len()],
}

impl Default for PolicyEngineMetrics {
    fn default() -> Self {
        Self {
            evaluate_requests_total: AtomicU64::new(0),
            evaluate_allow_total: AtomicU64::new(0),
            evaluate_deny_total: AtomicU64::new(0),
            health_checks_total: AtomicU64::new(0),
            rules_loaded: AtomicU64::new(0),
            evaluate_latency_sum_us: AtomicU64::new(0),
            evaluate_latency_count: AtomicU64::new(0),
            evaluate_latency_bucket_counts: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl PolicyEngineMetrics {
    pub fn set_rules_loaded(&self, count: usize) {
        self.rules_loaded.store(count as u64, Ordering::Relaxed);
    }

    pub fn inc_evaluate_requests(&self) {
        self.evaluate_requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_allow(&self) {
        self.evaluate_allow_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_deny(&self) {
        self.evaluate_deny_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_health_checks(&self) {
        self.health_checks_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn observe_evaluate_latency(&self, elapsed: Duration) {
        let elapsed_us = elapsed.as_micros().min(u64::MAX as u128) as u64;
        self.evaluate_latency_sum_us
            .fetch_add(elapsed_us, Ordering::Relaxed);
        self.evaluate_latency_count.fetch_add(1, Ordering::Relaxed);

        for (idx, boundary) in LATENCY_BUCKETS_US.iter().enumerate() {
            if elapsed_us <= *boundary {
                self.evaluate_latency_bucket_counts[idx].fetch_add(1, Ordering::Relaxed);
                return;
            }
        }
    }

    pub fn render_prometheus(&self) -> String {
        let mut out = String::new();
        out.push_str("# TYPE llmos_policy_engine_evaluate_requests_total counter\n");
        out.push_str(&format!(
            "llmos_policy_engine_evaluate_requests_total {}\n",
            self.evaluate_requests_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_engine_allow_total counter\n");
        out.push_str(&format!(
            "llmos_policy_engine_allow_total {}\n",
            self.evaluate_allow_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_engine_deny_total counter\n");
        out.push_str(&format!(
            "llmos_policy_engine_deny_total {}\n",
            self.evaluate_deny_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_engine_health_checks_total counter\n");
        out.push_str(&format!(
            "llmos_policy_engine_health_checks_total {}\n",
            self.health_checks_total.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_engine_rules_loaded gauge\n");
        out.push_str(&format!(
            "llmos_policy_engine_rules_loaded {}\n",
            self.rules_loaded.load(Ordering::Relaxed)
        ));
        out.push_str("# TYPE llmos_policy_engine_evaluate_latency_us histogram\n");

        let mut cumulative = 0u64;
        for (idx, boundary) in LATENCY_BUCKETS_US.iter().enumerate() {
            cumulative = cumulative
                .saturating_add(self.evaluate_latency_bucket_counts[idx].load(Ordering::Relaxed));
            out.push_str(&format!(
                "llmos_policy_engine_evaluate_latency_us_bucket{{le=\"{}\"}} {}\n",
                boundary, cumulative
            ));
        }
        out.push_str(&format!(
            "llmos_policy_engine_evaluate_latency_us_bucket{{le=\"+Inf\"}} {}\n",
            self.evaluate_latency_count.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "llmos_policy_engine_evaluate_latency_us_sum {}\n",
            self.evaluate_latency_sum_us.load(Ordering::Relaxed)
        ));
        out.push_str(&format!(
            "llmos_policy_engine_evaluate_latency_us_count {}\n",
            self.evaluate_latency_count.load(Ordering::Relaxed)
        ));

        out
    }
}

pub async fn run_metrics_server(
    listen_addr: SocketAddr,
    metrics: Arc<PolicyEngineMetrics>,
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

#[cfg(test)]
mod tests {
    use super::PolicyEngineMetrics;
    use std::time::Duration;

    #[test]
    fn render_prometheus_contains_expected_fields() {
        let metrics = PolicyEngineMetrics::default();
        metrics.set_rules_loaded(3);
        metrics.inc_evaluate_requests();
        metrics.inc_allow();
        metrics.inc_deny();
        metrics.inc_health_checks();
        metrics.observe_evaluate_latency(Duration::from_micros(1200));

        let rendered = metrics.render_prometheus();
        assert!(rendered.contains("llmos_policy_engine_evaluate_requests_total 1"));
        assert!(rendered.contains("llmos_policy_engine_allow_total 1"));
        assert!(rendered.contains("llmos_policy_engine_deny_total 1"));
        assert!(rendered.contains("llmos_policy_engine_health_checks_total 1"));
        assert!(rendered.contains("llmos_policy_engine_rules_loaded 3"));
        assert!(rendered.contains("llmos_policy_engine_evaluate_latency_us_count 1"));
    }
}
