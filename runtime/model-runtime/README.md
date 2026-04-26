# Model Runtime Module

Purpose: provider-agnostic inference execution layer for local LLM workloads.

## Implementation

The `llmos-model-runtime` crate in `crates/model-runtime/` provides:

- `InferenceBackend` trait for pluggable inference engines
- `MockBackend` for testing without real model weights
- `LlamaCppBackend` for real local inference via llama.cpp (feature-gated)
- `ModelConfig` for model loading parameters (path, quantization, context length, GPU layers)
- `InferenceRequest` and `InferenceResponse` for the inference API
- `TokenUsage` for prompt/completion token accounting

## Feature flags

| Feature | Description |
|---|---|
| `llama-cpp` | Enable the llama.cpp backend (requires C++ compiler and cmake) |
| `cuda` | Enable CUDA GPU support (implies `llama-cpp`) |
| `metal` | Enable Apple Metal GPU support (implies `llama-cpp`) |
| `vulkan` | Enable Vulkan GPU support (implies `llama-cpp`) |

## Usage

Default (mock only, no native dependencies):
```toml
llmos-model-runtime = { path = "crates/model-runtime" }
```

With llama.cpp CPU inference:
```toml
llmos-model-runtime = { path = "crates/model-runtime", features = ["llama-cpp"] }
```

With CUDA GPU acceleration:
```toml
llmos-model-runtime = { path = "crates/model-runtime", features = ["cuda"] }
```

## Design

The trait is designed to support both local backends (llama.cpp, GGML, candle) and
remote backends through the same interface. The mock backend enables full pipeline
testing without model weights on disk. The llama.cpp backend is feature-gated to
keep CI fast and avoid requiring a C++ toolchain for development.
