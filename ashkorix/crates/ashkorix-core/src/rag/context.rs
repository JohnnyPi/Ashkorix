use crate::documents::storage::DocumentStore;
use crate::error::Result;
use crate::rag::types::RankedChunk;
use std::sync::Arc;

pub struct ContextExpander {
    store: Arc<DocumentStore>,
    budget_tokens: u32,
}

impl ContextExpander {
    pub fn new(store: Arc<DocumentStore>, budget_tokens: u32) -> Self {
        Self {
            store,
            budget_tokens,
        }
    }

    pub fn expand(&self, mut chunks: Vec<RankedChunk>) -> Result<Vec<RankedChunk>> {
        let mut total = 0u32;
        for chunk in &mut chunks {
            if total >= self.budget_tokens {
                break;
            }
            let mut parts = Vec::new();

            if let Some(ref path) = chunk.chunk.heading_path {
                parts.push(format!("Heading path: {path}"));
            } else if let Some(ref title) = chunk.chunk.section_title {
                parts.push(format!("Section: {title}"));
            }

            if let Some(ref section_id) = chunk.chunk.parent_section_id {
                if let Some(section) = self.store.get_section(section_id)? {
                    let section_text = self.section_excerpt(&section.document_id, section.start_offset, section.end_offset, 200);
                    if !section_text.is_empty() {
                        parts.push(format!("Parent section: {section_text}"));
                    }
                    if let Some(ref summary) = section.summary {
                        parts.push(format!("Section summary: {summary}"));
                    }
                }
            }

            if let Some(ref prev_id) = chunk.chunk.prev_chunk_id {
                if let Some(prev) = self.store.get_chunk(prev_id)? {
                    let preview: String = prev.text.chars().take(150).collect();
                    parts.push(format!("Previous context: {preview}"));
                }
            }
            if let Some(ref next_id) = chunk.chunk.next_chunk_id {
                if let Some(next) = self.store.get_chunk(next_id)? {
                    let preview: String = next.text.chars().take(150).collect();
                    parts.push(format!("Next context: {preview}"));
                }
            }

            if let Some(ref table_id) = chunk.chunk.table_id {
                if let Some(table) = self.store.get_table(table_id)? {
                    if let Some(ref caption) = table.caption {
                        parts.push(format!("Table: {caption}"));
                    }
                    if !table.headers.is_empty() {
                        parts.push(format!("Headers: {}", table.headers.join(", ")));
                    }
                }
            }

            if let Some(doc) = self.store.get_document(&chunk.chunk.document_id.0)? {
                if let Some(ref title) = doc.title {
                    parts.push(format!("Document: {title}"));
                }
                let summaries = self.store.list_document_summaries(&doc.id.0)?;
                if let Some(doc_summary) = summaries.iter().find(|s| s.section_id.is_none()) {
                    parts.push(format!("Document summary: {}", doc_summary.summary));
                }
            }

            if let Some(ref page) = chunk.chunk.page_number {
                parts.push(format!("Page: {page}"));
            }

            if let Some(ref score) = chunk.rerank_score {
                parts.push(format!(
                    "Confidence: rerank_score={score:.3}, matched={}",
                    chunk.source_type
                ));
            }

            let expanded = parts.join("\n");
            let tokens = (expanded.len() / 4) as u32;
            if total + tokens <= self.budget_tokens {
                chunk.expanded_context = Some(expanded);
                total += tokens;
            }
        }
        Ok(chunks)
    }

    fn section_excerpt(
        &self,
        document_id: &str,
        start: usize,
        end: usize,
        max_chars: usize,
    ) -> String {
        if let Ok(Some(doc)) = self.store.get_document(document_id) {
            let excerpt = doc
                .extracted_text
                .get(start..end.min(start + max_chars))
                .unwrap_or("");
            return excerpt.chars().take(max_chars).collect();
        }
        String::new()
    }
}
