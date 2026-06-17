use crate::config::MemoryConfig;
use crate::documents::storage::DocumentStore;
use crate::embeddings::LlamaEmbeddingService;
use crate::error::Result;
use crate::memory::format::type_relevance_score;
use crate::memory::store::MemoryStore;
use crate::memory::types::{memory_cache_key, Memory};
use crate::traits::EmbeddingService;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct MemoryRetriever;

#[derive(Debug, Clone)]
struct ScoredMemory {
    memory: Memory,
    score: f64,
}

impl MemoryRetriever {
    pub async fn retrieve(
        memory_store: &MemoryStore,
        doc_store: &DocumentStore,
        embedding: Arc<Mutex<LlamaEmbeddingService>>,
        config: &MemoryConfig,
        session_id: &str,
        query: &str,
    ) -> Result<Vec<Memory>> {
        if !config.enabled {
            return Ok(vec![]);
        }

        let scopes = config.active_scopes(session_id);
        let candidates = memory_store.list_by_scopes(&scopes, config.min_confidence)?;
        if candidates.is_empty() {
            return Ok(vec![]);
        }

        let query_embedding = if embedding.lock().await.is_loaded() {
            let vecs = embedding
                .lock()
                .await
                .embed_batch(&[query.to_string()])
                .await?;
            vecs.into_iter().next()
        } else {
            tracing::warn!("embedding model not loaded; memory retrieval uses non-semantic ranking");
            None
        };

        let mut memory_embeddings: Vec<(usize, Vec<f32>)> = Vec::new();
        if query_embedding.is_some() {
            let mut to_embed: Vec<(usize, String)> = Vec::new();
            for (i, mem) in candidates.iter().enumerate() {
                let key = memory_cache_key(&mem.content);
                if let Some(cached) = doc_store.get_cached_embedding(&key)? {
                    memory_embeddings.push((i, cached));
                } else {
                    to_embed.push((i, mem.content.clone()));
                }
            }
            if !to_embed.is_empty() {
                const BATCH: usize = 32;
                for chunk in to_embed.chunks(BATCH) {
                    let texts: Vec<String> = chunk.iter().map(|(_, t)| t.clone()).collect();
                    let vectors = embedding.lock().await.embed_batch(&texts).await?;
                    for ((idx, text), vector) in chunk.iter().zip(vectors.iter()) {
                        let key = memory_cache_key(text);
                        doc_store.cache_embedding(&key, vector)?;
                        memory_embeddings.push((*idx, vector.clone()));
                    }
                }
            }
        }

        let mut scored: Vec<ScoredMemory> = candidates
            .into_iter()
            .enumerate()
            .map(|(i, memory)| {
                let score = Self::score_memory(
                    &memory,
                    session_id,
                    query,
                    query_embedding.as_deref(),
                    memory_embeddings
                        .iter()
                        .find(|(idx, _)| *idx == i)
                        .map(|(_, v)| v.as_slice()),
                );
                ScoredMemory { memory, score }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let limit = config.max_injected.max(3).min(8);
        let selected: Vec<Memory> = scored
            .into_iter()
            .filter(|s| s.memory.importance >= config.min_importance)
            .take(limit)
            .map(|s| s.memory)
            .collect();

        let ids: Vec<String> = selected.iter().map(|m| m.id.clone()).collect();
        memory_store.touch_last_used(&ids)?;

        Ok(selected)
    }

    fn score_memory(
        memory: &Memory,
        session_id: &str,
        query: &str,
        query_embedding: Option<&[f32]>,
        memory_embedding: Option<&[f32]>,
    ) -> f64 {
        let scope_score = scope_match_score(&memory.scope, session_id);
        let type_score = type_relevance_score(memory.memory_type, query);
        let importance_score = memory.importance.clamp(0.0, 1.0);
        let semantic_score = match (query_embedding, memory_embedding) {
            (Some(q), Some(m)) => cosine_similarity(q, m).clamp(0.0, 1.0),
            _ => 0.5,
        };
        let recency_score = recency_score(memory.last_used_at.or(Some(memory.updated_at)));

        0.30 * scope_score
            + 0.15 * type_score
            + 0.25 * importance_score
            + 0.25 * semantic_score
            + 0.05 * recency_score
    }
}

fn scope_match_score(scope: &str, session_id: &str) -> f64 {
    if scope == format!("conversation:{session_id}") {
        1.0
    } else if scope.starts_with("project:") {
        0.75
    } else if scope == "global" {
        0.5
    } else {
        0.4
    }
}

fn recency_score(timestamp: Option<DateTime<Utc>>) -> f64 {
    let Some(ts) = timestamp else {
        return 0.3;
    };
    let age_days = (Utc::now() - ts).num_days().max(0) as f64;
    (1.0 / (1.0 + age_days / 30.0)).clamp(0.1, 1.0)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    (dot / (norm_a * norm_b)) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_vectors() {
        let v = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn scope_prefers_conversation() {
        assert!(scope_match_score("conversation:abc", "abc") > scope_match_score("global", "abc"));
    }
}
