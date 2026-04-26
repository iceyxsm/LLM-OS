mod backend;
mod config;
mod request;

#[cfg(feature = "llama-cpp")]
mod llama;

pub use backend::{InferenceBackend, MockBackend};
pub use config::ModelConfig;
pub use request::{InferenceError, InferenceRequest, InferenceResponse, TokenUsage};

#[cfg(feature = "llama-cpp")]
pub use llama::LlamaCppBackend;

#[cfg(test)]
mod tests;
