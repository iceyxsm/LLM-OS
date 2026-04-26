use serde::{Deserialize, Serialize};

/// Linux namespace isolation settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamespaceConfig {
    pub pid: bool,
    pub net: bool,
    pub mount: bool,
    pub ipc: bool,
    pub user: bool,
}

/// Pre-built namespace presets.
pub struct NamespacePreset;

impl NamespacePreset {
    /// Full isolation: all namespaces enabled.
    pub fn full() -> NamespaceConfig {
        NamespaceConfig {
            pid: true,
            net: true,
            mount: true,
            ipc: true,
            user: true,
        }
    }

    /// Network-only isolation: separate network namespace, shared everything else.
    pub fn network_only() -> NamespaceConfig {
        NamespaceConfig {
            pid: false,
            net: true,
            mount: false,
            ipc: false,
            user: false,
        }
    }

    /// No isolation: all namespaces shared with host.
    pub fn none() -> NamespaceConfig {
        NamespaceConfig {
            pid: false,
            net: false,
            mount: false,
            ipc: false,
            user: false,
        }
    }
}
