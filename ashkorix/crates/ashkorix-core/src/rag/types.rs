use serde::{Deserialize, Serialize};

use crate::chunking::types::Chunk;
use crate::search::fusion::SourceType;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RetrievalMode {
    Fast,
    Balanced,
    Thorough,
    Deep,
    CorpusMap,
}

impl RetrievalMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "fast" => Self::Fast,
            "thorough" => Self::Thorough,
            "deep" => Self::Deep,
            "corpus-map" | "corpusmap" | "corpus_map" => Self::CorpusMap,
            _ => Self::Balanced,
        }
    }

    /// (vector_k, lexical_k, rerank, max_chunks, query_variants, iterative)
    pub fn params(&self) -> (usize, usize, bool, usize, usize, bool) {
        match self {
            Self::Fast => (20, 20, false, 5, 1, false),
            Self::Balanced => (50, 50, true, 10, 3, false),
            Self::Thorough => (50, 50, true, 15, 3, false),
            Self::Deep => (50, 50, true, 12, 5, true),
            Self::CorpusMap => (25, 25, false, 0, 1, false),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalFilters {
    pub document_ids: Vec<String>,
    pub file_types: Vec<String>,
    pub page_min: Option<u32>,
    pub page_max: Option<u32>,
    pub section_prefix: Option<String>,
    pub entity_match: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedChunk {
    pub chunk: Chunk,
    pub score: f64,
    pub source_type: String,
    pub source_number: Option<u32>,
    pub rerank_score: Option<f64>,
    pub expanded_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusMapResult {
    pub themes: Vec<ThemeEntry>,
    pub entities: Vec<EntityFrequency>,
    pub sections: Vec<SectionEntry>,
    pub related_chunks: Vec<RankedChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeEntry {
    pub document_id: String,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityFrequency {
    pub value: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionEntry {
    pub document_id: String,
    pub heading_path: String,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsupportedClaim {
    pub sentence: String,
    pub cited_source: Option<u32>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagAnswer {
    pub text: String,
    pub citations: Vec<crate::cite::types::Citation>,
    pub dangling_citations: Vec<u32>,
    pub uncited_warning: bool,
    pub unsupported_claims: Vec<UnsupportedClaim>,
    pub retrieved_chunks: Vec<RankedChunk>,
    pub prompt: String,
    pub query_variants: Vec<String>,
    pub corpus_map: Option<CorpusMapResult>,
}

impl SourceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Vector => "vector",
            Self::Keyword => "keyword",
            Self::Both => "both",
        }
    }
}
