use crate::backend::{InferenceBackend, MockBackend};
use crate::config::ModelConfig;
use crate::request::{InferenceError, InferenceRequest};

fn test_config() -> ModelConfig {
    ModelConfig::cpu_only("test-model", "/tmp/test.gguf")
}

#[tokio::test]
async fn mock_backend_load_and_infer() {
    let mut backend = MockBackend::new();
    assert!(!backend.is_loaded());

    backend.load(&test_config()).await.unwrap();
    assert!(backend.is_loaded());
    assert_eq!(backend.loaded_model_name(), Some("test-model"));

    let request = InferenceRequest::new("Hello world", 32);
    let response = backend.infer(&request).await.unwrap();
    assert_eq!(response.text, "mock response");
    assert!(response.usage.total() > 0);
}

#[tokio::test]
async fn mock_backend_infer_without_load_fails() {
    let backend = MockBackend::new();
    let request = InferenceRequest::new("Hello", 32);
    let err = backend.infer(&request).await.unwrap_err();
    assert!(matches!(err, InferenceError::NotLoaded(_)));
}

#[tokio::test]
async fn mock_backend_unload_clears_state() {
    let mut backend = MockBackend::new();
    backend.load(&test_config()).await.unwrap();
    assert!(backend.is_loaded());

    backend.unload().await.unwrap();
    assert!(!backend.is_loaded());
    assert!(backend.loaded_model_name().is_none());
}

#[tokio::test]
async fn mock_backend_custom_response() {
    let mut backend = MockBackend::new().with_response("custom output");
    backend.load(&test_config()).await.unwrap();

    let request = InferenceRequest::new("test prompt", 16);
    let response = backend.infer(&request).await.unwrap();
    assert_eq!(response.text, "custom output");
}

#[tokio::test]
async fn inference_request_defaults() {
    let request = InferenceRequest::new("prompt", 128);
    assert_eq!(request.max_tokens, 128);
    assert!((request.temperature - 0.7).abs() < f32::EPSILON);
    assert!(request.stop.is_empty());
}

#[tokio::test]
async fn token_usage_total() {
    use crate::request::TokenUsage;
    let usage = TokenUsage {
        prompt_tokens: 10,
        completion_tokens: 20,
    };
    assert_eq!(usage.total(), 30);
}

#[tokio::test]
async fn cpu_only_config_defaults() {
    let config = ModelConfig::cpu_only("my-model", "/path/to/model.gguf");
    assert_eq!(config.name, "my-model");
    assert_eq!(config.gpu_layers, 0);
    assert_eq!(config.context_length, 4096);
    assert_eq!(config.quantization, "Q4_K_M");
}
