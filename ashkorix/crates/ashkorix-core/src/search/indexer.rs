use crate::config::AshkorixConfig;
use crate::documents::storage::DocumentStore;
use crate::embeddings::LlamaEmbeddingService;
use crate::error::{AshkorixError, Result};
use crate::traits::{EmbeddingService, LexicalIndex, VectorIndex};
use crate::vectorstore::UsearchVectorIndex;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct PoolIndexer {
    config: AshkorixConfig,
    store: Arc<DocumentStore>,
    embedding: Arc<Mutex<LlamaEmbeddingService>>,
    batch_size: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexHealth {
    pub chunk_count: u32,
    pub vector_count: usize,
    pub lexical_count: usize,
    pub indexed: bool,
    pub embedding_loaded: bool,
    pub embedding_model_path: Option<String>,
    pub message: String,
}

impl PoolIndexer {
    pub fn new(
        config: AshkorixConfig,
        store: Arc<DocumentStore>,
        embedding: Arc<Mutex<LlamaEmbeddingService>>,
    ) -> Self {
        Self {
            config,
            store,
            embedding,
            batch_size: 32,
        }
    }

    fn index_paths(&self) -> (PathBuf, PathBuf) {
        let base = self.config.index_dir();
        (base.join("tantivy"), base.join("vectors.usearch"))
    }

    fn embed_text_for_chunk(
        chunk: &crate::chunking::types::Chunk,
        doc_title: &str,
    ) -> String {
        chunk
            .contextual_text
            .clone()
            .unwrap_or_else(|| chunk.build_contextual_text(doc_title))
    }

    pub async fn build_index(&self) -> Result<IndexHealth> {
        let chunks = self.store.list_pool_chunks()?;
        let dim = self.embedding.lock().await.dimension();
        if dim == 0 {
            return Err(AshkorixError::Config(
                "Embedding model is not loaded. In Settings, set embedding model path to a \
                 .gguf embedding file (not a folder), save config, then rebuild the index."
                    .into(),
            ));
        }
        if chunks.is_empty() {
            return Err(AshkorixError::Config(
                "No document chunks to index. Import files on the Documents page first.".into(),
            ));
        }
        let (lex_path, vec_path) = self.index_paths();

        let mut lexical = crate::search::lexical::TantivyLexicalIndex::open(&lex_path)?;
        let mut vector = UsearchVectorIndex::open(&vec_path, dim)?;

        for batch in chunks.chunks(self.batch_size) {
            let mut texts = Vec::new();
            let mut to_embed = Vec::new();
            for chunk in batch {
                lexical.index_chunk(chunk)?;
                let doc_title = self
                    .store
                    .get_document(&chunk.document_id.0)?
                    .and_then(|d| d.title)
                    .unwrap_or_else(|| chunk.source_filename.clone());
                let embed_text = Self::embed_text_for_chunk(chunk, &doc_title);
                let cache_key = format!("{}:ctx", chunk.content_hash);
                if let Some(cached) = self.store.get_cached_embedding(&cache_key)? {
                    vector.upsert(&chunk.id.0, &cached)?;
                } else if let Some(cached) = self.store.get_cached_embedding(&chunk.content_hash)? {
                    vector.upsert(&chunk.id.0, &cached)?;
                } else {
                    texts.push(embed_text);
                    to_embed.push((chunk, cache_key));
                }
            }
            if !texts.is_empty() {
                let embeddings = self.embedding.lock().await.embed_batch(&texts).await?;
                for ((chunk, cache_key), emb) in to_embed.iter().zip(embeddings.iter()) {
                    self.store.cache_embedding(cache_key, emb)?;
                    vector.upsert(&chunk.id.0, emb)?;
                }
            }
        }

        lexical.commit()?;
        drop(lexical);
        vector.save()?;
        drop(vector);
        self.build_summaries()?;
        self.store.mark_pool_indexed()?;

        self.health()
    }

    fn build_summaries(&self) -> Result<()> {
        use crate::documents::graph_types::{DocumentSummary, SummaryLevel};
        use crate::types::{hash_text, short_id_from_hash};

        for doc in self.store.list_documents()? {
            let text = &doc.extracted_text;
            let summary_text: String = text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .take(5)
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(400)
                .collect();
            if summary_text.is_empty() {
                continue;
            }
            let id = format!(
                "sum-{}",
                short_id_from_hash(&hash_text(&format!("doc-{}", doc.id.0)))
            );
            self.store.upsert_document_summary(&DocumentSummary {
                id,
                document_id: doc.id.0.clone(),
                section_id: None,
                level: SummaryLevel::Document,
                summary: summary_text,
            })?;

            for section in self.store.list_sections_for_document(&doc.id.0)? {
                let section_text = doc
                    .extracted_text
                    .get(section.start_offset..section.end_offset)
                    .unwrap_or("");
                let sec_summary: String = section_text
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(250)
                    .collect();
                if sec_summary.is_empty() {
                    continue;
                }
                self.store.update_section_summary(&section.id, &sec_summary)?;
                let sec_id = format!(
                    "sum-{}",
                    short_id_from_hash(&hash_text(&format!("sec-{}", section.id)))
                );
                self.store.upsert_document_summary(&DocumentSummary {
                    id: sec_id,
                    document_id: doc.id.0.clone(),
                    section_id: Some(section.id.clone()),
                    level: SummaryLevel::Section,
                    summary: sec_summary,
                })?;
            }
        }
        Ok(())
    }

    pub async fn rebuild_index(&self) -> Result<IndexHealth> {
        let (lex_path, vec_path) = self.index_paths();
        if lex_path.exists() {
            std::fs::remove_dir_all(&lex_path)?;
        }
        let usearch_file = vec_path.with_extension("usearch");
        if usearch_file.exists() {
            std::fs::remove_file(&usearch_file)?;
        }
        if vec_path.with_extension("usearch.meta").exists() {
            std::fs::remove_file(vec_path.with_extension("usearch.meta"))?;
        }
        if vec_path.exists() {
            std::fs::remove_file(&vec_path)?;
        }
        self.build_index().await
    }

    pub fn health(&self) -> Result<IndexHealth> {
        let chunks = self.store.list_pool_chunks()?;
        let (lex_path, vec_path) = self.index_paths();
        let lexical_count = if lex_path.exists() {
            crate::search::lexical::TantivyLexicalIndex::open(&lex_path)?.doc_count()
        } else {
            0
        };
        let embedding_loaded = futures::executor::block_on(async {
            self.embedding.lock().await.is_loaded()
        });
        let dim = futures::executor::block_on(async { self.embedding.lock().await.dimension() });
        let vector_count = if dim > 0 {
            let usearch_file = vec_path.with_extension("usearch");
            if usearch_file.exists() || vec_path.exists() {
                UsearchVectorIndex::open(&vec_path, dim)
                    .map(|i| i.len())
                    .unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };
        let indexed = self.store.pool_indexed_at()?.is_some();
        let embedding_model_path = self
            .config
            .embedding_model_path
            .as_ref()
            .map(|p| p.display().to_string());

        let message = if !embedding_loaded {
            "Embedding model not loaded — set a .gguf embedding file in Settings and save config."
                .to_string()
        } else if chunks.is_empty() {
            "No documents imported yet.".to_string()
        } else if !indexed {
            if lexical_count > 0 || vector_count > 0 {
                "Index incomplete — rebuild the index after embedding model is loaded.".to_string()
            } else {
                "Not indexed — import documents, then build or rebuild the index.".to_string()
            }
        } else {
            format!(
                "Indexed {} chunks ({} vectors, {} lexical docs).",
                chunks.len(),
                vector_count,
                lexical_count
            )
        };

        Ok(IndexHealth {
            chunk_count: chunks.len() as u32,
            vector_count,
            lexical_count,
            indexed,
            embedding_loaded,
            embedding_model_path,
            message,
        })
    }

    pub fn open_indexes(&self, dimension: usize) -> Result<(crate::search::lexical::TantivyLexicalIndex, UsearchVectorIndex)> {
        let (lex_path, vec_path) = self.index_paths();
        Ok((
            crate::search::lexical::TantivyLexicalIndex::open(&lex_path)?,
            UsearchVectorIndex::open(&vec_path, dimension)?,
        ))
    }
}

pub type CollectionIndexer = PoolIndexer;
