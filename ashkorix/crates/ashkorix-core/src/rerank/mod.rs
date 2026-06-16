use crate::error::Result;
use crate::rag::types::RankedChunk;

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
                if entity.contains(term) {
                    overlap += 0.75;
                }
            }

            if text_lower.contains(&query_lower) {
                overlap += 2.0;
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
    use_heuristic: bool,
}

impl CompositeReranker {
    pub fn new(_reranker_model_loaded: bool) -> Self {
        Self {
            use_heuristic: true,
        }
    }

    pub fn rerank(
        &self,
        query: &str,
        chunks: Vec<RankedChunk>,
        top_k: usize,
    ) -> Result<Vec<RankedChunk>> {
        if self.use_heuristic {
            HeuristicReranker::rerank(query, chunks, top_k)
        } else {
            Ok(chunks.into_iter().take(top_k).collect())
        }
    }
}
