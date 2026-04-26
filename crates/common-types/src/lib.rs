use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDescriptor {
    pub id: String,
    pub version: String,
    pub status: String,
}

#[derive(Debug, Error)]
pub enum LlmOsError {
    #[error("module not found: {0}")]
    ModuleNotFound(String),
}
