use crate::config::AshkorixConfig;
use crate::documents::storage::DocumentStore;
use crate::embeddings::LlamaEmbeddingService;
use crate::error::Result;
use crate::rag::agent::{build_corpus_map, RetrievalAgent, supports_corpus_map};
use crate::rag::context::ContextExpander;
use crate::rag::types::{CorpusMapResult, RankedChunk, RetrievalFilters, RetrievalMode};
use crate::rerank::CompositeReranker;
use crate::search::fusion::multi_reciprocal_rank_fusion;
use crate::search::indexer::PoolIndexer;
use crate::search::query_plan::QueryPlanner;
use crate::traits::{EmbeddingService, LexicalIndex, RetrievalService, VectorIndex};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct HybridRetrievalService {
    config: AshkorixConfig,
    store: Arc<DocumentStore>,
    embedding: Arc<Mutex<LlamaEmbeddingService>>,
    indexer: PoolIndexer,
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
    ) -> Self {
        let max_context_tokens = config.generation.context_size;
        let budget = max_context_tokens * config.retrieval.retrieval_context_budget_pct / 100;
        let indexer = PoolIndexer::new(config.clone(), store.clone(), embedding.clone());
        Self {
            query_planner: QueryPlanner::new(store.clone()),
            context_expander: ContextExpander::new(store.clone(), budget),
            store,
            embedding,
            indexer,
            reranker: CompositeReranker::new(config.reranker_model_path.is_some()),
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

    fn apply_filters(chunks: Vec<RankedChunk>, filters: &RetrievalFilters) -> Vec<RankedChunk> {
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
                .map(|e| (e.len() / 4) as u32)
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

    fn stable_order(mut chunks: Vec<RankedChunk>) -> Vec<RankedChunk> {
        chunks.sort_by(|a, b| {
            a.chunk
                .document_id
                .0
                .cmp(&b.chunk.document_id.0)
                .then(a.chunk.chunk_index.cmp(&b.chunk.chunk_index))
                .then(a.chunk.start_offset.cmp(&b.chunk.start_offset))
        });
        for (i, chunk) in chunks.iter_mut().enumerate() {
            chunk.source_number = Some((i + 1) as u32);
        }
        chunks
    }

    async fn search_variant(
        &self,
        query: &str,
        vector_k: usize,
        lexical_k: usize,
        lexical: &mut crate::search::lexical::TantivyLexicalIndex,
        vector: &UsearchVectorIndex,
    ) -> Result<(Vec<(String, f32)>, Vec<(String, f32)>)> {
        let query_vec = self
            .embedding
            .lock()
            .await
            .embed_batch(&[query.to_string()])
            .await?
            .into_iter()
            .next()
            .unwrap_or_default();
        let vector_results = if query_vec.is_empty() {
            vec![]
        } else {
            vector.search(&query_vec, vector_k)?
        };
        let lexical_results = lexical.search(query, lexical_k)?;
        Ok((vector_results, lexical_results))
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
        let (mut lexical, vector) = self.indexer.open_indexes(dim)?;

        let plan = self.query_planner.plan(query, mode)?;
        let mut all_lists: Vec<Vec<(String, f32)>> = Vec::new();

        for variant in &plan.variants {
            let (vr, lr) = self
                .search_variant(variant, vector_k, lexical_k, &mut lexical, &vector)
                .await?;
            all_lists.push(vr);
            all_lists.push(lr);
        }

        let list_refs: Vec<&[(String, f32)]> = all_lists.iter().map(|l| l.as_slice()).collect();
        let fused = multi_reciprocal_rank_fusion(&list_refs, 60.0);

        let candidate_cap = self.config.retrieval.candidate_pool_size.max(max_chunks);
        let mut ranked = Vec::new();
        for (chunk_id, score, src_type) in fused {
            if exclude_chunk_ids.contains(&chunk_id) {
                continue;
            }
            if let Some(chunk) = self.store.get_chunk(&chunk_id)? {
                ranked.push(RankedChunk {
                    chunk,
                    score,
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

        ranked = Self::dedup_chunks(ranked);
        ranked = Self::apply_filters(ranked, filters);

        if iterative && RetrievalAgent::needs_second_pass(query, &ranked) {
            let second_queries = RetrievalAgent::second_pass_queries(&plan);
            let mut second_lists: Vec<Vec<(String, f32)>> = Vec::new();
            for variant in &second_queries {
                let (vr, lr) = self
                    .search_variant(variant, vector_k, lexical_k, &mut lexical, &vector)
                    .await?;
                second_lists.push(vr);
                second_lists.push(lr);
            }
            if !second_lists.is_empty() {
                let refs: Vec<&[(String, f32)]> =
                    second_lists.iter().map(|l| l.as_slice()).collect();
                let second_fused = multi_reciprocal_rank_fusion(&refs, 60.0);
                let mut second_ranked = Vec::new();
                for (chunk_id, score, src_type) in second_fused {
                    if let Some(chunk) = self.store.get_chunk(&chunk_id)? {
                        second_ranked.push(RankedChunk {
                            chunk,
                            score,
                            source_type: src_type.as_str().to_string(),
                            source_number: None,
                            rerank_score: None,
                            expanded_context: None,
                        });
                    }
                }
                ranked = RetrievalAgent::merge_results(ranked, second_ranked, candidate_cap);
            }
        }

        if rerank {
            ranked = self.reranker.rerank(query, ranked, max_chunks)?;
        } else {
            ranked.truncate(max_chunks);
        }

        ranked = self.context_expander.expand(ranked)?;
        ranked = Self::apply_budget(ranked, self.max_context_tokens);
        ranked = Self::stable_order(ranked);

        Ok((ranked, plan.variants, None))
    }
}

use crate::vectorstore::UsearchVectorIndex;

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
