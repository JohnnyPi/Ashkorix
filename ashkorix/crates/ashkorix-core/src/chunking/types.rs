use serde::{Deserialize, Serialize};

use crate::types::{ChunkId, CollectionId, DocumentId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: ChunkId,
    pub document_id: DocumentId,
    pub collection_id: CollectionId,
    pub text: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub page_number: Option<u32>,
    pub section_title: Option<String>,
    pub row_sheet_info: Option<String>,
    pub source_filename: String,
    pub content_hash: String,
    pub token_count: u32,
    pub parent_section_id: Option<String>,
    pub heading_path: Option<String>,
    pub chunk_index: u32,
    pub prev_chunk_id: Option<String>,
    pub next_chunk_id: Option<String>,
    pub contextual_text: Option<String>,
    pub table_id: Option<String>,
    pub entity_tokens: Option<String>,
}

impl Chunk {
    pub fn build_contextual_text(&self, doc_title: &str) -> String {
        let section = self
            .heading_path
            .as_deref()
            .or(self.section_title.as_deref())
            .unwrap_or("General");
        format!(
            "Document: {doc_title}. Section: {section}. {}",
            self.text
        )
    }
}

#[derive(Debug, Clone)]
pub struct ChunkingOutput {
    pub chunks: Vec<Chunk>,
}
