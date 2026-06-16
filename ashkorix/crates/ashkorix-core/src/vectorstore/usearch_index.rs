use crate::error::{AshkorixError, Result};
use crate::traits::VectorIndex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use usearch::IndexOptions;
use usearch::{Index, MetricKind, ScalarKind};

pub struct UsearchVectorIndex {
    path: PathBuf,
    dimension: usize,
    index: Index,
    id_to_key: HashMap<String, u64>,
    key_to_id: HashMap<u64, String>,
    next_key: u64,
}

impl UsearchVectorIndex {
    pub fn open(path: &Path, dimension: usize) -> Result<Self> {
        let mut id_to_key = HashMap::new();
        let mut key_to_id = HashMap::new();
        let mut next_key = 1u64;

        let index = if path.exists() && path.extension().is_some_and(|e| e == "usearch") {
            let idx = Index::new(&IndexOptions {
                dimensions: dimension,
                metric: MetricKind::Cos,
                quantization: ScalarKind::F32,
                connectivity: 16,
                expansion_add: 128,
                expansion_search: 64,
                multi: false,
            })
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
            idx.load(
                path.to_str()
                    .ok_or_else(|| AshkorixError::Index("invalid path".into()))?,
            )
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
            let meta_path = path.with_extension("usearch.meta");
            if meta_path.exists() {
                let data = std::fs::read_to_string(&meta_path)
                    .map_err(|e| AshkorixError::Index(e.to_string()))?;
                let parsed: IdMapping = serde_json::from_str(&data)
                    .map_err(|e| AshkorixError::Index(e.to_string()))?;
                id_to_key = parsed.id_to_key;
                key_to_id = parsed.key_to_id;
                next_key = parsed.next_key;
            }
            idx
        } else if path.exists() {
            let data = std::fs::read(path).map_err(|e| AshkorixError::Index(e.to_string()))?;
            if data.first() == Some(&b'{') {
                Self::migrate_json_index(path, dimension, &mut id_to_key, &mut key_to_id, &mut next_key)?
            } else {
                let idx = Index::new(&IndexOptions {
                    dimensions: dimension,
                    metric: MetricKind::Cos,
                    quantization: ScalarKind::F32,
                    connectivity: 16,
                    expansion_add: 128,
                    expansion_search: 64,
                    multi: false,
                })
                .map_err(|e| AshkorixError::Index(e.to_string()))?;
                idx.load(
                    path.to_str()
                        .ok_or_else(|| AshkorixError::Index("invalid path".into()))?,
                )
                .map_err(|e| AshkorixError::Index(e.to_string()))?;
                idx
            }
        } else {
            Index::new(&IndexOptions {
                dimensions: dimension,
                metric: MetricKind::Cos,
                quantization: ScalarKind::F32,
                connectivity: 16,
                expansion_add: 128,
                expansion_search: 64,
                multi: false,
            })
            .map_err(|e| AshkorixError::Index(e.to_string()))?
        };

        Ok(Self {
            path: path.to_path_buf(),
            dimension,
            index,
            id_to_key,
            key_to_id,
            next_key,
        })
    }

    fn migrate_json_index(
        path: &Path,
        dimension: usize,
        id_to_key: &mut HashMap<String, u64>,
        key_to_id: &mut HashMap<u64, String>,
        next_key: &mut u64,
    ) -> Result<Index> {
        let data = std::fs::read_to_string(path).map_err(|e| AshkorixError::Index(e.to_string()))?;
        let parsed: LegacyStoredIndex = serde_json::from_str(&data)
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        let index = Index::new(&IndexOptions {
            dimensions: dimension.max(parsed.dimension),
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        })
        .map_err(|e| AshkorixError::Index(e.to_string()))?;
        for (id, vec) in parsed.ids.iter().zip(parsed.vectors.iter()) {
            let key = *next_key;
            *next_key += 1;
            id_to_key.insert(id.clone(), key);
            key_to_id.insert(key, id.clone());
            let needed = index.size() + 1;
            if needed > index.capacity() {
                index
                    .reserve(needed.max(index.capacity().max(64) * 2))
                    .map_err(|e| AshkorixError::Index(e.to_string()))?;
            }
            index
                .add(key, vec)
                .map_err(|e| AshkorixError::Index(e.to_string()))?;
        }
        Ok(index)
    }

    fn usearch_path(&self) -> PathBuf {
        if self.path.extension().is_some_and(|e| e == "usearch") {
            self.path.clone()
        } else {
            self.path.with_extension("usearch")
        }
    }

    fn meta_path(&self) -> PathBuf {
        self.usearch_path().with_extension("usearch.meta")
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LegacyStoredIndex {
    dimension: usize,
    ids: Vec<String>,
    vectors: Vec<Vec<f32>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct IdMapping {
    id_to_key: HashMap<String, u64>,
    key_to_id: HashMap<u64, String>,
    next_key: u64,
}

impl VectorIndex for UsearchVectorIndex {
    fn upsert(&mut self, chunk_id: &str, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dimension {
            return Err(AshkorixError::Index(format!(
                "dimension mismatch: expected {}, got {}",
                self.dimension,
                vector.len()
            )));
        }
        let is_update = self.id_to_key.contains_key(chunk_id);
        let key = if let Some(k) = self.id_to_key.get(chunk_id) {
            *k
        } else {
            let k = self.next_key;
            self.next_key += 1;
            self.id_to_key.insert(chunk_id.to_string(), k);
            self.key_to_id.insert(k, chunk_id.to_string());
            k
        };
        if is_update {
            let _ = self.index.remove(key);
        }
        let needed = self.index.size() + 1;
        if needed > self.index.capacity() {
            self.index
                .reserve(needed.max(self.index.capacity().max(64) * 2))
                .map_err(|e| AshkorixError::Index(e.to_string()))?;
        }
        self.index
            .add(key, vector)
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        Ok(())
    }

    fn search(&self, query: &[f32], top_k: usize) -> Result<Vec<(String, f32)>> {
        if query.len() != self.dimension {
            return Err(AshkorixError::Index(format!(
                "query dimension mismatch: expected {}, got {}",
                self.dimension,
                query.len()
            )));
        }
        let matches = self
            .index
            .search(query, top_k)
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        let mut results = Vec::new();
        for m in matches.keys.iter().zip(matches.distances.iter()) {
            if let Some(id) = self.key_to_id.get(m.0) {
                results.push((id.clone(), 1.0 - m.1));
            }
        }
        Ok(results)
    }

    fn remove_collection(&mut self) -> Result<()> {
        let usearch_path = self.usearch_path();
        if usearch_path.exists() {
            std::fs::remove_file(&usearch_path)?;
        }
        if self.meta_path().exists() {
            std::fs::remove_file(self.meta_path())?;
        }
        if self.path.exists() && self.path != usearch_path {
            std::fs::remove_file(&self.path)?;
        }
        self.id_to_key.clear();
        self.key_to_id.clear();
        self.next_key = 1;
        self.index = Index::new(&IndexOptions {
            dimensions: self.dimension,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
            multi: false,
        })
        .map_err(|e| AshkorixError::Index(e.to_string()))?;
        Ok(())
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.usearch_path().parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.index
            .save(
                self.usearch_path()
                    .to_str()
                    .ok_or_else(|| AshkorixError::Index("invalid path".into()))?,
            )
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        let mapping = IdMapping {
            id_to_key: self.id_to_key.clone(),
            key_to_id: self.key_to_id.clone(),
            next_key: self.next_key,
        };
        let json = serde_json::to_string(&mapping)
            .map_err(|e| AshkorixError::Index(e.to_string()))?;
        std::fs::write(self.meta_path(), json)?;
        Ok(())
    }

    fn len(&self) -> usize {
        self.id_to_key.len()
    }
}
