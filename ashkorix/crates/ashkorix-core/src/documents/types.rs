use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::types::DocumentId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    Txt,
    Markdown,
    Html,
    Csv,
    Json,
    Xml,
    Pdf,
    Docx,
    Xlsx,
    Unknown,
}

impl FileType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "txt" => Self::Txt,
            "md" | "markdown" => Self::Markdown,
            "html" | "htm" => Self::Html,
            "csv" => Self::Csv,
            "json" => Self::Json,
            "xml" => Self::Xml,
            "pdf" => Self::Pdf,
            "docx" => Self::Docx,
            "xlsx" | "xls" => Self::Xlsx,
            _ => Self::Unknown,
        }
    }

    pub fn badge(&self) -> &'static str {
        match self {
            Self::Txt => "TXT",
            Self::Markdown => "MD",
            Self::Html => "HTML",
            Self::Csv => "CSV",
            Self::Json => "JSON",
            Self::Xml => "XML",
            Self::Pdf => "PDF",
            Self::Docx => "DOCX",
            Self::Xlsx => "XLSX",
            Self::Unknown => "?",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub name: Option<String>,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub content_hash: String,
    pub original_filename: String,
    pub file_path: PathBuf,
    pub file_type: FileType,
    pub collection_id: String,
    pub imported_at: DateTime<Utc>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub created_date: Option<DateTime<Utc>>,
    pub modified_date: Option<DateTime<Utc>>,
    pub extracted_text: String,
    pub extracted_tables: Vec<Table>,
    pub metadata: serde_json::Value,
    pub chunk_count: u32,
    pub import_status: ImportStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImportStatus {
    Imported,
    Duplicate,
    Failed,
    Chunked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub document: Option<Document>,
    pub status: ImportStatus,
    pub message: String,
}
