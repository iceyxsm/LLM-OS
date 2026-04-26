# Model Runtime Module

Purpose: provider-agnostic inference execution layer for local LLM workloads.

## Implementation

The `llmos-model-runtime` crate in `crates/model-runtime/` provides:

- `InferenceBackend` trait for pluggable inference engines
- `MockBackend` for testing without real model weights
- `ModelConfig` for model loading parameters (path, quantization, context length, GPU layers)
- `InferenceRequest` and `InferenceResponse` for the inference API
- `TokenUsage` for prompt/completion token accounting

## Design

The trait is designed to support both local backends (llama.cpp, GGML, candle) and
remote backends through the same interface. The mock backend enables full pipeline
testing without model weights on disk.
