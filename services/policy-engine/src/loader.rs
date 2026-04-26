use std::path::Path;

use anyhow::{Context, Result};

use crate::model::PolicyDocument;

pub fn load_policy_document(path: &Path) -> Result<PolicyDocument> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read policy file at {}", path.display()))?;

    let policy: PolicyDocument = serde_yaml::from_str(&content)
        .with_context(|| format!("failed to parse policy YAML at {}", path.display()))?;

    Ok(policy)
}

