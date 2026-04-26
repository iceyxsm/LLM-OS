use std::fmt;

use serde::{Deserialize, Serialize};

/// A structured workload identifier following the `<namespace>/<name>` convention
/// used throughout LLM-OS (e.g. `runtime/model-runtime`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkloadId {
    namespace: String,
    name: String,
}

impl WorkloadId {
    /// Parse a workload id from a `namespace/name` string.
    pub fn parse(raw: &str) -> Result<Self, WorkloadIdError> {
        let (namespace, name) = raw
            .split_once('/')
            .ok_or_else(|| WorkloadIdError::InvalidFormat(raw.to_string()))?;

        if namespace.is_empty() || name.is_empty() {
            return Err(WorkloadIdError::InvalidFormat(raw.to_string()));
        }

        if !is_valid_segment(namespace) || !is_valid_segment(name) {
            return Err(WorkloadIdError::InvalidCharacters(raw.to_string()));
        }

        Ok(Self {
            namespace: namespace.to_string(),
            name: name.to_string(),
        })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for WorkloadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.namespace, self.name)
    }
}

fn is_valid_segment(s: &str) -> bool {
    s.chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
}

/// The full identity record for a workload, including its id and the set of
/// capabilities it is allowed to claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadIdentity {
    pub id: WorkloadId,
    pub capabilities: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkloadIdError {
    #[error("invalid workload id format (expected namespace/name): {0}")]
    InvalidFormat(String),
    #[error("workload id contains invalid characters: {0}")]
    InvalidCharacters(String),
}
