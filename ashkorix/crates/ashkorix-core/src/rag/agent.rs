use crate::error::Result;
use crate::rag::types::{
    CorpusMapResult, EntityFrequency, RankedChunk, RetrievalMode, SectionEntry, ThemeEntry,
};
use crate::search::query_plan::QueryPlan;

pub struct RetrievalAgent;

impl RetrievalAgent {
    pub fn needs_second_pass(query: &str, chunks: &[RankedChunk]) -> bool {
        if chunks.is_empty() {
            return true;
        }
        let query_lower = query.to_lowercase();
        let terms: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|t| t.len() > 3)
            .collect();
        if terms.is_empty() {
            return false;
        }
        let covered = terms.iter().filter(|term| {
            chunks.iter().any(|c| {
                c.chunk.text.to_lowercase().contains(*term)
                    || c.chunk
                        .heading_path
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(*term)
            })
        }).count();
        covered < terms.len() / 2
    }

    pub fn second_pass_queries(plan: &QueryPlan) -> Vec<String> {
        plan.variants.iter().skip(1).cloned().collect()
    }

    pub fn merge_results(
        first: Vec<RankedChunk>,
        second: Vec<RankedChunk>,
        max: usize,
    ) -> Vec<RankedChunk> {
        let mut seen = std::collections::HashSet::new();
        let mut merged = Vec::new();
        for chunk in first.into_iter().chain(second) {
            if seen.insert(chunk.chunk.content_hash.clone()) {
                merged.push(chunk);
            }
            if merged.len() >= max {
                break;
            }
        }
        merged
    }
}

pub fn build_corpus_map(
    store: &crate::documents::storage::DocumentStore,
    query: &str,
    chunks: Vec<RankedChunk>,
) -> Result<CorpusMapResult> {
    let summaries = store.list_all_document_summaries()?;
    let mut themes = Vec::new();
    for summary in &summaries {
        if summary.section_id.is_none() {
            if let Some(doc) = store.get_document(&summary.document_id)? {
                themes.push(ThemeEntry {
                    document_id: summary.document_id.clone(),
                    title: doc
                        .title
                        .clone()
                        .unwrap_or_else(|| doc.original_filename.clone()),
                    summary: summary.summary.clone(),
                });
            }
        }
    }

    let entities_raw = store.list_entities()?;
    let mut entity_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for e in entities_raw {
        *entity_counts.entry(e.value).or_insert(0) += 1;
    }
    let mut entities: Vec<EntityFrequency> = entity_counts
        .into_iter()
        .map(|(value, count)| EntityFrequency { value, count })
        .collect();
    entities.sort_by(|a, b| b.count.cmp(&a.count));
    entities.truncate(30);

    let mut sections = Vec::new();
    for doc in store.list_documents()? {
        for sec in store.list_sections_for_document(&doc.id.0)? {
            sections.push(SectionEntry {
                document_id: doc.id.0.clone(),
                heading_path: sec.heading_path,
                summary: sec.summary,
            });
        }
    }

    let query_lower = query.to_lowercase();
    let related: Vec<RankedChunk> = if query_lower.is_empty() {
        chunks
    } else {
        chunks
            .into_iter()
            .filter(|c| {
                c.chunk.text.to_lowercase().contains(&query_lower)
                    || c.chunk
                        .heading_path
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query_lower)
            })
            .take(20)
            .collect()
    };

    Ok(CorpusMapResult {
        themes,
        entities,
        sections,
        related_chunks: related,
    })
}

pub fn supports_corpus_map(mode: RetrievalMode) -> bool {
    matches!(mode, RetrievalMode::CorpusMap)
}
