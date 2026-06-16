use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    pub document_id: String,
    pub parent_section_id: Option<String>,
    pub title: String,
    pub level: u32,
    pub heading_path: String,
    pub start_offset: usize,
    pub end_offset: usize,
    pub page_start: Option<u32>,
    pub page_end: Option<u32>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTable {
    pub id: String,
    pub document_id: String,
    pub section_id: Option<String>,
    pub caption: Option<String>,
    pub headers: Vec<String>,
    pub row_data: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityKind {
    Acronym,
    PartNumber,
    FieldId,
    ErrorCode,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub document_id: String,
    pub chunk_id: Option<String>,
    pub kind: EntityKind,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationKind {
    Neighbor,
    ParentSection,
    SameTable,
    CitationOf,
    SharedEntity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRelation {
    pub from_chunk_id: String,
    pub to_chunk_id: String,
    pub kind: RelationKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SummaryLevel {
    Document,
    Section,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: String,
    pub document_id: String,
    pub section_id: Option<String>,
    pub level: SummaryLevel,
    pub summary: String,
}
