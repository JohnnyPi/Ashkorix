//! Golden fixture tests for retrieval pipeline components.

#[cfg(test)]
mod retrieval_eval {
    use ashkorix_core::chunking::HeadingHierarchyChunker;
    use ashkorix_core::config::ChunkingConfig;
    use ashkorix_core::documents::structure::build_section_tree;
    use ashkorix_core::documents::types::{Document, FileType, ImportStatus};
    use ashkorix_core::rag::types::RetrievalMode;
    use ashkorix_core::rerank::HeuristicReranker;
    use ashkorix_core::search::fusion::multi_reciprocal_rank_fusion;
    use ashkorix_core::types::DocumentId;
    use chrono::Utc;
    use std::path::PathBuf;

    fn sample_document(text: &str) -> Document {
        Document {
            id: DocumentId("fixture-doc".into()),
            content_hash: "fixture".into(),
            original_filename: "sample_manual.md".into(),
            file_path: PathBuf::from("fixtures/sample_manual.md"),
            file_type: FileType::Markdown,
            collection_id: "pool".into(),
            imported_at: Utc::now(),
            title: Some("Compliance Manual".into()),
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

    #[test]
    fn hierarchical_chunking_preserves_heading_paths() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("sample_manual.md");
        let text = std::fs::read_to_string(&fixture_path).expect("fixture file");
        let doc = sample_document(&text);
        let chunker = HeadingHierarchyChunker::new(ChunkingConfig::default());
        let result = chunker.chunk_with_graph(&doc, "pool").expect("chunk");
        assert!(!result.chunks.is_empty());
        assert!(!result.sections.is_empty());
        assert!(
            result
                .chunks
                .iter()
                .any(|c| c.heading_path.as_deref().unwrap_or("").contains("Field 04600")),
            "expected field section chunk"
        );
        assert!(
            result
                .entities
                .iter()
                .any(|e| e.value.contains("04600") || e.value.contains("4042")),
            "expected entity extraction"
        );
    }

    #[test]
    fn section_tree_parses_markdown_headings() {
        let text = "# Title\n\n## Section A\n\nBody text.";
        let doc = sample_document(text);
        let sections = build_section_tree(&doc);
        assert!(sections.len() >= 2);
    }

    #[test]
    fn multi_rrf_promotes_overlap() {
        let a = vec![("chunk-a".into(), 1.0), ("chunk-b".into(), 0.9)];
        let b = vec![("chunk-b".into(), 1.0), ("chunk-c".into(), 0.8)];
        let fused = multi_reciprocal_rank_fusion(&[&a, &b], 60.0);
        assert_eq!(fused[0].0, "chunk-b");
    }

    #[test]
    fn heuristic_reranker_boosts_phase_heading() {
        use ashkorix_core::chunking::types::Chunk;
        use ashkorix_core::rag::types::RankedChunk;
        use ashkorix_core::types::{ChunkId, CollectionId};

        let make_chunk = |text: &str, title: &str, index: u32, score: f64| RankedChunk {
            chunk: Chunk {
                id: ChunkId(format!("id-{index}")),
                document_id: DocumentId("d".into()),
                collection_id: CollectionId("pool".into()),
                text: text.to_string(),
                start_offset: 0,
                end_offset: text.len(),
                page_number: None,
                section_title: Some(title.into()),
                row_sheet_info: None,
                source_filename: "plan.md".into(),
                content_hash: format!("hash-{index}"),
                token_count: 10,
                parent_section_id: None,
                heading_path: Some(title.into()),
                chunk_index: index,
                prev_chunk_id: None,
                next_chunk_id: None,
                contextual_text: None,
                table_id: None,
                entity_tokens: None,
            },
            score,
            source_type: "both".into(),
            source_number: None,
            rerank_score: None,
            expanded_context: None,
        };

        let chunks = vec![
            make_chunk(
                "## 4. Phases\n\nEach phase lists what to build.",
                "4. Phases",
                4,
                0.9,
            ),
            make_chunk(
                "### Phase 1 — Vector foundation\n\n**Build.** Implement `vectormath`.",
                "Phase 1 — Vector foundation (vectormath + embeddings)",
                6,
                0.7,
            ),
        ];
        let reranked = HeuristicReranker::rerank("What is Phase 1?", chunks, 2).unwrap();
        assert!(
            reranked[0]
                .chunk
                .section_title
                .as_deref()
                .unwrap_or("")
                .contains("Phase 1"),
            "Phase 1 section should outrank generic phases overview"
        );
    }

    #[test]
    fn heuristic_reranker_boosts_exact_match() {
        use ashkorix_core::chunking::types::Chunk;
        use ashkorix_core::rag::types::RankedChunk;
        use ashkorix_core::types::{ChunkId, CollectionId};

        let make_chunk = |text: &str, score: f64| RankedChunk {
            chunk: Chunk {
                id: ChunkId(format!("id-{text}")),
                document_id: DocumentId("d".into()),
                collection_id: CollectionId("pool".into()),
                text: text.to_string(),
                start_offset: 0,
                end_offset: text.len(),
                page_number: None,
                section_title: None,
                row_sheet_info: None,
                source_filename: "test.md".into(),
                content_hash: text.into(),
                token_count: 10,
                parent_section_id: None,
                heading_path: None,
                chunk_index: 0,
                prev_chunk_id: None,
                next_chunk_id: None,
                contextual_text: None,
                table_id: None,
                entity_tokens: None,
            },
            score,
            source_type: "both".into(),
            source_number: None,
            rerank_score: None,
            expanded_context: None,
        };

        let chunks = vec![
            make_chunk("unrelated paragraph about weather", 0.9),
            make_chunk("Field 04600 compliance flag details", 0.7),
        ];
        let reranked = HeuristicReranker::rerank("Field 04600", chunks, 2).unwrap();
        assert!(
            reranked[0].chunk.text.contains("04600"),
            "exact term chunk should rank first"
        );
    }

    #[test]
    fn retrieval_mode_params_include_deep_and_corpus() {
        let (_, _, rerank, max, variants, iterative) = RetrievalMode::Deep.params();
        assert!(rerank);
        assert!(iterative);
        assert!(variants >= 5);
        assert!(max >= 10);

        let (_, _, _, _, _, _) = RetrievalMode::CorpusMap.params();
    }
}
