use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};

use tracing::{info, warn};

use crate::loader::load_policy_document;
use crate::model::PolicyDocument;

/// A shared policy holder that supports atomic swaps for hot-reload.
///
/// Readers acquire a read lock and get an Arc to the current policy.
/// The reload task acquires a write lock to swap the inner Arc.
#[derive(Clone)]
pub struct SharedPolicy {
    inner: Arc<RwLock<Arc<PolicyDocument>>>,
}

impl SharedPolicy {
    pub fn new(policy: PolicyDocument) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Arc::new(policy))),
        }
    }

    /// Create from an existing Arc, for backward compatibility with code
    /// that already has an Arc<PolicyDocument>.
    pub fn from_arc(policy: Arc<PolicyDocument>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(policy)),
        }
    }

    /// Get the current policy document.
    pub fn current(&self) -> Arc<PolicyDocument> {
        self.inner
            .read()
            .expect("shared policy lock poisoned")
            .clone()
    }

    /// Replace the current policy with a new one.
    pub fn swap(&self, policy: PolicyDocument) {
        let mut guard = self.inner.write().expect("shared policy lock poisoned");
        *guard = Arc::new(policy);
    }
}

/// Poll a policy file for changes and reload when the modification time changes.
///
/// This runs in a loop and should be spawned as a background task.
pub async fn poll_and_reload(path: PathBuf, shared: SharedPolicy, interval: Duration) {
    let mut last_modified = file_modified_time(&path);

    loop {
        tokio::time::sleep(interval).await;

        let current_modified = file_modified_time(&path);
        if current_modified == last_modified {
            continue;
        }

        info!(
            target: "policy-engine::reload",
            path = %path.display(),
            "policy file change detected, reloading"
        );

        match load_policy_document(&path) {
            Ok(policy) => {
                let rules = policy.rules.len();
                let version = policy.version.clone();
                shared.swap(policy);

                if let Some(metrics) = crate::metrics::try_policy_metrics_handle() {
                    metrics.set_rules_loaded(rules);
                }

                last_modified = current_modified;
                info!(
                    target: "policy-engine::reload",
                    version = %version,
                    rules = rules,
                    "policy reloaded"
                );
            }
            Err(err) => {
                warn!(
                    target: "policy-engine::reload",
                    error = %err,
                    "failed to reload policy; keeping current version"
                );
            }
        }
    }
}

fn file_modified_time(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}
