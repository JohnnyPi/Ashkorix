use crate::cite::types::Citation;
use crate::cite::DefaultCitationService;
use crate::memory::types::Memory;
use crate::pool::KNOWLEDGE_BASE_NAME;
use crate::error::Result;
use crate::llm::LlamaModelService;
use crate::rag::prompt::{build_rag_messages, build_source_blocks};
use crate::rag::types::{RagAnswer, RankedChunk, RetrievalFilters, RetrievalMode, UnsupportedClaim};
use crate::rag::verify::CitationVerifier;
use crate::rag::HybridRetrievalService;
use crate::traits::model::{ChatMessage, GenerateParams, TokenEvent};
use crate::traits::{CitationService, ModelService};
use futures::Stream;
use futures::StreamExt;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RagStreamMeta {
    pub citations: Vec<Citation>,
    pub retrieved_chunks: Vec<RankedChunk>,
    pub query_variants: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RagVerification {
    pub resolved_citations: Vec<Citation>,
    pub dangling_citations: Vec<u32>,
    pub uncited_warning: bool,
    pub unsupported_claims: Vec<UnsupportedClaim>,
}

pub struct RagAnswerService {
    retrieval: Arc<HybridRetrievalService>,
    model: Arc<Mutex<LlamaModelService>>,
    citation_service: DefaultCitationService,
}

impl RagAnswerService {
    pub fn new(
        retrieval: Arc<HybridRetrievalService>,
        model: Arc<Mutex<LlamaModelService>>,
    ) -> Self {
        Self {
            retrieval,
            model,
            citation_service: DefaultCitationService,
        }
    }

    pub fn verify_response(
        text: &str,
        citations: &[Citation],
        chunks: &[RankedChunk],
        memories: &[Memory],
    ) -> RagVerification {
        let parse_result = DefaultCitationService.parse_markers(text, citations);
        let has_memory_citations = Self::has_memory_citations(text);
        let uncited_warning = (!citations.is_empty() || !memories.is_empty())
            && parse_result.resolved.is_empty()
            && !has_memory_citations;
        let unsupported_claims = CitationVerifier::verify(text, citations, chunks, memories);
        RagVerification {
            resolved_citations: parse_result.resolved,
            dangling_citations: parse_result.dangling,
            uncited_warning,
            unsupported_claims,
        }
    }

    fn has_memory_citations(text: &str) -> bool {
        regex::Regex::new(r"\[Memory\s+\d+\]")
            .map(|re| re.is_match(text))
            .unwrap_or(false)
    }

    pub async fn ask(
        &self,
        question: &str,
        mode: RetrievalMode,
        gen_params: GenerateParams,
        exclude_chunk_ids: &[String],
        conversation: &[ChatMessage],
        filters: &RetrievalFilters,
        memories: &[Memory],
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
        let messages = build_rag_messages(question, &sources, conversation, memories);

        let citations = self
            .citation_service
            .assemble_sources(&chunks, KNOWLEDGE_BASE_NAME);

        let mut model = self.model.lock().await;
        let prompt = model.format_prompt(&messages)?;
        let mut stream = model.generate_stream(rag_generation_params(
            GenerateParams {
                prompt: prompt.clone(),
                ..gen_params
            },
        ))?;

        let mut text = String::new();
        while let Some(event) = stream.next().await {
            let event = event?;
            text.push_str(&event.token);
            if event.finished {
                break;
            }
        }

        let verification = Self::verify_response(&text, &citations, &chunks, memories);

        Ok(RagAnswer {
            text,
            citations: verification.resolved_citations,
            dangling_citations: verification.dangling_citations,
            uncited_warning: verification.uncited_warning,
            unsupported_claims: verification.unsupported_claims,
            retrieved_chunks: chunks,
            prompt,
            query_variants,
            corpus_map: None,
        })
    }

    pub async fn stream_answer(
        &self,
        question: &str,
        mode: RetrievalMode,
        gen_params: GenerateParams,
        exclude_chunk_ids: &[String],
        conversation: &[ChatMessage],
        filters: &RetrievalFilters,
        memories: &[Memory],
    ) -> Result<(
        Pin<Box<dyn Stream<Item = Result<TokenEvent>> + Send>>,
        RagStreamMeta,
    )> {
        let (chunks, query_variants, corpus_map) = self
            .retrieval
            .retrieve_with_options(question, mode, exclude_chunk_ids, filters)
            .await?;

        if corpus_map.is_some() {
            return Err(crate::error::AshkorixError::Config(
                "Corpus Map mode is not supported for streaming chat.".into(),
            ));
        }

        let sources = build_source_blocks(&chunks);
        let messages = build_rag_messages(question, &sources, conversation, memories);

        let citations = self
            .citation_service
            .assemble_sources(&chunks, KNOWLEDGE_BASE_NAME);

        let mut model = self.model.lock().await;
        let prompt = model.format_prompt(&messages)?;
        let stream = model.generate_stream(rag_generation_params(GenerateParams {
            prompt,
            ..gen_params
        }))?;

        Ok((
            stream,
            RagStreamMeta {
                citations,
                retrieved_chunks: chunks,
                query_variants,
            },
        ))
    }
}

fn rag_generation_params(params: GenerateParams) -> GenerateParams {
    GenerateParams {
        temperature: params.temperature.min(0.25),
        top_p: params.top_p.min(0.9),
        ..params
    }
}
