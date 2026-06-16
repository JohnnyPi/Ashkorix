use crate::chunking::types::Chunk;
use crate::config::ChunkingConfig;
use crate::documents::types::Document;
use crate::error::Result;
use crate::traits::Chunker;
use crate::types::{hash_text, short_id_from_hash, ChunkId, CollectionId};

pub struct RecursiveChunker {
    config: ChunkingConfig,
}

impl RecursiveChunker {
    pub fn new(config: ChunkingConfig) -> Self {
        Self { config }
    }

    fn estimate_tokens(text: &str) -> u32 {
        (text.len() / 4).max(1) as u32
    }
}

impl Chunker for RecursiveChunker {
    fn chunk(&self, document: &Document, collection_id: &str) -> Result<Vec<Chunk>> {
        let text = &document.extracted_text;
        let segments = split_recursive(text, self.config.max_tokens as usize * 4);
        let mut chunks = Vec::new();
        let mut offset = 0usize;

        for segment in segments {
            if segment.trim().is_empty() {
                offset += segment.len();
                continue;
            }
            let start = offset;
            let end = offset + segment.len();
            let content_hash = hash_text(&segment);
            let id = ChunkId(format!(
                "{}-{}",
                short_id_from_hash(&document.content_hash),
                short_id_from_hash(&content_hash)
            ));
            let token_count = Self::estimate_tokens(&segment);
            chunks.push(Chunk {
                id,
                document_id: document.id.clone(),
                collection_id: CollectionId(collection_id.to_string()),
                text: segment,
                start_offset: start,
                end_offset: end,
                page_number: None,
                section_title: None,
                row_sheet_info: None,
                source_filename: document.original_filename.clone(),
                content_hash,
                token_count,
                parent_section_id: None,
                heading_path: None,
                chunk_index: chunks.len() as u32,
                prev_chunk_id: None,
                next_chunk_id: None,
                contextual_text: None,
                table_id: None,
                entity_tokens: None,
            });
            offset = end;
        }
        Ok(chunks)
    }
}

fn split_recursive(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }

    // Try headings
    if let Some(parts) = split_by_regex(text, r"(?m)^#{1,6}\s+") {
        if parts.len() > 1 {
            return parts
                .into_iter()
                .flat_map(|p| split_recursive(&p, max_chars))
                .collect();
        }
    }

    // Paragraphs
    if text.contains("\n\n") {
        let parts: Vec<String> = text.split("\n\n").map(String::from).collect();
        if parts.len() > 1 {
            return parts
                .into_iter()
                .flat_map(|p| split_recursive(&p, max_chars))
                .collect();
        }
    }

    // Sentences
    if let Some(parts) = split_by_regex(text, r"(?<=[.!?])\s+") {
        if parts.len() > 1 {
            return merge_to_size(parts, max_chars);
        }
    }

    // Fixed window with overlap
    window_chunk(text, max_chars, max_chars / 8)
}

pub(crate) fn split_by_regex(text: &str, pattern: &str) -> Option<Vec<String>> {
    let re = regex::Regex::new(pattern).ok()?;
    let mut parts = Vec::new();
    let mut last = 0;
    for m in re.find_iter(text) {
        if m.start() > last {
            parts.push(text[last..m.start()].to_string());
        }
        last = m.start();
    }
    if last < text.len() {
        parts.push(text[last..].to_string());
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

pub(crate) fn merge_to_size(parts: Vec<String>, max_chars: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    for part in parts {
        if current.len() + part.len() + 1 > max_chars && !current.is_empty() {
            result.push(current.clone());
            current.clear();
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(&part);
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
}

pub(crate) fn window_chunk(text: &str, size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let end = (start + size).min(text.len());
        chunks.push(text[start..end].to_string());
        if end == text.len() {
            break;
        }
        start = end.saturating_sub(overlap);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_paragraphs() {
        let text = "First paragraph.\n\nSecond paragraph with more text.";
        let parts = split_recursive(text, 30);
        assert!(parts.len() >= 2);
    }
}
