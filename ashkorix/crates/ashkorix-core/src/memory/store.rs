use crate::error::{AshkorixError, Result};
use crate::memory::types::{
    CandidateStatus, CreateMemoryInput, EditCandidateInput, Memory, MemoryCandidate,
    MemoryStatus, MemoryType, UpdateMemoryInput, normalize_content,
};
use crate::types::{hash_text, short_id_from_hash};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct MemoryStore {
    conn: Mutex<Connection>,
}

impl MemoryStore {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                type TEXT NOT NULL,
                scope TEXT NOT NULL,
                title TEXT NOT NULL,
                content TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                confidence REAL DEFAULT 0.8,
                status TEXT DEFAULT 'active',
                source_type TEXT,
                source_ref TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_used_at TEXT,
                supersedes_id TEXT,
                metadata_json TEXT
            );
            CREATE TABLE IF NOT EXISTS memory_candidates (
                id TEXT PRIMARY KEY,
                proposed_type TEXT NOT NULL,
                proposed_scope TEXT NOT NULL,
                proposed_title TEXT NOT NULL,
                proposed_content TEXT NOT NULL,
                importance REAL DEFAULT 0.5,
                confidence REAL DEFAULT 0.8,
                reason TEXT,
                source_type TEXT,
                source_ref TEXT,
                created_at TEXT NOT NULL,
                status TEXT DEFAULT 'pending'
            );
            CREATE INDEX IF NOT EXISTS idx_memories_scope_status ON memories(scope, status);
            CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(type);
            CREATE INDEX IF NOT EXISTS idx_candidates_status ON memory_candidates(status);
            "#,
        )?;
        Ok(())
    }

    fn new_memory_id(title: &str, content: &str) -> String {
        let stamp = Utc::now().to_rfc3339();
        format!(
            "mem_{}",
            short_id_from_hash(&hash_text(&format!("{title}{content}{stamp}")))
        )
    }

    fn new_candidate_id(title: &str, content: &str) -> String {
        let stamp = Utc::now().to_rfc3339();
        format!(
            "cand_{}",
            short_id_from_hash(&hash_text(&format!("{title}{content}{stamp}")))
        )
    }

    pub fn seed_if_empty(&self) -> Result<()> {
        if !self.list_active(None)?.is_empty() {
            return Ok(());
        }
        let seeds = [
            CreateMemoryInput {
                memory_type: MemoryType::ProjectFact,
                scope: "project:ashkorix".into(),
                title: "Ashkorix is local-first".into(),
                content: "Ashkorix is intended to be a local-first GGUF model runner.".into(),
                importance: 0.95,
                confidence: 1.0,
                source_type: Some("seed".into()),
                source_ref: None,
                supersedes_id: None,
                metadata_json: None,
            },
            CreateMemoryInput {
                memory_type: MemoryType::Decision,
                scope: "project:ashkorix".into(),
                title: "Remove Salutori branding".into(),
                content: "The user decided that Salutori branding is no longer relevant and should not be used for Ashkorix.".into(),
                importance: 0.95,
                confidence: 1.0,
                source_type: Some("seed".into()),
                source_ref: None,
                supersedes_id: None,
                metadata_json: None,
            },
            CreateMemoryInput {
                memory_type: MemoryType::Procedure,
                scope: "global".into(),
                title: "Use phased implementation plans".into(),
                content: "When the user asks for implementation planning, prefer a phased implementation plan with concrete build steps.".into(),
                importance: 0.75,
                confidence: 0.9,
                source_type: Some("seed".into()),
                source_ref: None,
                supersedes_id: None,
                metadata_json: None,
            },
            CreateMemoryInput {
                memory_type: MemoryType::UserPreference,
                scope: "global".into(),
                title: "Prefer practical architecture".into(),
                content: "The user prefers practical system architecture and implementation details over abstract theory.".into(),
                importance: 0.75,
                confidence: 0.85,
                source_type: Some("seed".into()),
                source_ref: None,
                supersedes_id: None,
                metadata_json: None,
            },
        ];
        for seed in seeds {
            self.insert(&seed)?;
        }
        Ok(())
    }

    pub fn insert(&self, input: &CreateMemoryInput) -> Result<Memory> {
        let now = Utc::now();
        let id = Self::new_memory_id(&input.title, &input.content);
        let memory = Memory {
            id: id.clone(),
            memory_type: input.memory_type,
            scope: input.scope.clone(),
            title: input.title.clone(),
            content: input.content.clone(),
            importance: input.importance,
            confidence: input.confidence,
            status: MemoryStatus::Active,
            source_type: input.source_type.clone(),
            source_ref: input.source_ref.clone(),
            created_at: now,
            updated_at: now,
            last_used_at: None,
            supersedes_id: input.supersedes_id.clone(),
            metadata_json: input.metadata_json.clone(),
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO memories (
                id, type, scope, title, content, importance, confidence, status,
                source_type, source_ref, created_at, updated_at, last_used_at,
                supersedes_id, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
            params![
                memory.id,
                memory.memory_type.as_str(),
                memory.scope,
                memory.title,
                memory.content,
                memory.importance,
                memory.confidence,
                memory.status.as_str(),
                memory.source_type,
                memory.source_ref,
                memory.created_at.to_rfc3339(),
                memory.updated_at.to_rfc3339(),
                memory.last_used_at.map(|t| t.to_rfc3339()),
                memory.supersedes_id,
                memory.metadata_json,
            ],
        )?;
        Ok(memory)
    }

    pub fn get(&self, id: &str) -> Result<Option<Memory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM memories WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_memory(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn update(&self, id: &str, input: &UpdateMemoryInput) -> Result<Memory> {
        let mut memory = self
            .get(id)?
            .ok_or_else(|| AshkorixError::Config(format!("memory not found: {id}")))?;
        if let Some(t) = input.memory_type {
            memory.memory_type = t;
        }
        if let Some(ref s) = input.scope {
            memory.scope = s.clone();
        }
        if let Some(ref t) = input.title {
            memory.title = t.clone();
        }
        if let Some(ref c) = input.content {
            memory.content = c.clone();
        }
        if let Some(i) = input.importance {
            memory.importance = i;
        }
        if let Some(c) = input.confidence {
            memory.confidence = c;
        }
        if let Some(s) = input.status {
            memory.status = s;
        }
        if let Some(ref m) = input.metadata_json {
            memory.metadata_json = Some(m.clone());
        }
        memory.updated_at = Utc::now();

        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"UPDATE memories SET
                type = ?2, scope = ?3, title = ?4, content = ?5,
                importance = ?6, confidence = ?7, status = ?8,
                updated_at = ?9, metadata_json = ?10
            WHERE id = ?1"#,
            params![
                memory.id,
                memory.memory_type.as_str(),
                memory.scope,
                memory.title,
                memory.content,
                memory.importance,
                memory.confidence,
                memory.status.as_str(),
                memory.updated_at.to_rfc3339(),
                memory.metadata_json,
            ],
        )?;
        Ok(memory)
    }

    pub fn deactivate(&self, id: &str) -> Result<()> {
        self.update(
            id,
            &UpdateMemoryInput {
                status: Some(MemoryStatus::Deleted),
                ..Default::default()
            },
        )?;
        Ok(())
    }

    pub fn mark_superseded(&self, old_id: &str, new_id: &str) -> Result<()> {
        let now = Utc::now();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memories SET status = 'superseded', supersedes_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![old_id, new_id, now.to_rfc3339()],
        )?;
        Ok(())
    }

    pub fn list_active(&self, scope_filter: Option<&str>) -> Result<Vec<Memory>> {
        let conn = self.conn.lock().unwrap();
        let (sql, scope) = if let Some(scope) = scope_filter {
            (
                "SELECT * FROM memories WHERE status = 'active' AND scope = ?1 ORDER BY updated_at DESC",
                Some(scope.to_string()),
            )
        } else {
            (
                "SELECT * FROM memories WHERE status = 'active' ORDER BY updated_at DESC",
                None,
            )
        };
        let mut stmt = conn.prepare(sql)?;
        let memories = if let Some(scope) = scope {
            let rows = stmt.query_map(params![scope], row_to_memory)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            let rows = stmt.query_map([], row_to_memory)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(memories)
    }

    pub fn list_by_scopes(
        &self,
        scopes: &[String],
        min_confidence: f64,
    ) -> Result<Vec<Memory>> {
        if scopes.is_empty() {
            return Ok(vec![]);
        }
        let placeholders: Vec<String> = scopes.iter().enumerate().map(|(i, _)| format!("?{}", i + 2)).collect();
        let sql = format!(
            "SELECT * FROM memories WHERE status = 'active' AND confidence >= ?1 AND scope IN ({})",
            placeholders.join(", ")
        );
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(&sql)?;
        let params: Vec<Box<dyn rusqlite::ToSql>> = {
            let mut p: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(min_confidence)];
            for s in scopes {
                p.push(Box::new(s.clone()));
            }
            p
        };
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(param_refs.as_slice(), row_to_memory)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>> {
        let pattern = format!("%{}%", query.to_lowercase());
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT * FROM memories WHERE status = 'active' AND (
                LOWER(title) LIKE ?1 OR LOWER(content) LIKE ?1 OR LOWER(scope) LIKE ?1
            ) ORDER BY importance DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], row_to_memory)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn touch_last_used(&self, ids: &[String]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        for id in ids {
            conn.execute(
                "UPDATE memories SET last_used_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
        }
        Ok(())
    }

    pub fn has_active_duplicate(&self, scope: &str, content: &str) -> Result<bool> {
        let normalized = normalize_content(content);
        let memories = self.list_by_scopes(&[scope.to_string()], 0.0)?;
        Ok(memories
            .iter()
            .any(|m| normalize_content(&m.content) == normalized))
    }

    pub fn insert_candidate(
        &self,
        proposed_type: MemoryType,
        proposed_scope: &str,
        proposed_title: &str,
        proposed_content: &str,
        importance: f64,
        confidence: f64,
        reason: Option<String>,
        source_type: Option<String>,
        source_ref: Option<String>,
    ) -> Result<MemoryCandidate> {
        let id = Self::new_candidate_id(proposed_title, proposed_content);
        let now = Utc::now();
        let candidate = MemoryCandidate {
            id: id.clone(),
            proposed_type,
            proposed_scope: proposed_scope.to_string(),
            proposed_title: proposed_title.to_string(),
            proposed_content: proposed_content.to_string(),
            importance,
            confidence,
            reason,
            source_type,
            source_ref,
            created_at: now,
            status: CandidateStatus::Pending,
        };
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"INSERT INTO memory_candidates (
                id, proposed_type, proposed_scope, proposed_title, proposed_content,
                importance, confidence, reason, source_type, source_ref, created_at, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"#,
            params![
                candidate.id,
                candidate.proposed_type.as_str(),
                candidate.proposed_scope,
                candidate.proposed_title,
                candidate.proposed_content,
                candidate.importance,
                candidate.confidence,
                candidate.reason,
                candidate.source_type,
                candidate.source_ref,
                candidate.created_at.to_rfc3339(),
                candidate.status.as_str(),
            ],
        )?;
        Ok(candidate)
    }

    pub fn list_pending_candidates(&self) -> Result<Vec<MemoryCandidate>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM memory_candidates WHERE status = 'pending' ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], row_to_candidate)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn list_all_candidates(&self) -> Result<Vec<MemoryCandidate>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT * FROM memory_candidates ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], row_to_candidate)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn get_candidate(&self, id: &str) -> Result<Option<MemoryCandidate>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM memory_candidates WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row_to_candidate(row)?))
        } else {
            Ok(None)
        }
    }

    fn set_candidate_status(&self, id: &str, status: CandidateStatus) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE memory_candidates SET status = ?2 WHERE id = ?1",
            params![id, status.as_str()],
        )?;
        Ok(())
    }

    pub fn reject_candidate(&self, id: &str) -> Result<()> {
        self.set_candidate_status(id, CandidateStatus::Rejected)
    }

    pub fn approve_candidate(&self, id: &str) -> Result<Memory> {
        let candidate = self
            .get_candidate(id)?
            .ok_or_else(|| AshkorixError::Config(format!("candidate not found: {id}")))?;
        if self.has_active_duplicate(&candidate.proposed_scope, &candidate.proposed_content)? {
            return Err(AshkorixError::Config(
                "an active memory with the same content already exists in this scope".into(),
            ));
        }
        let memory = self.insert(&CreateMemoryInput {
            memory_type: candidate.proposed_type,
            scope: candidate.proposed_scope.clone(),
            title: candidate.proposed_title.clone(),
            content: candidate.proposed_content.clone(),
            importance: candidate.importance,
            confidence: candidate.confidence,
            source_type: candidate.source_type.clone(),
            source_ref: candidate.source_ref.clone(),
            supersedes_id: None,
            metadata_json: None,
        })?;
        self.set_candidate_status(id, CandidateStatus::Approved)?;
        Ok(memory)
    }

    pub fn edit_and_approve_candidate(
        &self,
        id: &str,
        edit: &EditCandidateInput,
    ) -> Result<Memory> {
        let candidate = self
            .get_candidate(id)?
            .ok_or_else(|| AshkorixError::Config(format!("candidate not found: {id}")))?;
        let scope = edit
            .proposed_scope
            .clone()
            .unwrap_or(candidate.proposed_scope);
        let content = edit
            .proposed_content
            .clone()
            .unwrap_or(candidate.proposed_content);
        if self.has_active_duplicate(&scope, &content)? {
            return Err(AshkorixError::Config(
                "an active memory with the same content already exists in this scope".into(),
            ));
        }
        let memory = self.insert(&CreateMemoryInput {
            memory_type: edit.proposed_type.unwrap_or(candidate.proposed_type),
            scope,
            title: edit
                .proposed_title
                .clone()
                .unwrap_or(candidate.proposed_title),
            content,
            importance: edit.importance.unwrap_or(candidate.importance),
            confidence: edit.confidence.unwrap_or(candidate.confidence),
            source_type: candidate.source_type.clone(),
            source_ref: candidate.source_ref.clone(),
            supersedes_id: None,
            metadata_json: None,
        })?;
        self.set_candidate_status(id, CandidateStatus::Edited)?;
        Ok(memory)
    }

    pub fn pending_candidate_exists(&self, scope: &str, content: &str) -> Result<bool> {
        let normalized = normalize_content(content);
        let pending = self.list_pending_candidates()?;
        Ok(pending
            .iter()
            .any(|c| c.proposed_scope == scope && normalize_content(&c.proposed_content) == normalized))
    }
}

impl Default for UpdateMemoryInput {
    fn default() -> Self {
        Self {
            memory_type: None,
            scope: None,
            title: None,
            content: None,
            importance: None,
            confidence: None,
            status: None,
            metadata_json: None,
        }
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| AshkorixError::Config(format!("invalid datetime: {e}")))
}

fn row_to_memory(row: &rusqlite::Row) -> rusqlite::Result<Memory> {
    let type_str: String = row.get("type")?;
    let status_str: String = row.get("status")?;
    let created_at: String = row.get("created_at")?;
    let updated_at: String = row.get("updated_at")?;
    let last_used_at: Option<String> = row.get("last_used_at")?;
    Ok(Memory {
        id: row.get("id")?,
        memory_type: MemoryType::from_str(&type_str).unwrap_or(MemoryType::ProjectFact),
        scope: row.get("scope")?,
        title: row.get("title")?,
        content: row.get("content")?,
        importance: row.get("importance")?,
        confidence: row.get("confidence")?,
        status: MemoryStatus::from_str(&status_str).unwrap_or(MemoryStatus::Active),
        source_type: row.get("source_type")?,
        source_ref: row.get("source_ref")?,
        created_at: parse_datetime(&created_at).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        updated_at: parse_datetime(&updated_at).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        last_used_at: last_used_at
            .map(|s| parse_datetime(&s))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
            })?,
        supersedes_id: row.get("supersedes_id")?,
        metadata_json: row.get("metadata_json")?,
    })
}

fn row_to_candidate(row: &rusqlite::Row) -> rusqlite::Result<MemoryCandidate> {
    let type_str: String = row.get("proposed_type")?;
    let status_str: String = row.get("status")?;
    let created_at: String = row.get("created_at")?;
    Ok(MemoryCandidate {
        id: row.get("id")?,
        proposed_type: MemoryType::from_str(&type_str).unwrap_or(MemoryType::ProjectFact),
        proposed_scope: row.get("proposed_scope")?,
        proposed_title: row.get("proposed_title")?,
        proposed_content: row.get("proposed_content")?,
        importance: row.get("importance")?,
        confidence: row.get("confidence")?,
        reason: row.get("reason")?,
        source_type: row.get("source_type")?,
        source_ref: row.get("source_ref")?,
        created_at: parse_datetime(&created_at).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        status: CandidateStatus::from_str(&status_str).unwrap_or(CandidateStatus::Pending),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::types::MemoryType;
    use tempfile::tempdir;

    fn open_test_store() -> MemoryStore {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.db");
        let store = MemoryStore::open(&path).unwrap();
        std::mem::forget(dir);
        store
    }

    #[test]
    fn insert_and_list_memories() {
        let store = open_test_store();
        store
            .insert(&CreateMemoryInput {
                memory_type: MemoryType::ProjectFact,
                scope: "project:ashkorix".into(),
                title: "Test fact".into(),
                content: "Ashkorix is local-first.".into(),
                importance: 0.9,
                confidence: 1.0,
                source_type: None,
                source_ref: None,
                supersedes_id: None,
                metadata_json: None,
            })
            .unwrap();
        let list = store.list_active(None).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "Test fact");
    }

    #[test]
    fn approve_candidate_creates_memory() {
        let store = open_test_store();
        let candidate = store
            .insert_candidate(
                MemoryType::Decision,
                "project:ashkorix",
                "Name choice",
                "User chose Ashkorix.",
                0.9,
                1.0,
                None,
                Some("test".into()),
                None,
            )
            .unwrap();
        let memory = store.approve_candidate(&candidate.id).unwrap();
        assert_eq!(memory.content, "User chose Ashkorix.");
        let pending = store.list_pending_candidates().unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn supersede_marks_old_memory() {
        let store = open_test_store();
        let old = store
            .insert(&CreateMemoryInput {
                memory_type: MemoryType::ProjectFact,
                scope: "project:ashkorix".into(),
                title: "Old name".into(),
                content: "Salutori is the product name.".into(),
                importance: 0.9,
                confidence: 1.0,
                source_type: None,
                source_ref: None,
                supersedes_id: None,
                metadata_json: None,
            })
            .unwrap();
        let new = store
            .insert(&CreateMemoryInput {
                memory_type: MemoryType::ProjectFact,
                scope: "project:ashkorix".into(),
                title: "New name".into(),
                content: "Ashkorix is the product name.".into(),
                importance: 0.95,
                confidence: 1.0,
                source_type: None,
                source_ref: None,
                supersedes_id: None,
                metadata_json: None,
            })
            .unwrap();
        store.mark_superseded(&old.id, &new.id).unwrap();
        let updated = store.get(&old.id).unwrap().unwrap();
        assert_eq!(updated.status, MemoryStatus::Superseded);
        assert_eq!(updated.supersedes_id, Some(new.id));
    }
}
