use crate::error::{AshkorixError, Result};
use crate::llm::backend::shared_llama_backend;
use crate::llm::gpu::resolve_gpu_layers;
use crate::traits::EmbeddingService;
use async_trait::async_trait;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

/// nomic-embed and similar BERT encoders need n_ubatch >= token count per sequence.
const EMBED_CTX_TOKENS: u32 = 2048;

pub struct LlamaEmbeddingService {
    model: Option<Arc<LlamaModel>>,
    dimension: usize,
}

impl LlamaEmbeddingService {
    pub fn new() -> Result<Self> {
        let _ = shared_llama_backend()?;
        Ok(Self {
            model: None,
            dimension: 0,
        })
    }
}

#[async_trait]
impl EmbeddingService for LlamaEmbeddingService {
    async fn load(&mut self, path: &Path) -> Result<()> {
        let backend = shared_llama_backend()?;
        let model_params = LlamaModelParams::default().with_n_gpu_layers(resolve_gpu_layers(0));
        let model = LlamaModel::load_from_file(backend, path, &model_params)
            .map_err(|e| AshkorixError::Model(e.to_string()))?;

        let dimension = usize::try_from(model.n_embd())
            .map_err(|_| AshkorixError::Model("embedding dimension overflow".into()))?;
        if dimension == 0 {
            return Err(AshkorixError::Model(
                "model reported zero embedding dimension".into(),
            ));
        }

        self.dimension = dimension;
        self.model = Some(Arc::new(model));
        Ok(())
    }

    fn is_loaded(&self) -> bool {
        self.model.is_some()
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| AshkorixError::Model("embedding model not loaded".into()))?;

        if texts.is_empty() {
            return Ok(vec![]);
        }
        if texts.len() == 1 {
            return Ok(vec![embed_text(model, &texts[0])?]);
        }

        embed_batch_with_shared_context(model, texts)
    }
}

fn embed_batch_with_shared_context(model: &LlamaModel, texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let backend = shared_llama_backend()?;
    let n_ctx = NonZeroU32::new(EMBED_CTX_TOKENS)
        .ok_or_else(|| AshkorixError::Model("invalid n_ctx".into()))?;
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx))
        .with_n_batch(EMBED_CTX_TOKENS)
        .with_n_ubatch(EMBED_CTX_TOKENS)
        .with_embeddings(true);
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| AshkorixError::Model(e.to_string()))?;

    let mut results = Vec::with_capacity(texts.len());
    for text in texts {
        results.push(embed_with_context(model, &mut ctx, text)?);
    }
    Ok(results)
}

fn embed_text(model: &LlamaModel, text: &str) -> Result<Vec<f32>> {
    let backend = shared_llama_backend()?;
    let n_ctx = NonZeroU32::new(EMBED_CTX_TOKENS)
        .ok_or_else(|| AshkorixError::Model("invalid n_ctx".into()))?;
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx))
        .with_n_batch(EMBED_CTX_TOKENS)
        .with_n_ubatch(EMBED_CTX_TOKENS)
        .with_embeddings(true);
    let mut ctx = model
        .new_context(backend, ctx_params)
        .map_err(|e| AshkorixError::Model(e.to_string()))?;
    embed_with_context(model, &mut ctx, text)
}

fn embed_with_context(
    model: &LlamaModel,
    ctx: &mut llama_cpp_2::context::LlamaContext,
    text: &str,
) -> Result<Vec<f32>> {
    let n_ctx = EMBED_CTX_TOKENS as usize;
    let mut tokens = model
        .str_to_token(text, AddBos::Always)
        .map_err(|e| AshkorixError::Model(e.to_string()))?;
    if tokens.is_empty() {
        return Err(AshkorixError::Model("text produced zero tokens".into()));
    }
    if tokens.len() > n_ctx {
        tokens.truncate(n_ctx);
    }

    let mut batch = LlamaBatch::new(tokens.len(), 1);
    batch
        .add_sequence(&tokens, 0, false)
        .map_err(|e| AshkorixError::Model(e.to_string()))?;
    ctx.decode(&mut batch)
        .map_err(|e| AshkorixError::Model(e.to_string()))?;

    let embedding = ctx
        .embeddings_seq_ith(0)
        .map_err(|e| AshkorixError::Model(e.to_string()))?;
    if embedding.is_empty() {
        return Err(AshkorixError::Model(
            "model returned empty embedding; ensure a GGUF embedding model is loaded".into(),
        ));
    }

    Ok(embedding.to_vec())
}
