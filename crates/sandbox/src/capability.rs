use serde::{Deserialize, Serialize};

/// A set of Linux capabilities to grant or deny.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitySet {
    pub allowed: Vec<String>,
    pub denied: Vec<String>,
}

/// Pre-built capability presets.
pub struct CapabilityPreset;

impl CapabilityPreset {
    /// Minimal capabilities for sandboxed plugins.
    ///
    /// No network, no raw I/O, no privilege escalation.
    pub fn minimal() -> CapabilitySet {
        CapabilitySet {
            allowed: vec![],
            denied: vec![
                "CAP_NET_RAW".to_string(),
                "CAP_NET_ADMIN".to_string(),
                "CAP_SYS_ADMIN".to_string(),
                "CAP_SYS_PTRACE".to_string(),
                "CAP_SYS_MODULE".to_string(),
                "CAP_MKNOD".to_string(),
            ],
        }
    }

    /// Capabilities for the model runtime.
    ///
    /// Allows network binding for metrics and API access.
    pub fn model_runtime() -> CapabilitySet {
        CapabilitySet {
            allowed: vec!["CAP_NET_BIND_SERVICE".to_string()],
            denied: vec![
                "CAP_SYS_ADMIN".to_string(),
                "CAP_SYS_PTRACE".to_string(),
                "CAP_SYS_MODULE".to_string(),
                "CAP_MKNOD".to_string(),
            ],
        }
    }
}
