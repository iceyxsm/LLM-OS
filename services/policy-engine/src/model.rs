use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PolicyDocument {
    pub version: String,
    pub rules: Vec<PolicyRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct PolicyRule {
    pub id: String,
    pub effect: RuleEffect,
    pub subject: String,
    pub actions: Vec<String>,
    pub resources: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleEffect {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRequest {
    pub subject: String,
    pub action: String,
    pub resource: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyDecision {
    pub effect: DecisionEffect,
    pub reason: DecisionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionEffect {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionReason {
    MatchedAllow { rule_id: String },
    MatchedDeny { rule_id: String },
    NoMatch,
}

