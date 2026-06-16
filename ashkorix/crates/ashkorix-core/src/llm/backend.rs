use std::sync::OnceLock;

use crate::error::{AshkorixError, Result};
use llama_cpp_2::llama_backend::LlamaBackend;

static BACKEND: OnceLock<LlamaBackend> = OnceLock::new();

/// llama.cpp allows only one backend init per process.
pub fn shared_llama_backend() -> Result<&'static LlamaBackend> {
    if let Some(backend) = BACKEND.get() {
        return Ok(backend);
    }
    let backend =
        LlamaBackend::init().map_err(|e| AshkorixError::Model(e.to_string()))?;
    let _ = BACKEND.set(backend);
    BACKEND
        .get()
        .ok_or_else(|| AshkorixError::Model("failed to store llama backend".into()))
}
