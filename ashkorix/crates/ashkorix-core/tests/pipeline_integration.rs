//! Integration-style tests for storage, filters, and index consistency.

#[cfg(test)]
mod pipeline_tests {
    use ashkorix_core::chunking::util::{estimate_tokens, matches_file_types};
    use ashkorix_core::chunking::HeadingHierarchyChunker;
    use ashkorix_core::chunking::types::Chunk;
    use ashkorix_core::config::ChunkingConfig;
    use ashkorix_core::documents::storage::DocumentStore;
    use ashkorix_core::documents::types::{Document, FileType, ImportStatus};
    use ashkorix_core::rag::types::RankedChunk;
    use ashkorix_core::search::indexer::PoolIndexer;
    use ashkorix_core::traits::{LexicalIndex, VectorIndex};
    use ashkorix_core::types::{ChunkId, CollectionId, DocumentId};
    use chrono::Utc;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn sample_document(id: &str, filename: &str, text: &str) -> Document {
        Document {
            id: DocumentId(id.into()),
            content_hash: format!("hash-{id}"),
            original_filename: filename.into(),
            file_path: PathBuf::from(format!("fixtures/{filename}")),
            file_type: FileType::Markdown,
            collection_id: "pool".into(),
            imported_at: Utc::now(),
            title: Some("Test Doc".into()),
            author: None,
            created_date: None,
            modified_date: None,
            extracted_text: text.to_string(),
            extracted_tables: vec![],
            metadata: serde_json::json!({}),
            chunk_count: 0,
            import_status: ImportStatus::Imported,
        }
    }

    fn ranked_chunk(id: &str, filename: &str) -> RankedChunk {
        RankedChunk {
            chunk: Chunk {
                id: ChunkId(id.into()),
                document_id: DocumentId("doc-1".into()),
                collection_id: CollectionId("pool".into()),
                text: "sample".into(),
                start_offset: 0,
                end_offset: 6,
                page_number: None,
                section_title: None,
                row_sheet_info: None,
                source_filename: filename.into(),
                content_hash: format!("c-{id}"),
                token_count: 2,
                parent_section_id: None,
                heading_path: None,
                chunk_index: 0,
                prev_chunk_id: None,
                next_chunk_id: None,
                contextual_text: None,
                table_id: None,
                entity_tokens: None,
            },
            score: 1.0,
            source_type: "both".into(),
            source_number: None,
            rerank_score: None,
            expanded_context: None,
        }
    }

    #[test]
    fn file_types_filter_matches_extension() {
        assert!(matches_file_types("report.md", &["md".into()]));
        assert!(matches_file_types("report.md", &[".md".into()]));
        assert!(!matches_file_types("report.txt", &["md".into()]));
    }

    #[test]
    fn estimate_tokens_is_nonzero_for_text() {
        assert!(estimate_tokens("hello world") >= 1);
    }

    #[test]
    fn delete_document_removes_chunks_from_store() {
        let dir = TempDir::new().expect("tempdir");
        let db = dir.path().join("test.db");
        let store = DocumentStore::open(&db).expect("open store");
        let doc = sample_document("doc-1", "sample.md", "# Title\n\nBody text.");
        store.insert_document(&doc).expect("insert doc");
        let chunker = HeadingHierarchyChunker::new(ChunkingConfig::default());
        let graph = chunker
            .chunk_with_graph(&doc, "pool")
            .expect("chunk");
        store.insert_chunks(&graph.chunks).expect("insert chunks");
        assert!(!store.list_chunks_for_document("doc-1").unwrap().is_empty());
        store.delete_document("doc-1").expect("delete");
        assert!(store.list_chunks_for_document("doc-1").unwrap().is_empty());
    }

    #[test]
    fn pool_indexer_remove_chunks_updates_vector_and_lexical() {
        let dir = TempDir::new().expect("tempdir");
        let data_dir = dir.path().to_path_buf();
        let mut config = ashkorix_core::config::AshkorixConfig::default();
        config.data_dir = data_dir.clone();
        config.ensure_dirs().expect("dirs");

        let db = data_dir.join("ashkorix.db");
        let store = Arc::new(DocumentStore::open(&db).expect("open store"));
        let embedding = Arc::new(tokio::sync::Mutex::new(
            ashkorix_core::embeddings::LlamaEmbeddingService::new().expect("embed svc"),
        ));
        let indexer = PoolIndexer::new(config.clone(), store.clone(), embedding.clone());

        let (lex_path, vec_path) = (
            config.index_dir().join("tantivy"),
            config.index_dir().join("vectors.usearch"),
        );
        let mut lexical =
            ashkorix_core::search::lexical::TantivyLexicalIndex::open(&lex_path).expect("lex");
        let mut vector = ashkorix_core::vectorstore::UsearchVectorIndex::open(&vec_path, 4)
            .expect("vec");

        let chunk = ranked_chunk("chunk-a", "a.md").chunk;
        lexical.index_chunk(&chunk).expect("lex index");
        vector.upsert("chunk-a", &[0.1, 0.2, 0.3, 0.4]).expect("vec upsert");
        lexical.commit().expect("commit");
        vector.save().expect("save");
        drop(lexical);
        drop(vector);

        indexer
            .remove_chunks(&["chunk-a".into()], 4)
            .expect("remove");

        let reopened_vec =
            ashkorix_core::vectorstore::UsearchVectorIndex::open(&vec_path, 4).expect("reopen");
        assert_eq!(reopened_vec.len(), 0);
        let reopened_lex =
            ashkorix_core::search::lexical::TantivyLexicalIndex::open(&lex_path).expect("reopen");
        assert_eq!(reopened_lex.doc_count(), 0);
    }
}
