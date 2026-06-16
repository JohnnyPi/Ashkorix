use crate::cite::DefaultCitationService;
use crate::pool::KNOWLEDGE_BASE_NAME;
use crate::error::Result;
use crate::llm::LlamaModelService;
use crate::rag::prompt::{build_source_blocks, DefaultPromptBuilder};
use crate::rag::types::{RagAnswer, RetrievalFilters, RetrievalMode};
use crate::rag::verify::CitationVerifier;
use crate::rag::HybridRetrievalService;
use crate::traits::model::{ChatMessage, GenerateParams};
use crate::traits::{CitationService, ModelService, PromptBuilder};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct RagAnswerService {
    retrieval: HybridRetrievalService,
    model: Arc<Mutex<LlamaModelService>>,
    prompt_builder: DefaultPromptBuilder,
    citation_service: DefaultCitationService,
}

impl RagAnswerService {
    pub fn new(
        retrieval: HybridRetrievalService,
        model: Arc<Mutex<LlamaModelService>>,
    ) -> Self {
        Self {
            retrieval,
            model,
            prompt_builder: DefaultPromptBuilder,
            citation_service: DefaultCitationService,
        }
    }

    pub async fn ask(
        &self,
        question: &str,
        mode: RetrievalMode,
        gen_params: GenerateParams,
        exclude_chunk_ids: &[String],
        conversation: &[ChatMessage],
        filters: &RetrievalFilters,
    ) -> Result<RagAnswer> {
        let (chunks, query_variants, corpus_map) = self
            .retrieval
            .retrieve_with_options(question, mode, exclude_chunk_ids, filters)
            .await?;

        if corpus_map.is_some() {
            return Ok(RagAnswer {
                text: "Corpus Map mode returns structured overview; use the corpus_map field."
                    .into(),
                citations: vec![],
                dangling_citations: vec![],
                uncited_warning: false,
                unsupported_claims: vec![],
                retrieved_chunks: vec![],
                prompt: String::new(),
                query_variants,
                corpus_map,
            });
        }

        let sources = build_source_blocks(&chunks);
        let prompt = self
            .prompt_builder
            .build_rag_prompt(question, &sources, conversation);

        let citations = self
            .citation_service
            .assemble_sources(&chunks, KNOWLEDGE_BASE_NAME);

        let mut model = self.model.lock().await;
        let mut stream = model.generate_stream(GenerateParams {
            prompt: prompt.clone(),
            ..gen_params
        })?;

        let mut text = String::new();
        while let Some(event) = stream.next().await {
            let event = event?;
            text.push_str(&event.token);
            if event.finished {
                break;
            }
        }

        let parse_result = self.citation_service.parse_markers(&text, &citations);
        let unsupported_claims =
            CitationVerifier::verify(&text, &parse_result.resolved, &chunks);

        Ok(RagAnswer {
            text,
            citations: parse_result.resolved,
            dangling_citations: parse_result.dangling,
            uncited_warning: parse_result.uncited_warning,
            unsupported_claims,
            retrieved_chunks: chunks,
            prompt,
            query_variants,
            corpus_map: None,
        })
    }

    pub async fn ask_stream(
        &self,
        question: &str,
        mode: RetrievalMode,
        _gen_params: GenerateParams,
        exclude_chunk_ids: &[String],
        conversation: &[ChatMessage],
        filters: &RetrievalFilters,
    ) -> Result<(String, Vec<crate::rag::types::RankedChunk>, Vec<crate::cite::types::Citation>)>
    {
        let (chunks, _, _) = self
            .retrieval
            .retrieve_with_options(question, mode, exclude_chunk_ids, filters)
            .await?;

        let sources = build_source_blocks(&chunks);
        let prompt = self
            .prompt_builder
            .build_rag_prompt(question, &sources, conversation);

        let citations = self
            .citation_service
            .assemble_sources(&chunks, KNOWLEDGE_BASE_NAME);

        Ok((prompt, chunks, citations))
    }
}
