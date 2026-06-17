use crate::error::{AshkorixError, Result};
use crate::llm::backend::shared_llama_backend;
use crate::llm::gpu::resolve_gpu_layers;
use crate::rag::types::RankedChunk;
use llama_cpp_2::context::params::{LlamaAttentionType, LlamaContextParams, LlamaPoolingType};
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::token::LlamaToken;
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

/// Max tokens per query+document pair. Must not exceed n_ctx / n_seq_max (llama.cpp
/// caps total context, often to 2048 → 256 per seq when n_seq_max = 8).
const MAX_SEQ_TOKENS: u32 = 256;
/// Max sequences packed into one llama encode call.
const MAX_SEQS_PER_BATCH: u32 = 4;
/// llama.cpp encoder assert: n_ubatch >= total tokens in the batch.
const MAX_UBATCH_TOKENS: u32 = 512;

pub struct LlamaRerankerService {
    model: Option<Arc<LlamaModel>>,
    use_encode: bool,
    n_threads: i32,
}

impl LlamaRerankerService {
    pub fn new(n_threads: i32) -> Result<Self> {
        let _ = shared_llama_backend()?;
        Ok(Self {
            model: None,
            use_encode: true,
            n_threads,
        })
    }

    pub async fn load(&mut self, path: &Path) -> Result<()> {
        let backend = shared_llama_backend()?;
        let model_params = LlamaModelParams::default().with_n_gpu_layers(resolve_gpu_layers(0));
        let model = LlamaModel::load_from_file(backend, path, &model_params)
            .map_err(|e| AshkorixError::Model(e.to_string()))?;

        let path_lower = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        self.use_encode = path_lower.contains("bert")
            || path_lower.contains("bge")
            || path_lower.contains("jina")
            || path_lower.contains("cross")
            || path_lower.contains("e5")
            || path_lower.contains("nomic");
        self.model = Some(Arc::new(model));
        Ok(())
    }

    pub fn unload(&mut self) {
        self.model = None;
    }

    pub fn is_loaded(&self) -> bool {
        self.model.is_some()
    }

    pub fn rerank(
        &self,
        query: &str,
        mut chunks: Vec<RankedChunk>,
        top_k: usize,
    ) -> Result<Vec<RankedChunk>> {
        if chunks.is_empty() {
            return Ok(chunks);
        }

        let model = self
            .model
            .as_ref()
            .ok_or_else(|| AshkorixError::Model("reranker model not loaded".into()))?;

        let pairs: Vec<String> = chunks
            .iter()
            .map(|c| format!("{query}\n\n{}", c.chunk.text))
            .collect();
        let scores = self.score_pairs(model, &pairs)?;

        for (chunk, score) in chunks.iter_mut().zip(scores) {
            chunk.rerank_score = Some(f64::from(score));
        }

        chunks.sort_by(|a, b| {
            b.rerank_score
                .partial_cmp(&a.rerank_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        chunks.truncate(top_k);
        Ok(chunks)
    }

    fn score_pairs(&self, model: &LlamaModel, pairs: &[String]) -> Result<Vec<f32>> {
        let tokenized = self.tokenize_pairs(model, pairs)?;
        let batches = pack_token_batches(
            tokenized,
            MAX_SEQS_PER_BATCH as usize,
            MAX_UBATCH_TOKENS as usize,
        );

        let backend = shared_llama_backend()?;
        // n_ctx must give each sequence slot up to MAX_SEQ_TOKENS (n_ctx_seq = n_ctx / n_seq_max).
        let n_ctx = NonZeroU32::new(MAX_SEQ_TOKENS * MAX_SEQS_PER_BATCH)
            .ok_or_else(|| AshkorixError::Model("invalid rerank n_ctx".into()))?;
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(Some(n_ctx))
            .with_n_batch(MAX_UBATCH_TOKENS)
            .with_n_ubatch(MAX_UBATCH_TOKENS)
            .with_n_seq_max(MAX_SEQS_PER_BATCH)
            .with_embeddings(true)
            .with_pooling_type(LlamaPoolingType::Rank)
            .with_attention_type(LlamaAttentionType::NonCausal)
            .with_n_threads(self.n_threads);
        let mut ctx = model
            .new_context(backend, ctx_params)
            .map_err(|e| AshkorixError::Model(e.to_string()))?;

        let mut scores = Vec::with_capacity(pairs.len());
        for batch in batches {
            scores.extend(self.score_token_batch(&mut ctx, &batch)?);
        }
        Ok(scores)
    }

    fn tokenize_pairs(&self, model: &LlamaModel, pairs: &[String]) -> Result<Vec<Vec<LlamaToken>>> {
        pairs
            .iter()
            .map(|text| {
                let mut tokens = model
                    .str_to_token(text, AddBos::Always)
                    .map_err(|e| AshkorixError::Model(e.to_string()))?;
                if tokens.len() > MAX_SEQ_TOKENS as usize {
                    tokens.truncate(MAX_SEQ_TOKENS as usize);
                }
                if tokens.is_empty() {
                    return Err(AshkorixError::Model(
                        "rerank pair produced zero tokens".into(),
                    ));
                }
                Ok(tokens)
            })
            .collect()
    }

    fn score_token_batch(
        &self,
        ctx: &mut llama_cpp_2::context::LlamaContext,
        tokenized: &[Vec<LlamaToken>],
    ) -> Result<Vec<f32>> {
        if tokenized.is_empty() {
            return Ok(vec![]);
        }

        let total_tokens: usize = tokenized.iter().map(|t| t.len()).sum();
        if total_tokens > MAX_UBATCH_TOKENS as usize {
            return Err(AshkorixError::Model(format!(
                "rerank batch has {total_tokens} tokens but n_ubatch is {MAX_UBATCH_TOKENS}"
            )));
        }

        let n_seqs = i32::try_from(tokenized.len())
            .map_err(|_| AshkorixError::Model("too many sequences in rerank batch".into()))?;
        let mut batch = LlamaBatch::new(total_tokens, n_seqs);
        for (seq_id, tokens) in tokenized.iter().enumerate() {
            batch
                .add_sequence(tokens, seq_id as i32, true)
                .map_err(|e| AshkorixError::Model(e.to_string()))?;
        }

        ctx.clear_kv_cache();
        if self.use_encode {
            ctx.encode(&mut batch)
                .map_err(|e| AshkorixError::Model(e.to_string()))?;
        } else {
            ctx.decode(&mut batch)
                .map_err(|e| AshkorixError::Model(e.to_string()))?;
        }

        let mut scores = Vec::with_capacity(tokenized.len());
        for seq_id in 0..tokenized.len() {
            let embedding = ctx
                .embeddings_seq_ith(seq_id as i32)
                .map_err(|e| AshkorixError::Model(e.to_string()))?;
            if embedding.is_empty() {
                return Err(AshkorixError::Model(
                    "reranker returned empty score vector".into(),
                ));
            }
            scores.push(embedding[0]);
        }
        Ok(scores)
    }
}

/// Pack tokenized pairs into encode batches that satisfy llama.cpp encoder limits.
fn pack_token_batches(
    tokenized: Vec<Vec<LlamaToken>>,
    max_seqs: usize,
    max_ubatch_tokens: usize,
) -> Vec<Vec<Vec<LlamaToken>>> {
    let mut batches = Vec::new();
    let mut current: Vec<Vec<LlamaToken>> = Vec::new();
    let mut current_tokens = 0usize;

    for seq in tokenized {
        let len = seq.len();
        let need_flush = !current.is_empty()
            && (current.len() >= max_seqs || current_tokens.saturating_add(len) > max_ubatch_tokens);
        if need_flush {
            batches.push(std::mem::take(&mut current));
            current_tokens = 0;
        }
        current.push(seq);
        current_tokens += len;
    }

    if !current.is_empty() {
        batches.push(current);
    }

    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_tokens(lens: &[usize]) -> Vec<Vec<LlamaToken>> {
        lens.iter()
            .map(|&n| (0..n).map(|i| LlamaToken(i as i32)).collect())
            .collect()
    }

    fn batch_lens(batches: &[Vec<Vec<LlamaToken>>]) -> Vec<Vec<usize>> {
        batches
            .iter()
            .map(|b| b.iter().map(|s| s.len()).collect())
            .collect()
    }

    #[test]
    fn pack_respects_ubatch_token_budget() {
        let batches = pack_token_batches(fake_tokens(&[100, 100, 100, 100, 100]), 8, 512);
        for batch in &batches {
            let total: usize = batch.iter().map(|s| s.len()).sum();
            assert!(total <= 512);
            assert!(batch.len() <= 8);
        }
        assert_eq!(batch_lens(&batches), vec![vec![100, 100, 100, 100, 100]]);
    }

    #[test]
    fn pack_splits_when_ubatch_exceeded() {
        let batches = pack_token_batches(fake_tokens(&[300, 300]), 8, 512);
        assert_eq!(batch_lens(&batches), vec![vec![300], vec![300]]);
    }

    #[test]
    fn pack_splits_when_seq_count_exceeded() {
        let batches = pack_token_batches(fake_tokens(&[50; 10]), 8, 512);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 8);
        assert_eq!(batches[1].len(), 2);
        for batch in &batches {
            assert!(batch.iter().map(|s| s.len()).sum::<usize>() <= 512);
        }
    }

    #[test]
    fn pack_allows_full_length_single_pair() {
        let batches = pack_token_batches(fake_tokens(&[256]), 4, 512);
        assert_eq!(batch_lens(&batches), vec![vec![256]]);
    }
}
