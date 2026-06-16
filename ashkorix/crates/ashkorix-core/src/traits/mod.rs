use crate::chunking::types::Chunk;
use crate::documents::types::Document;
use crate::error::Result;
use crate::rag::types::{RankedChunk, RetrievalMode};
use async_trait::async_trait;
use futures::Stream;
use std::path::Path;
use std::pin::Pin;

pub mod model;

#[async_trait]
pub trait ModelService: Send + Sync {
    async fn load(&mut self, path: &Path, options: model::LoadOptions) -> Result<()>;
    async fn unload(&mut self) -> Result<()>;
    fn is_loaded(&self) -> Option<model::ModelInfo>;
    fn generate_stream(
        &mut self,
        params: model::GenerateParams,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<model::TokenEvent>> + Send>>>;
    fn cancel(&self);
    fn format_prompt(&self, messages: &[model::ChatMessage]) -> Result<String>;
}

#[async_trait]
pub trait DocumentImporter: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn supported_extensions(&self) -> &[&str];
    fn can_handle(&self, path: &Path) -> bool;
    async fn import(&self, path: &Path) -> Result<Document>;
}

pub trait Chunker: Send + Sync {
    fn chunk(&self, document: &Document, collection_id: &str) -> Result<Vec<Chunk>>;
}

#[async_trait]
pub trait EmbeddingService: Send + Sync {
    async fn load(&mut self, path: &Path) -> Result<()>;
    fn is_loaded(&self) -> bool;
    fn dimension(&self) -> usize;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

pub trait VectorIndex: Send + Sync {
    fn upsert(&mut self, chunk_id: &str, vector: &[f32]) -> Result<()>;
    fn search(&self, query: &[f32], top_k: usize) -> Result<Vec<(String, f32)>>;
    fn remove_collection(&mut self) -> Result<()>;
    fn save(&self) -> Result<()>;
    fn len(&self) -> usize;
}

pub trait LexicalIndex: Send + Sync {
    fn index_chunk(&mut self, chunk: &Chunk) -> Result<()>;
    fn search(&self, query: &str, top_k: usize) -> Result<Vec<(String, f32)>>;
    fn remove_collection(&mut self) -> Result<()>;
    fn commit(&mut self) -> Result<()>;
    fn doc_count(&self) -> usize;
}

#[async_trait]
pub trait RetrievalService: Send + Sync {
    async fn retrieve(
        &self,
        query: &str,
        mode: RetrievalMode,
        exclude_chunk_ids: &[String],
        filters: &crate::rag::types::RetrievalFilters,
    ) -> Result<Vec<RankedChunk>>;
}

pub trait Reranker: Send + Sync {
    fn rerank(&self, query: &str, chunks: Vec<RankedChunk>) -> Result<Vec<RankedChunk>>;
}

pub trait PromptBuilder: Send + Sync {
    fn build_rag_prompt(
        &self,
        question: &str,
        sources: &[crate::cite::types::SourceBlock],
        conversation: &[model::ChatMessage],
    ) -> String;
}

pub trait CitationService: Send + Sync {
    fn assemble_sources(&self, chunks: &[RankedChunk], collection_name: &str)
        -> Vec<crate::cite::types::Citation>;
    fn parse_markers(
        &self,
        response: &str,
        citations: &[crate::cite::types::Citation],
    ) -> crate::cite::types::CitationParseResult;
}

pub trait ExtensionHost: Send + Sync {
    fn discover(&mut self) -> Result<()>;
    fn list_extensions(&self) -> Vec<crate::extensions::types::ExtensionInfo>;
    fn is_importer_enabled(&self, importer_id: &str) -> bool;
    fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<()>;
}
