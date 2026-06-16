use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceBlock {
    pub number: u32,
    pub filename: String,
    pub page_number: Option<u32>,
    pub section_title: Option<String>,
    pub heading_path: Option<String>,
    pub table_caption: Option<String>,
    pub neighbor_context: Option<String>,
    pub confidence: Option<String>,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub source_number: u32,
    pub document_id: String,
    pub original_filename: String,
    pub page_number: Option<u32>,
    pub section_title: Option<String>,
    pub chunk_preview: String,
    pub score: f64,
    pub collection_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationParseResult {
    pub resolved: Vec<Citation>,
    pub dangling: Vec<u32>,
    pub uncited_warning: bool,
}
