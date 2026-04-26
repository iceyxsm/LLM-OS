use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDescriptor {
    pub id: String,
    pub version: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionRequest {
    pub version: String,
    pub subject: String,
    pub action: String,
    pub resource: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionResult {
    pub version: String,
    pub status: ActionStatus,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ActionStatus {
    Executed,
    Denied,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyDecisionRecord {
    pub version: String,
    pub effect: PolicyEffect,
    pub reason: String,
    pub rule_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyEffect {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub version: String,
    pub timestamp_unix_ms: u128,
    pub subject: String,
    pub action: String,
    pub resource: String,
    pub decision: PolicyDecisionRecord,
    pub outcome: ActionStatus,
}

#[derive(Debug, Error)]
pub enum LlmOsError {
    #[error("module not found: {0}")]
    ModuleNotFound(String),
    #[error("action denied: {0}")]
    ActionDenied(String),
    #[error("policy unavailable: {0}")]
    PolicyUnavailable(String),
}
