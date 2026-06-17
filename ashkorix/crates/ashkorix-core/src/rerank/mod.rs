mod service;

use crate::error::Result;
use crate::rag::types::RankedChunk;
pub use service::LlamaRerankerService;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct HeuristicReranker;

impl HeuristicReranker {
    pub fn rerank(query: &str, mut chunks: Vec<RankedChunk>, top_k: usize) -> Result<Vec<RankedChunk>> {
        let query_lower = query.to_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        for chunk in &mut chunks {
            let text_lower = chunk.chunk.text.to_lowercase();
            let heading = chunk
                .chunk
                .heading_path
                .as_deref()
                .unwrap_or("")
                .to_lowercase();
            let section = chunk
                .chunk
                .section_title
                .as_deref()
                .unwrap_or("")
                .to_lowercase();
            let entity = chunk
                .chunk
                .entity_tokens
                .as_deref()
                .unwrap_or("")
                .to_lowercase();

            let mut overlap = 0f64;
            for term in &query_terms {
                if term.len() < 2 {
                    continue;
                }
                if text_lower.contains(term) {
                    overlap += 1.0;
                }
                if heading.contains(term) {
                    overlap += 0.5;
                }
                if section.contains(term) {
                    overlap += 0.5;
                }
                if entity.contains(term) {
                    overlap += 0.75;
                }
            }

            if text_lower.contains(&query_lower) {
                overlap += 2.0;
            }

            if let Ok(re) = regex::Regex::new(r"(?i)\bphase\s+\d+\b") {
                if let Some(m) = re.find(query) {
                    let phrase = m.as_str().to_lowercase();
                    if heading.contains(&phrase)
                        || section.contains(&phrase)
                        || text_lower.contains(&phrase)
                    {
                        overlap += 5.0;
                    }
                }
            }

            let rrf = chunk.score;
            let rerank_score = rrf * 0.4 + overlap * 0.6;
            chunk.rerank_score = Some(rerank_score);
        }

        chunks.sort_by(|a, b| {
            b.rerank_score
                .partial_cmp(&a.rerank_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        chunks.truncate(top_k);
        Ok(chunks)
    }
}

pub struct CompositeReranker {
    model: Arc<Mutex<LlamaRerankerService>>,
}

impl CompositeReranker {
    pub fn new(model: Arc<Mutex<LlamaRerankerService>>) -> Self {
        Self { model }
    }

    pub async fn rerank(
        &self,
        query: &str,
        chunks: Vec<RankedChunk>,
        top_k: usize,
    ) -> Result<Vec<RankedChunk>> {
        let service = self.model.lock().await;
        if service.is_loaded() {
            match service.rerank(query, chunks.clone(), top_k) {
                Ok(ranked) => return Ok(ranked),
                Err(e) => {
                    tracing::warn!(
                        "GGUF reranker failed ({e}); falling back to heuristic rerank"
                    );
                }
            }
        }
        HeuristicReranker::rerank(query, chunks, top_k)
    }
}
