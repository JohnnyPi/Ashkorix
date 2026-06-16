use crate::chunking::types::Chunk;
use crate::documents::graph_types::{
    ChunkRelation, DocumentSummary, Entity, RelationKind, Section, StoredTable, SummaryLevel,
};
use crate::documents::types::Document;
use crate::error::Result;
use crate::pool::{KNOWLEDGE_BASE_NAME, POOL_ID};
use crate::types::{ChunkId, CollectionId, DocumentId};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct DocumentStore {
    conn: Mutex<Connection>,
}

impl DocumentStore {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        store.ensure_pool()?;
        store.ensure_legacy_pool_collection()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS pool_state (
                id TEXT PRIMARY KEY,
                indexed_at TEXT,
                document_count INTEGER DEFAULT 0,
                chunk_count INTEGER DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                content_hash TEXT UNIQUE NOT NULL,
                original_filename TEXT NOT NULL,
                file_path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                collection_id TEXT NOT NULL DEFAULT 'pool',
                imported_at TEXT NOT NULL,
                title TEXT,
                author TEXT,
                created_date TEXT,
                modified_date TEXT,
                extracted_text TEXT NOT NULL,
                extracted_tables TEXT NOT NULL DEFAULT '[]',
                metadata TEXT NOT NULL DEFAULT '{}',
                chunk_count INTEGER DEFAULT 0,
                import_status TEXT NOT NULL DEFAULT 'Imported'
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                collection_id TEXT NOT NULL DEFAULT 'pool',
                text TEXT NOT NULL,
                start_offset INTEGER NOT NULL,
                end_offset INTEGER NOT NULL,
                page_number INTEGER,
                section_title TEXT,
                row_sheet_info TEXT,
                source_filename TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                token_count INTEGER NOT NULL,
                FOREIGN KEY (document_id) REFERENCES documents(id)
            );
            CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id);
            CREATE TABLE IF NOT EXISTS embedding_cache (
                content_hash TEXT PRIMARY KEY,
                vector BLOB NOT NULL,
                dimension INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sections (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                parent_section_id TEXT,
                title TEXT NOT NULL,
                level INTEGER NOT NULL,
                heading_path TEXT NOT NULL,
                start_offset INTEGER NOT NULL,
                end_offset INTEGER NOT NULL,
                page_start INTEGER,
                page_end INTEGER,
                summary TEXT,
                FOREIGN KEY (document_id) REFERENCES documents(id)
            );
            CREATE INDEX IF NOT EXISTS idx_sections_document ON sections(document_id);
            CREATE TABLE IF NOT EXISTS doc_tables (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                section_id TEXT,
                caption TEXT,
                headers TEXT NOT NULL DEFAULT '[]',
                row_data TEXT NOT NULL DEFAULT '[]',
                FOREIGN KEY (document_id) REFERENCES documents(id)
            );
            CREATE TABLE IF NOT EXISTS entities (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                chunk_id TEXT,
                kind TEXT NOT NULL,
                value TEXT NOT NULL,
                FOREIGN KEY (document_id) REFERENCES documents(id)
            );
            CREATE INDEX IF NOT EXISTS idx_entities_value ON entities(value);
            CREATE TABLE IF NOT EXISTS chunk_relations (
                from_chunk_id TEXT NOT NULL,
                to_chunk_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                PRIMARY KEY (from_chunk_id, to_chunk_id, kind)
            );
            CREATE TABLE IF NOT EXISTS document_summaries (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                section_id TEXT,
                level TEXT NOT NULL,
                summary TEXT NOT NULL,
                FOREIGN KEY (document_id) REFERENCES documents(id)
            );
            "#,
        )?;
        drop(conn);
        self.add_column_if_missing("chunks", "parent_section_id", "TEXT")?;
        self.add_column_if_missing("chunks", "heading_path", "TEXT")?;
        self.add_column_if_missing("chunks", "chunk_index", "INTEGER DEFAULT 0")?;
        self.add_column_if_missing("chunks", "prev_chunk_id", "TEXT")?;
        self.add_column_if_missing("chunks", "next_chunk_id", "TEXT")?;
        self.add_column_if_missing("chunks", "contextual_text", "TEXT")?;
        self.add_column_if_missing("chunks", "table_id", "TEXT")?;
        self.add_column_if_missing("chunks", "entity_tokens", "TEXT")?;
        Ok(())
    }

    fn add_column_if_missing(&self, table: &str, column: &str, col_type: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .collect();
        if !cols.iter().any(|c| c == column) {
            conn.execute(
                &format!("ALTER TABLE {table} ADD COLUMN {column} {col_type}"),
                [],
            )?;
        }
        Ok(())
    }

    fn ensure_pool(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO pool_state (id, document_count, chunk_count) VALUES (?1, 0, 0)",
            params![POOL_ID],
        )?;
        Ok(())
    }

    fn ensure_legacy_pool_collection(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let has_collections: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='collections'",
                [],
                |row| row.get::<_, i64>(0).map(|n| n > 0),
            )
            .unwrap_or(false);
        if has_collections {
            conn.execute(
                "INSERT OR IGNORE INTO collections (id, name, created_at, chunk_count, document_count) VALUES (?1, ?2, ?3, 0, 0)",
                params![POOL_ID, KNOWLEDGE_BASE_NAME, Utc::now().to_rfc3339()],
            )?;
        }
        Ok(())
    }

    fn refresh_pool_counts(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let doc_count: i64 = conn.query_row("SELECT COUNT(*) FROM documents", [], |r| r.get(0))?;
        let chunk_count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
        conn.execute(
            "UPDATE pool_state SET document_count = ?1, chunk_count = ?2 WHERE id = ?3",
            params![doc_count, chunk_count, POOL_ID],
        )?;
        Ok(())
    }

    pub fn pool_indexed_at(&self) -> Result<Option<DateTime<Utc>>> {
        let conn = self.conn.lock().unwrap();
        let raw: Option<String> = conn
            .query_row(
                "SELECT indexed_at FROM pool_state WHERE id = ?1",
                params![POOL_ID],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        Ok(raw.and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&Utc)))
    }

    pub fn mark_pool_indexed(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE pool_state SET indexed_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), POOL_ID],
        )?;
        Ok(())
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<Document>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM documents WHERE content_hash = ?1 LIMIT 1")?;
        let mut rows = stmt.query(params![hash])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_document(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn insert_document(&self, doc: &Document) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO documents (
                id, content_hash, original_filename, file_path, file_type, collection_id,
                imported_at, title, author, created_date, modified_date, extracted_text,
                extracted_tables, metadata, chunk_count, import_status
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)"#,
            params![
                doc.id.0,
                doc.content_hash,
                doc.original_filename,
                doc.file_path.to_string_lossy().to_string(),
                format!("{:?}", doc.file_type),
                doc.collection_id,
                doc.imported_at.to_rfc3339(),
                doc.title,
                doc.author,
                doc.created_date.map(|d| d.to_rfc3339()),
                doc.modified_date.map(|d| d.to_rfc3339()),
                doc.extracted_text,
                serde_json::to_string(&doc.extracted_tables).unwrap_or_default(),
                doc.metadata.to_string(),
                doc.chunk_count,
                format!("{:?}", doc.import_status),
            ],
        )?;
        drop(conn);
        self.refresh_pool_counts()?;
        Ok(())
    }

    pub fn list_documents(&self) -> Result<Vec<Document>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM documents ORDER BY imported_at DESC")?;
        let rows = stmt.query_map([], row_to_document)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn delete_document(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM chunk_relations WHERE from_chunk_id IN (SELECT id FROM chunks WHERE document_id = ?1)", params![id])?;
        conn.execute("DELETE FROM entities WHERE document_id = ?1", params![id])?;
        conn.execute("DELETE FROM doc_tables WHERE document_id = ?1", params![id])?;
        conn.execute("DELETE FROM sections WHERE document_id = ?1", params![id])?;
        conn.execute("DELETE FROM document_summaries WHERE document_id = ?1", params![id])?;
        conn.execute("DELETE FROM chunks WHERE document_id = ?1", params![id])?;
        conn.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
        drop(conn);
        self.refresh_pool_counts()?;
        Ok(())
    }

    pub fn insert_chunks(&self, chunks: &[Chunk]) -> Result<()> {
        if chunks.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        for chunk in chunks {
            tx.execute(
                r#"INSERT OR REPLACE INTO chunks (
                    id, document_id, collection_id, text, start_offset, end_offset,
                    page_number, section_title, row_sheet_info, source_filename,
                    content_hash, token_count, parent_section_id, heading_path,
                    chunk_index, prev_chunk_id, next_chunk_id, contextual_text,
                    table_id, entity_tokens
                ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)"#,
                params![
                    chunk.id.0,
                    chunk.document_id.0,
                    chunk.collection_id.0,
                    chunk.text,
                    chunk.start_offset as i64,
                    chunk.end_offset as i64,
                    chunk.page_number.map(|p| p as i64),
                    chunk.section_title,
                    chunk.row_sheet_info,
                    chunk.source_filename,
                    chunk.content_hash,
                    chunk.token_count as i64,
                    chunk.parent_section_id,
                    chunk.heading_path,
                    chunk.chunk_index as i64,
                    chunk.prev_chunk_id,
                    chunk.next_chunk_id,
                    chunk.contextual_text,
                    chunk.table_id,
                    chunk.entity_tokens,
                ],
            )?;
        }
        let doc_id = &chunks[0].document_id.0;
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM chunks WHERE document_id = ?1",
            params![doc_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "UPDATE documents SET chunk_count = ?1, import_status = 'Chunked' WHERE id = ?2",
            params![count, doc_id],
        )?;
        tx.commit()?;
        drop(conn);
        self.refresh_pool_counts()?;
        Ok(())
    }

    pub fn insert_sections(&self, sections: &[Section]) -> Result<()> {
        if sections.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        let doc_id = &sections[0].document_id;
        tx.execute("DELETE FROM sections WHERE document_id = ?1", params![doc_id])?;
        for s in sections {
            tx.execute(
                r#"INSERT INTO sections (
                    id, document_id, parent_section_id, title, level, heading_path,
                    start_offset, end_offset, page_start, page_end, summary
                ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)"#,
                params![
                    s.id,
                    s.document_id,
                    s.parent_section_id,
                    s.title,
                    s.level as i64,
                    s.heading_path,
                    s.start_offset as i64,
                    s.end_offset as i64,
                    s.page_start.map(|p| p as i64),
                    s.page_end.map(|p| p as i64),
                    s.summary,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_tables(&self, tables: &[StoredTable]) -> Result<()> {
        if tables.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        let doc_id = &tables[0].document_id;
        tx.execute("DELETE FROM doc_tables WHERE document_id = ?1", params![doc_id])?;
        for t in tables {
            tx.execute(
                r#"INSERT INTO doc_tables (id, document_id, section_id, caption, headers, row_data)
                   VALUES (?1,?2,?3,?4,?5,?6)"#,
                params![
                    t.id,
                    t.document_id,
                    t.section_id,
                    t.caption,
                    serde_json::to_string(&t.headers).unwrap_or_default(),
                    serde_json::to_string(&t.row_data).unwrap_or_default(),
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_entities(&self, entities: &[Entity]) -> Result<()> {
        if entities.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        let doc_id = &entities[0].document_id;
        tx.execute("DELETE FROM entities WHERE document_id = ?1", params![doc_id])?;
        for e in entities {
            tx.execute(
                "INSERT INTO entities (id, document_id, chunk_id, kind, value) VALUES (?1,?2,?3,?4,?5)",
                params![
                    e.id,
                    e.document_id,
                    e.chunk_id,
                    entity_kind_str(&e.kind),
                    e.value,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_relations(&self, relations: &[ChunkRelation]) -> Result<()> {
        if relations.is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let tx = conn.unchecked_transaction()?;
        for r in relations {
            tx.execute(
                "INSERT OR IGNORE INTO chunk_relations (from_chunk_id, to_chunk_id, kind) VALUES (?1,?2,?3)",
                params![r.from_chunk_id, r.to_chunk_id, relation_kind_str(&r.kind)],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_document_summary(&self, summary: &DocumentSummary) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT OR REPLACE INTO document_summaries (id, document_id, section_id, level, summary)
               VALUES (?1,?2,?3,?4,?5)"#,
            params![
                summary.id,
                summary.document_id,
                summary.section_id,
                summary_level_str(&summary.level),
                summary.summary,
            ],
        )?;
        Ok(())
    }

    pub fn get_section(&self, id: &str) -> Result<Option<Section>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM sections WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_section(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn get_table(&self, id: &str) -> Result<Option<StoredTable>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM doc_tables WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_table(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_document_summaries(&self, document_id: &str) -> Result<Vec<DocumentSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM document_summaries WHERE document_id = ?1")?;
        let rows = stmt.query_map(params![document_id], row_to_summary)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_all_document_summaries(&self) -> Result<Vec<DocumentSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM document_summaries")?;
        let rows = stmt.query_map([], row_to_summary)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_entities(&self) -> Result<Vec<Entity>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM entities")?;
        let rows = stmt.query_map([], row_to_entity)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn lookup_entity_expansions(&self, term: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT value FROM entities WHERE value LIKE ?1 OR value LIKE ?2 LIMIT 20",
        )?;
        let upper = term.to_uppercase();
        let rows = stmt.query_map(params![format!("%{term}%"), format!("%{upper}%")], |row| {
            row.get::<_, String>(0)
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn update_section_summary(&self, section_id: &str, summary: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE sections SET summary = ?1 WHERE id = ?2",
            params![summary, section_id],
        )?;
        Ok(())
    }

    pub fn list_sections_for_document(&self, document_id: &str) -> Result<Vec<Section>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM sections WHERE document_id = ?1 ORDER BY start_offset")?;
        let rows = stmt.query_map(params![document_id], row_to_section)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_chunks_for_document(&self, document_id: &str) -> Result<Vec<Chunk>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM chunks WHERE document_id = ?1 ORDER BY chunk_index, start_offset")?;
        let rows = stmt.query_map(params![document_id], row_to_chunk)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn list_pool_chunks(&self) -> Result<Vec<Chunk>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT * FROM chunks WHERE collection_id = ?1 ORDER BY chunk_index, start_offset",
        )?;
        let rows = stmt.query_map(params![POOL_ID], row_to_chunk)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn get_chunk(&self, id: &str) -> Result<Option<Chunk>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM chunks WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_chunk(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn get_document(&self, id: &str) -> Result<Option<Document>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM documents WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_document(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn cache_embedding(&self, hash: &str, vector: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO embedding_cache (content_hash, vector, dimension) VALUES (?1, ?2, ?3)",
            params![hash, bytes, vector.len() as i64],
        )?;
        Ok(())
    }

    pub fn get_cached_embedding(&self, hash: &str) -> Result<Option<Vec<f32>>> {
        let conn = self.conn.lock().unwrap();
        let result: Option<(Vec<u8>, i64)> = conn
            .query_row(
                "SELECT vector, dimension FROM embedding_cache WHERE content_hash = ?1",
                params![hash],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .ok();
        if let Some((bytes, dim)) = result {
            let floats: Vec<f32> = bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            if floats.len() == dim as usize {
                return Ok(Some(floats));
            }
        }
        Ok(None)
    }
}

fn entity_kind_str(kind: &crate::documents::graph_types::EntityKind) -> &'static str {
    match kind {
        crate::documents::graph_types::EntityKind::Acronym => "acronym",
        crate::documents::graph_types::EntityKind::PartNumber => "part_number",
        crate::documents::graph_types::EntityKind::FieldId => "field_id",
        crate::documents::graph_types::EntityKind::ErrorCode => "error_code",
        crate::documents::graph_types::EntityKind::Other => "other",
    }
}

fn relation_kind_str(kind: &RelationKind) -> &'static str {
    match kind {
        RelationKind::Neighbor => "neighbor",
        RelationKind::ParentSection => "parent_section",
        RelationKind::SameTable => "same_table",
        RelationKind::CitationOf => "citation_of",
        RelationKind::SharedEntity => "shared_entity",
    }
}

fn summary_level_str(level: &SummaryLevel) -> &'static str {
    match level {
        SummaryLevel::Document => "document",
        SummaryLevel::Section => "section",
    }
}

fn row_to_document(row: &rusqlite::Row) -> rusqlite::Result<Document> {
    use crate::documents::types::{FileType, ImportStatus};
    let file_type_str: String = row.get(4)?;
    let file_type = match file_type_str.as_str() {
        "Markdown" => FileType::Markdown,
        "Html" => FileType::Html,
        "Csv" => FileType::Csv,
        "Json" => FileType::Json,
        "Xml" => FileType::Xml,
        "Pdf" => FileType::Pdf,
        "Docx" => FileType::Docx,
        "Xlsx" => FileType::Xlsx,
        _ => FileType::Txt,
    };
    let status_str: String = row.get(15)?;
    let import_status = match status_str.as_str() {
        "Duplicate" => ImportStatus::Duplicate,
        "Failed" => ImportStatus::Failed,
        "Chunked" => ImportStatus::Chunked,
        _ => ImportStatus::Imported,
    };
    Ok(Document {
        id: DocumentId(row.get(0)?),
        content_hash: row.get(1)?,
        original_filename: row.get(2)?,
        file_path: row.get::<_, String>(3)?.into(),
        file_type,
        collection_id: row.get(5)?,
        imported_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
        title: row.get(7)?,
        author: row.get(8)?,
        created_date: row
            .get::<_, Option<String>>(9)?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&Utc)),
        modified_date: row
            .get::<_, Option<String>>(10)?
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&Utc)),
        extracted_text: row.get(11)?,
        extracted_tables: serde_json::from_str(&row.get::<_, String>(12)?).unwrap_or_default(),
        metadata: serde_json::from_str(&row.get::<_, String>(13)?).unwrap_or(serde_json::json!({})),
        chunk_count: row.get::<_, i64>(14)? as u32,
        import_status,
    })
}

fn row_to_chunk(row: &rusqlite::Row) -> rusqlite::Result<Chunk> {
    let col_count = row.as_ref().column_count();
    let optional_str = |idx: usize| -> rusqlite::Result<Option<String>> {
        if idx < col_count {
            row.get(idx)
        } else {
            Ok(None)
        }
    };
    let optional_i64 = |idx: usize| -> rusqlite::Result<Option<i64>> {
        if idx < col_count {
            row.get(idx)
        } else {
            Ok(None)
        }
    };
    Ok(Chunk {
        id: ChunkId(row.get(0)?),
        document_id: DocumentId(row.get(1)?),
        collection_id: CollectionId(row.get(2)?),
        text: row.get(3)?,
        start_offset: row.get::<_, i64>(4)? as usize,
        end_offset: row.get::<_, i64>(5)? as usize,
        page_number: row.get::<_, Option<i64>>(6)?.map(|p| p as u32),
        section_title: row.get(7)?,
        row_sheet_info: row.get(8)?,
        source_filename: row.get(9)?,
        content_hash: row.get(10)?,
        token_count: row.get::<_, i64>(11)? as u32,
        parent_section_id: optional_str(12)?,
        heading_path: optional_str(13)?,
        chunk_index: optional_i64(14)?.unwrap_or(0) as u32,
        prev_chunk_id: optional_str(15)?,
        next_chunk_id: optional_str(16)?,
        contextual_text: optional_str(17)?,
        table_id: optional_str(18)?,
        entity_tokens: optional_str(19)?,
    })
}

fn row_to_section(row: &rusqlite::Row) -> rusqlite::Result<Section> {
    Ok(Section {
        id: row.get(0)?,
        document_id: row.get(1)?,
        parent_section_id: row.get(2)?,
        title: row.get(3)?,
        level: row.get::<_, i64>(4)? as u32,
        heading_path: row.get(5)?,
        start_offset: row.get::<_, i64>(6)? as usize,
        end_offset: row.get::<_, i64>(7)? as usize,
        page_start: row.get::<_, Option<i64>>(8)?.map(|p| p as u32),
        page_end: row.get::<_, Option<i64>>(9)?.map(|p| p as u32),
        summary: row.get(10)?,
    })
}

fn row_to_table(row: &rusqlite::Row) -> rusqlite::Result<StoredTable> {
    Ok(StoredTable {
        id: row.get(0)?,
        document_id: row.get(1)?,
        section_id: row.get(2)?,
        caption: row.get(3)?,
        headers: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
        row_data: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
    })
}

fn row_to_entity(row: &rusqlite::Row) -> rusqlite::Result<Entity> {
    use crate::documents::graph_types::EntityKind;
    let kind_str: String = row.get(3)?;
    let kind = match kind_str.as_str() {
        "acronym" => EntityKind::Acronym,
        "part_number" => EntityKind::PartNumber,
        "field_id" => EntityKind::FieldId,
        "error_code" => EntityKind::ErrorCode,
        _ => EntityKind::Other,
    };
    Ok(Entity {
        id: row.get(0)?,
        document_id: row.get(1)?,
        chunk_id: row.get(2)?,
        kind,
        value: row.get(4)?,
    })
}

fn row_to_summary(row: &rusqlite::Row) -> rusqlite::Result<DocumentSummary> {
    let level_str: String = row.get(3)?;
    Ok(DocumentSummary {
        id: row.get(0)?,
        document_id: row.get(1)?,
        section_id: row.get(2)?,
        level: if level_str == "section" {
            SummaryLevel::Section
        } else {
            SummaryLevel::Document
        },
        summary: row.get(4)?,
    })
}
