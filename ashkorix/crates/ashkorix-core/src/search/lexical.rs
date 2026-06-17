use crate::chunking::types::Chunk;
use crate::chunking::util::file_extension;
use crate::error::{AshkorixError, Result};
use crate::traits::LexicalIndex;
use std::path::{Path, PathBuf};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::Term;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};

pub const SCHEMA_VERSION: u32 = 2;

pub struct TantivyLexicalIndex {
    path: PathBuf,
    index: Index,
    schema: Schema,
    chunk_id_field: Field,
    document_id_field: Field,
    text_field: Field,
    contextual_field: Field,
    source_field: Field,
    section_field: Field,
    heading_path_field: Field,
    page_field: Field,
    file_type_field: Field,
    entity_field: Field,
    writer: Option<IndexWriter>,
    reader: tantivy::IndexReader,
}

impl TantivyLexicalIndex {
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;
        let version_path = path.join("schema_version.txt");
        let needs_recreate = if path.join("meta.json").exists() {
            let stored: u32 = std::fs::read_to_string(&version_path)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(1);
            stored < SCHEMA_VERSION
        } else {
            false
        };

        if needs_recreate {
            std::fs::remove_dir_all(path)?;
            std::fs::create_dir_all(path)?;
        }

        let (schema, fields) = build_schema();
        let index = if path.join("meta.json").exists() {
            Index::open_in_dir(path).map_err(|e| AshkorixError::Index(e.to_string()))?
        } else {
            Index::create_in_dir(path, schema.clone())
                .map_err(|e| AshkorixError::Index(e.to_string()))?
        };

        let writer = index
            .writer(50_000_000)
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| AshkorixError::Index(e.to_string()))?;

        if !version_path.exists() {
            std::fs::write(&version_path, SCHEMA_VERSION.to_string())?;
        }

        Ok(Self {
            path: path.to_path_buf(),
            index,
            schema,
            chunk_id_field: fields.0,
            document_id_field: fields.1,
            text_field: fields.2,
            contextual_field: fields.3,
            source_field: fields.4,
            section_field: fields.5,
            heading_path_field: fields.6,
            page_field: fields.7,
            file_type_field: fields.8,
            entity_field: fields.9,
            writer: Some(writer),
            reader,
        })
    }

    fn delete_chunk_id(&mut self, chunk_id: &str) -> Result<()> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| AshkorixError::Index("writer closed".into()))?;
        let term = Term::from_field_text(self.chunk_id_field, chunk_id);
        writer.delete_term(term);
        Ok(())
    }
}

type SchemaFields = (
    Field,
    Field,
    Field,
    Field,
    Field,
    Field,
    Field,
    Field,
    Field,
    Field,
);

fn build_schema() -> (Schema, SchemaFields) {
    let mut schema_builder = Schema::builder();
    let chunk_id_field = schema_builder.add_text_field("chunk_id", STRING | STORED);
    let document_id_field = schema_builder.add_text_field("document_id", STRING | STORED);
    let text_field = schema_builder.add_text_field("text", TEXT | STORED);
    let contextual_field = schema_builder.add_text_field("contextual_text", TEXT | STORED);
    let source_field = schema_builder.add_text_field("source_filename", STRING | STORED);
    let section_field = schema_builder.add_text_field("section_title", STRING | STORED);
    let heading_path_field = schema_builder.add_text_field("heading_path", TEXT | STORED);
    let page_field = schema_builder.add_u64_field("page_number", INDEXED | STORED);
    let file_type_field = schema_builder.add_text_field("file_type", STRING | STORED);
    let entity_field = schema_builder.add_text_field("entity_tokens", TEXT | STORED);
    let schema = schema_builder.build();
    (
        schema,
        (
            chunk_id_field,
            document_id_field,
            text_field,
            contextual_field,
            source_field,
            section_field,
            heading_path_field,
            page_field,
            file_type_field,
            entity_field,
        ),
    )
}

impl LexicalIndex for TantivyLexicalIndex {
    fn index_chunk(&mut self, chunk: &Chunk) -> Result<()> {
        self.delete_chunk_id(&chunk.id.0)?;
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| AshkorixError::Index("writer closed".into()))?;
        let contextual = chunk
            .contextual_text
            .clone()
            .unwrap_or_else(|| chunk.text.clone());
        writer
            .add_document(doc!(
                self.chunk_id_field => chunk.id.0.clone(),
                self.document_id_field => chunk.document_id.0.clone(),
                self.text_field => chunk.text.clone(),
                self.contextual_field => contextual,
                self.source_field => chunk.source_filename.clone(),
                self.section_field => chunk.section_title.clone().unwrap_or_default(),
                self.heading_path_field => chunk.heading_path.clone().unwrap_or_default(),
                self.page_field => chunk.page_number.unwrap_or(0) as u64,
                self.file_type_field => file_extension(&chunk.source_filename),
                self.entity_field => chunk.entity_tokens.clone().unwrap_or_default(),
            ))
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        Ok(())
    }

    fn search(&self, query: &str, top_k: usize) -> Result<Vec<(String, f32)>> {
        let searcher = self.reader.searcher();
        let fields = vec![
            self.text_field,
            self.contextual_field,
            self.heading_path_field,
            self.entity_field,
            self.section_field,
        ];
        let parser = QueryParser::for_index(&self.index, fields);
        let q = parser
            .parse_query(query)
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        let top_docs = searcher
            .search(&q, &TopDocs::with_limit(top_k))
            .map_err(|e| AshkorixError::Index(e.to_string()))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: tantivy::TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| AshkorixError::Index(e.to_string()))?;
            if let Some(chunk_id) = doc.get_first(self.chunk_id_field).and_then(|v| v.as_str()) {
                results.push((chunk_id.to_string(), score));
            }
        }
        Ok(results)
    }

    fn remove_chunk(&mut self, chunk_id: &str) -> Result<()> {
        self.delete_chunk_id(chunk_id)
    }

    fn remove_collection(&mut self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_dir_all(&self.path)?;
        }
        std::fs::create_dir_all(&self.path)?;
        let (schema, fields) = build_schema();
        let index = Index::create_in_dir(&self.path, schema.clone())
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        let writer = index
            .writer(50_000_000)
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        self.index = index;
        self.schema = schema;
        self.chunk_id_field = fields.0;
        self.document_id_field = fields.1;
        self.text_field = fields.2;
        self.contextual_field = fields.3;
        self.source_field = fields.4;
        self.section_field = fields.5;
        self.heading_path_field = fields.6;
        self.page_field = fields.7;
        self.file_type_field = fields.8;
        self.entity_field = fields.9;
        self.writer = Some(writer);
        self.reader = reader;
        std::fs::write(
            self.path.join("schema_version.txt"),
            SCHEMA_VERSION.to_string(),
        )?;
        Ok(())
    }

    fn commit(&mut self) -> Result<()> {
        if let Some(writer) = self.writer.as_mut() {
            writer
                .commit()
                .map_err(|e| AshkorixError::Index(e.to_string()))?;
            self.reader
                .reload()
                .map_err(|e| AshkorixError::Index(e.to_string()))?;
        }
        Ok(())
    }

    fn doc_count(&self) -> usize {
        self.reader.searcher().num_docs() as usize
    }
}
