use crate::chunking::recursive::{merge_to_size, split_by_regex, window_chunk};
use crate::chunking::util::estimate_tokens;
use crate::chunking::types::Chunk;
use crate::config::ChunkingConfig;
use crate::documents::entities::extract_entities_from_text;
use crate::documents::graph_types::{ChunkRelation, RelationKind, StoredTable};
use crate::documents::structure::{build_section_tree, find_section_for_offset, sections_to_stored};
use crate::documents::types::Document;
use crate::error::Result;
use crate::traits::Chunker;
use crate::types::{hash_text, short_id_from_hash, ChunkId, CollectionId};

pub struct HeadingHierarchyChunker {
    config: ChunkingConfig,
}

impl HeadingHierarchyChunker {
    pub fn new(config: ChunkingConfig) -> Self {
        Self { config }
    }

    fn max_chars(&self) -> usize {
        self.config.max_tokens as usize * 4
    }

    fn overlap_chars(&self) -> usize {
        self.config.overlap_tokens as usize * 4
    }

    fn split_section_text(text: &str, max_chars: usize, overlap: usize) -> Vec<String> {
        if text.len() <= max_chars {
            return vec![text.to_string()];
        }
        if text.contains("\n\n") {
            let parts: Vec<String> = text.split("\n\n").map(String::from).collect();
            if parts.len() > 1 {
                return parts
                    .into_iter()
                    .flat_map(|p| {
                        if p.len() <= max_chars {
                            vec![p]
                        } else {
                            Self::split_section_text(&p, max_chars, overlap)
                        }
                    })
                    .collect();
            }
        }
        if let Some(parts) = split_by_regex(text, r"(?<=[.!?])\s+") {
            if parts.len() > 1 {
                return merge_to_size(parts, max_chars);
            }
        }
        window_chunk(text, max_chars, overlap)
    }
}

impl Chunker for HeadingHierarchyChunker {
    fn chunk(&self, document: &Document, collection_id: &str) -> Result<Vec<Chunk>> {
        Ok(self
            .chunk_with_graph(document, collection_id)?
            .chunks)
    }
}

impl HeadingHierarchyChunker {
    pub fn chunk_with_graph(
        &self,
        document: &Document,
        collection_id: &str,
    ) -> Result<HierarchyChunkResult> {
        let section_nodes = build_section_tree(document);
        let sections = sections_to_stored(&document.id.0, &section_nodes);
        let doc_title = document
            .title
            .clone()
            .unwrap_or_else(|| document.original_filename.clone());

        let max_chars = self.max_chars();
        let overlap = self.overlap_chars();
        let mut chunks = Vec::new();
        let mut tables = Vec::new();
        let mut entities = Vec::new();
        let mut chunk_index = 0u32;

        for (table_idx, table) in document.extracted_tables.iter().enumerate() {
            let table_id = format!(
                "{}-tbl-{}",
                &document.id.0[..document.id.0.len().min(8)],
                table_idx
            );
            let caption = table.name.clone();
            let row_text: Vec<String> = table
                .rows
                .iter()
                .map(|row| {
                    table
                        .headers
                        .iter()
                        .zip(row.iter())
                        .map(|(h, v)| format!("{h}: {v}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .collect();
            for (row_i, row_line) in row_text.iter().enumerate() {
                if row_line.trim().is_empty() {
                    continue;
                }
                let content_hash = hash_text(row_line);
                let id = ChunkId(format!(
                    "{}-{}",
                    short_id_from_hash(&document.content_hash),
                    short_id_from_hash(&format!("{table_id}{row_i}{content_hash}"))
                ));
                let token_count = estimate_tokens(row_line);
                let contextual_text = format!(
                    "Document: {doc_title}. Table: {}. Row: {row_line}",
                    caption.as_deref().unwrap_or("Table")
                );
                chunks.push(Chunk {
                    id: id.clone(),
                    document_id: document.id.clone(),
                    collection_id: CollectionId(collection_id.to_string()),
                    text: row_line.clone(),
                    start_offset: 0,
                    end_offset: row_line.len(),
                    page_number: None,
                    section_title: caption.clone(),
                    row_sheet_info: caption.clone(),
                    source_filename: document.original_filename.clone(),
                    content_hash,
                    token_count,
                    parent_section_id: None,
                    heading_path: caption.clone(),
                    chunk_index,
                    prev_chunk_id: None,
                    next_chunk_id: None,
                    contextual_text: Some(contextual_text),
                    table_id: Some(table_id.clone()),
                    entity_tokens: None,
                });
                entities.extend(extract_entities_from_text(
                    &document.id.0,
                    &id.0,
                    row_line,
                ));
                chunk_index += 1;
            }
            tables.push(StoredTable {
                id: table_id,
                document_id: document.id.0.clone(),
                section_id: None,
                caption,
                headers: table.headers.clone(),
                row_data: table.rows.clone(),
            });
        }

        for section in &sections {
            let body = &document.extracted_text[section.start_offset..section.end_offset];
            let segments = Self::split_section_text(body, max_chars, overlap);
            let mut local_offset = section.start_offset;

            for segment in segments {
                if segment.trim().is_empty() {
                    local_offset += segment.len();
                    continue;
                }
                let start = local_offset;
                let end = local_offset + segment.len();
                let content_hash = hash_text(&segment);
                let id = ChunkId(format!(
                    "{}-{}",
                    short_id_from_hash(&document.content_hash),
                    short_id_from_hash(&content_hash)
                ));
                let token_count = estimate_tokens(&segment);
                let mut chunk = Chunk {
                    id: id.clone(),
                    document_id: document.id.clone(),
                    collection_id: CollectionId(collection_id.to_string()),
                    text: segment.clone(),
                    start_offset: start,
                    end_offset: end,
                    page_number: section.page_start,
                    section_title: Some(section.title.clone()),
                    row_sheet_info: None,
                    source_filename: document.original_filename.clone(),
                    content_hash,
                    token_count,
                    parent_section_id: Some(section.id.clone()),
                    heading_path: Some(section.heading_path.clone()),
                    chunk_index,
                    prev_chunk_id: None,
                    next_chunk_id: None,
                    contextual_text: None,
                    table_id: None,
                    entity_tokens: None,
                };
                chunk.contextual_text = Some(chunk.build_contextual_text(&doc_title));
                let entity_list =
                    extract_entities_from_text(&document.id.0, &id.0, &segment);
                if !entity_list.is_empty() {
                    chunk.entity_tokens = Some(
                        entity_list
                            .iter()
                            .map(|e| e.value.as_str())
                            .collect::<Vec<_>>()
                            .join(" "),
                    );
                }
                entities.extend(entity_list);
                chunks.push(chunk);
                chunk_index += 1;
                local_offset = end;
            }
        }

        if chunks.is_empty() && !document.extracted_text.is_empty() {
            let text = &document.extracted_text;
            let segments = Self::split_section_text(text, max_chars, overlap);
            let mut offset = 0usize;
            for segment in segments {
                if segment.trim().is_empty() {
                    offset += segment.len();
                    continue;
                }
                let content_hash = hash_text(&segment);
                let id = ChunkId(format!(
                    "{}-{}",
                    short_id_from_hash(&document.content_hash),
                    short_id_from_hash(&content_hash)
                ));
                let section = find_section_for_offset(&sections, offset);
                let mut chunk = Chunk {
                    id: id.clone(),
                    document_id: document.id.clone(),
                    collection_id: CollectionId(collection_id.to_string()),
                    text: segment.clone(),
                    start_offset: offset,
                    end_offset: offset + segment.len(),
                    page_number: None,
                    section_title: section.map(|s| s.title.clone()),
                    row_sheet_info: None,
                    source_filename: document.original_filename.clone(),
                    content_hash,
                    token_count: estimate_tokens(&segment),
                    parent_section_id: section.map(|s| s.id.clone()),
                    heading_path: section.map(|s| s.heading_path.clone()),
                    chunk_index,
                    prev_chunk_id: None,
                    next_chunk_id: None,
                    contextual_text: None,
                    table_id: None,
                    entity_tokens: None,
                };
                chunk.contextual_text = Some(chunk.build_contextual_text(&doc_title));
                entities.extend(extract_entities_from_text(
                    &document.id.0,
                    &id.0,
                    &segment,
                ));
                chunks.push(chunk);
                chunk_index += 1;
                offset += segment.len();
            }
        }

        link_neighbors(&mut chunks);
        let relations = build_neighbor_relations(&chunks);

        Ok(HierarchyChunkResult {
            chunks,
            sections,
            tables,
            entities,
            relations,
        })
    }
}

#[derive(Debug, Clone)]
pub struct HierarchyChunkResult {
    pub chunks: Vec<Chunk>,
    pub sections: Vec<crate::documents::graph_types::Section>,
    pub tables: Vec<StoredTable>,
    pub entities: Vec<crate::documents::graph_types::Entity>,
    pub relations: Vec<ChunkRelation>,
}

fn link_neighbors(chunks: &mut [Chunk]) {
    for i in 0..chunks.len() {
        if i > 0 {
            chunks[i].prev_chunk_id = Some(chunks[i - 1].id.0.clone());
        }
        if i + 1 < chunks.len() {
            chunks[i].next_chunk_id = Some(chunks[i + 1].id.0.clone());
        }
    }
}

fn build_neighbor_relations(chunks: &[Chunk]) -> Vec<ChunkRelation> {
    let mut relations = Vec::new();
    for chunk in chunks {
        if let Some(ref prev) = chunk.prev_chunk_id {
            relations.push(ChunkRelation {
                from_chunk_id: chunk.id.0.clone(),
                to_chunk_id: prev.clone(),
                kind: RelationKind::Neighbor,
            });
        }
        if let Some(ref next) = chunk.next_chunk_id {
            relations.push(ChunkRelation {
                from_chunk_id: chunk.id.0.clone(),
                to_chunk_id: next.clone(),
                kind: RelationKind::Neighbor,
            });
        }
        if let Some(ref section_id) = chunk.parent_section_id {
            relations.push(ChunkRelation {
                from_chunk_id: chunk.id.0.clone(),
                to_chunk_id: section_id.clone(),
                kind: RelationKind::ParentSection,
            });
        }
    }
    relations
}
