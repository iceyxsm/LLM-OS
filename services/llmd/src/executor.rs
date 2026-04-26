use std::sync::Arc;

use async_trait::async_trait;
use common_types::{ActionRequest, ActionResult, ActionStatus, LlmOsError};
use llmos_model_runtime::{InferenceBackend, InferenceRequest};
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::ActionExecutor;

/// An executor that delegates "model:invoke" actions to an InferenceBackend.
///
/// For actions that are not model invocations, it falls back to a simple
/// pass-through that marks the action as executed.
pub struct ModelExecutor {
    backend: Arc<RwLock<dyn InferenceBackend>>,
}

impl ModelExecutor {
    pub fn new(backend: Arc<RwLock<dyn InferenceBackend>>) -> Self {
        Self { backend }
    }
}

#[async_trait]
impl ActionExecutor for ModelExecutor {
    async fn execute(&self, request: &ActionRequest) -> Result<ActionResult, LlmOsError> {
        // Only handle model:invoke actions through the backend.
        // All other actions pass through as executed.
        if request.action != "model:invoke" {
            return Ok(ActionResult {
                version: request.version.clone(),
                status: ActionStatus::Executed,
                message: format!(
                    "executed {} for subject {} on {}",
                    request.action, request.subject, request.resource
                ),
            });
        }

        let backend = self.backend.read().await;

        if !backend.is_loaded() {
            return Ok(ActionResult {
                version: request.version.clone(),
                status: ActionStatus::Executed,
                message: "model:invoke accepted (no model loaded; pass-through)".to_string(),
            });
        }

        // The resource field carries the prompt for model:invoke actions.
        let inference_request = InferenceRequest::new(&request.resource, 128);

        info!(
            target: "llmd::executor",
            subject = %request.subject,
            prompt_len = request.resource.len(),
            "running inference"
        );

        match backend.infer(&inference_request).await {
            Ok(response) => {
                debug!(
                    target: "llmd::executor",
                    prompt_tokens = response.usage.prompt_tokens,
                    completion_tokens = response.usage.completion_tokens,
                    ttft_ms = response.ttft_ms,
                    total_ms = response.total_ms,
                    "inference complete"
                );

                Ok(ActionResult {
                    version: request.version.clone(),
                    status: ActionStatus::Executed,
                    message: format!(
                        "model:invoke complete ({}ms, {} tokens): {}",
                        response.total_ms,
                        response.usage.completion_tokens,
                        truncate_text(&response.text, 200)
                    ),
                })
            }
            Err(err) => Err(LlmOsError::ActionDenied(format!("inference failed: {err}"))),
        }
    }
}

fn truncate_text(text: &str, max_len: usize) -> &str {
    if text.len() <= max_len {
        text
    } else {
        &text[..max_len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llmos_model_runtime::MockBackend;

    fn test_request(action: &str) -> ActionRequest {
        ActionRequest {
            version: "v1".to_string(),
            request_id: "req-1".to_string(),
            correlation_id: "corr-1".to_string(),
            subject: "runtime/model-runtime".to_string(),
            action: action.to_string(),
            resource: "test prompt".to_string(),
        }
    }

    #[tokio::test]
    async fn non_model_action_passes_through() {
        let backend = Arc::new(RwLock::new(MockBackend::new()));
        let executor = ModelExecutor::new(backend);
        let result = executor
            .execute(&test_request("network:connect"))
            .await
            .unwrap();
        assert_eq!(result.status, ActionStatus::Executed);
        assert!(result.message.contains("network:connect"));
    }

    #[tokio::test]
    async fn model_invoke_without_loaded_model() {
        let backend = Arc::new(RwLock::new(MockBackend::new()));
        let executor = ModelExecutor::new(backend);
        let result = executor
            .execute(&test_request("model:invoke"))
            .await
            .unwrap();
        assert_eq!(result.status, ActionStatus::Executed);
        assert!(result.message.contains("pass-through"));
    }

    #[tokio::test]
    async fn model_invoke_with_loaded_model() {
        let mut mock = MockBackend::new().with_response("generated text here");
        mock.load(&llmos_model_runtime::ModelConfig::cpu_only(
            "test",
            "/tmp/test.gguf",
        ))
        .await
        .unwrap();
        let backend = Arc::new(RwLock::new(mock));
        let executor = ModelExecutor::new(backend);
        let result = executor
            .execute(&test_request("model:invoke"))
            .await
            .unwrap();
        assert_eq!(result.status, ActionStatus::Executed);
        assert!(result.message.contains("model:invoke complete"));
        assert!(result.message.contains("generated text here"));
    }

    #[tokio::test]
    async fn model_invoke_includes_timing_info() {
        let mut mock = MockBackend::new();
        mock.load(&llmos_model_runtime::ModelConfig::cpu_only(
            "test",
            "/tmp/test.gguf",
        ))
        .await
        .unwrap();
        let backend = Arc::new(RwLock::new(mock));
        let executor = ModelExecutor::new(backend);
        let result = executor
            .execute(&test_request("model:invoke"))
            .await
            .unwrap();
        // MockBackend returns 50ms total, some number of tokens
        assert!(result.message.contains("50ms"));
    }
}
