use crate::chunking::util::{estimate_tokens, matches_file_types};
use crate::config::AshkorixConfig;
use crate::documents::storage::DocumentStore;
use crate::embeddings::LlamaEmbeddingService;
use crate::error::Result;
use crate::rag::agent::{build_corpus_map, RetrievalAgent, supports_corpus_map};
use crate::rag::context::ContextExpander;
use crate::rag::types::{CorpusMapResult, RankedChunk, RetrievalFilters, RetrievalMode};
use crate::rerank::{CompositeReranker, LlamaRerankerService};
use crate::search::fusion::{multi_reciprocal_rank_fusion, SourceType};
use crate::search::indexer::PoolIndexer;
use crate::search::query_plan::QueryPlanner;
use crate::traits::{EmbeddingService, LexicalIndex, RetrievalService, VectorIndex};
use crate::vectorstore::UsearchVectorIndex;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct HybridRetrievalService {
    config: AshkorixConfig,
    store: Arc<DocumentStore>,
    embedding: Arc<Mutex<LlamaEmbeddingService>>,
    indexer: Arc<PoolIndexer>,
    reranker: CompositeReranker,
    max_context_tokens: u32,
    query_planner: QueryPlanner,
    context_expander: ContextExpander,
}

impl HybridRetrievalService {
    pub fn new(
        config: AshkorixConfig,
        store: Arc<DocumentStore>,
        embedding: Arc<Mutex<LlamaEmbeddingService>>,
        indexer: Arc<PoolIndexer>,
        reranker: Arc<Mutex<LlamaRerankerService>>,
    ) -> Self {
        let max_context_tokens = config.generation.context_size;
        let budget = max_context_tokens * config.retrieval.retrieval_context_budget_pct / 100;
        Self {
            query_planner: QueryPlanner::new(store.clone()),
            context_expander: ContextExpander::new(store.clone(), budget),
            store,
            embedding,
            indexer,
            reranker: CompositeReranker::new(reranker),
            max_context_tokens,
            config,
        }
    }

    fn dedup_chunks(chunks: Vec<RankedChunk>) -> Vec<RankedChunk> {
        let mut seen_hashes = std::collections::HashSet::new();
        chunks
            .into_iter()
            .filter(|c| seen_hashes.insert(c.chunk.content_hash.clone()))
            .collect()
    }

    pub(crate) fn apply_filters(chunks: Vec<RankedChunk>, filters: &RetrievalFilters) -> Vec<RankedChunk> {
        if filters.document_ids.is_empty()
            && filters.file_types.is_empty()
            && filters.page_min.is_none()
            && filters.page_max.is_none()
            && filters.section_prefix.is_none()
            && filters.entity_match.is_none()
        {
            return chunks;
        }
        chunks
            .into_iter()
            .filter(|c| {
                if !filters.document_ids.is_empty()
                    && !filters
                        .document_ids
                        .contains(&c.chunk.document_id.0)
                {
                    return false;
                }
                if !filters.file_types.is_empty()
                    && !matches_file_types(&c.chunk.source_filename, &filters.file_types)
                {
                    return false;
                }
                if let Some(ref prefix) = filters.section_prefix {
                    let path = c.chunk.heading_path.as_deref().unwrap_or("");
                    if !path.starts_with(prefix) {
                        return false;
                    }
                }
                if let Some(ref entity) = filters.entity_match {
                    let tokens = c.chunk.entity_tokens.as_deref().unwrap_or("");
                    if !tokens.to_lowercase().contains(&entity.to_lowercase()) {
                        return false;
                    }
                }
                if let Some(min) = filters.page_min {
                    if c.chunk.page_number.unwrap_or(0) < min {
                        return false;
                    }
                }
                if let Some(max) = filters.page_max {
                    if c.chunk.page_number.unwrap_or(u32::MAX) > max {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    fn apply_budget(chunks: Vec<RankedChunk>, max_tokens: u32) -> Vec<RankedChunk> {
        let mut total = 0u32;
        let mut result = Vec::new();
        for chunk in chunks {
            let expanded_tokens = chunk
                .expanded_context
                .as_ref()
                .map(|e| estimate_tokens(e))
                .unwrap_or(0);
            let chunk_tokens = chunk.chunk.token_count + expanded_tokens;
            if total + chunk_tokens > max_tokens {
                break;
            }
            total += chunk_tokens;
            result.push(chunk);
        }
        result
    }

    /// Assign `[Source N]` in retrieval relevance order (post-rerank / fusion).
    fn assign_source_numbers(mut chunks: Vec<RankedChunk>) -> Vec<RankedChunk> {
        for (i, chunk) in chunks.iter_mut().enumerate() {
            chunk.source_number = Some((i + 1) as u32);
        }
        chunks
    }

    fn search_variant_with_embedding(
        query: &str,
        query_vec: &[f32],
        vector_k: usize,
        lexical_k: usize,
        lexical: &crate::search::lexical::TantivyLexicalIndex,
        vector: &UsearchVectorIndex,
    ) -> Result<(Vec<(String, f32)>, Vec<(String, f32)>)> {
        let vector_results = if query_vec.is_empty() {
            vec![]
        } else {
            vector.search(query_vec, vector_k)?
        };
        let lexical_results = lexical.search(query, lexical_k)?;
        Ok((vector_results, lexical_results))
    }

    async fn search_variants_batch(
        &self,
        variants: &[String],
        vector_k: usize,
        lexical_k: usize,
        lexical: &crate::search::lexical::TantivyLexicalIndex,
        vector: &UsearchVectorIndex,
    ) -> Result<Vec<Vec<(String, f32)>>> {
        if variants.is_empty() {
            return Ok(vec![]);
        }
        let embeddings = self.embedding.lock().await.embed_batch(variants).await?;
        let mut all_lists = Vec::with_capacity(variants.len() * 2);
        for (variant, query_vec) in variants.iter().zip(embeddings.iter()) {
            let (vr, lr) = Self::search_variant_with_embedding(
                variant, query_vec, vector_k, lexical_k, lexical, vector,
            )?;
            all_lists.push(vr);
            all_lists.push(lr);
        }
        Ok(all_lists)
    }

    pub async fn retrieve_with_options(
        &self,
        query: &str,
        mode: RetrievalMode,
        exclude_chunk_ids: &[String],
        filters: &RetrievalFilters,
    ) -> Result<(Vec<RankedChunk>, Vec<String>, Option<CorpusMapResult>)> {
        if supports_corpus_map(mode) {
            let (chunks, variants, _) = Box::pin(self.retrieve_with_options(
                query,
                RetrievalMode::Balanced,
                exclude_chunk_ids,
                filters,
            ))
            .await?;
            let map = build_corpus_map(&self.store, query, chunks)?;
            return Ok((vec![], variants, Some(map)));
        }

        self.retrieve_hybrid(query, mode, exclude_chunk_ids, filters)
            .await
    }

    async fn retrieve_hybrid(
        &self,
        query: &str,
        mode: RetrievalMode,
        exclude_chunk_ids: &[String],
        filters: &RetrievalFilters,
    ) -> Result<(Vec<RankedChunk>, Vec<String>, Option<CorpusMapResult>)> {
        let (vector_k, lexical_k, rerank, max_chunks, _, iterative) = mode.params();
        let dim = self.embedding.lock().await.dimension();
        let (lexical, vector) = self.indexer.take_open_indexes(dim)?;

        let plan = self.query_planner.plan(query, mode)?;
        let all_lists = self
            .search_variants_batch(&plan.variants, vector_k, lexical_k, &lexical, &vector)
            .await?;

        let list_refs: Vec<&[(String, f32)]> = all_lists.iter().map(|l| l.as_slice()).collect();
        let fused = multi_reciprocal_rank_fusion(&list_refs, 60.0);

        let candidate_cap = self.config.retrieval.candidate_pool_size.max(max_chunks);
        let mut ranked = Self::collect_ranked(&self.store, &fused, exclude_chunk_ids, candidate_cap)?;

        ranked = Self::dedup_chunks(ranked);
        ranked = Self::apply_filters(ranked, filters);

        if iterative && RetrievalAgent::needs_second_pass(query, &ranked) {
            let second_queries = RetrievalAgent::second_pass_queries(&plan);
            let second_lists = self
                .search_variants_batch(&second_queries, vector_k, lexical_k, &lexical, &vector)
                .await?;
            if !second_lists.is_empty() {
                let refs: Vec<&[(String, f32)]> =
                    second_lists.iter().map(|l| l.as_slice()).collect();
                let second_fused = multi_reciprocal_rank_fusion(&refs, 60.0);
                let second_ranked =
                    Self::collect_ranked(&self.store, &second_fused, exclude_chunk_ids, candidate_cap)?;
                ranked = RetrievalAgent::merge_results(ranked, second_ranked, candidate_cap);
            }
        }

        self.indexer.restore_open_indexes(dim, lexical, vector);

        if rerank {
            ranked = self.reranker.rerank(query, ranked, max_chunks).await?;
        } else {
            ranked.truncate(max_chunks);
        }

        ranked = self.context_expander.expand(ranked)?;
        ranked = Self::apply_budget(ranked, self.max_context_tokens);
        ranked = Self::assign_source_numbers(ranked);

        Ok((ranked, plan.variants, None))
    }

    fn collect_ranked(
        store: &DocumentStore,
        fused: &[(String, f64, SourceType)],
        exclude_chunk_ids: &[String],
        candidate_cap: usize,
    ) -> Result<Vec<RankedChunk>> {
        let mut ranked = Vec::new();
        for (chunk_id, score, src_type) in fused {
            if exclude_chunk_ids.contains(chunk_id) {
                continue;
            }
            if let Some(chunk) = store.get_chunk(chunk_id)? {
                ranked.push(RankedChunk {
                    chunk,
                    score: *score,
                    source_type: src_type.as_str().to_string(),
                    source_number: None,
                    rerank_score: None,
                    expanded_context: None,
                });
            }
            if ranked.len() >= candidate_cap {
                break;
            }
        }
        Ok(ranked)
    }
}

#[async_trait]
impl RetrievalService for HybridRetrievalService {
    async fn retrieve(
        &self,
        query: &str,
        mode: RetrievalMode,
        exclude_chunk_ids: &[String],
        filters: &RetrievalFilters,
    ) -> Result<Vec<RankedChunk>> {
        let (chunks, _, _) = self
            .retrieve_with_options(query, mode, exclude_chunk_ids, filters)
            .await?;
        Ok(chunks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunking::types::Chunk;
    use crate::types::{ChunkId, CollectionId, DocumentId};

    fn ranked(id: &str, index: u32, score: f64, rerank: Option<f64>) -> RankedChunk {
        RankedChunk {
            chunk: Chunk {
                id: ChunkId(id.into()),
                document_id: DocumentId("doc".into()),
                collection_id: CollectionId("pool".into()),
                text: format!("chunk {index}"),
                start_offset: 0,
                end_offset: 10,
                page_number: None,
                section_title: None,
                row_sheet_info: None,
                source_filename: "plan.md".into(),
                content_hash: id.into(),
                token_count: 5,
                parent_section_id: None,
                heading_path: None,
                chunk_index: index,
                prev_chunk_id: None,
                next_chunk_id: None,
                contextual_text: None,
                table_id: None,
                entity_tokens: None,
            },
            score,
            source_type: "both".into(),
            source_number: None,
            rerank_score: rerank,
            expanded_context: None,
        }
    }

    #[test]
    fn assign_source_numbers_preserves_rerank_order() {
        // Simulates post-rerank order: Phase 1 chunk first, overview second.
        let chunks = vec![
            ranked("phase-1", 6, 0.7, Some(4.0)),
            ranked("overview", 4, 0.9, Some(2.0)),
        ];
        let numbered = HybridRetrievalService::assign_source_numbers(chunks);
        assert_eq!(numbered[0].source_number, Some(1));
        assert_eq!(numbered[0].chunk.chunk_index, 6);
        assert_eq!(numbered[1].source_number, Some(2));
        assert_eq!(numbered[1].chunk.chunk_index, 4);
    }
}
