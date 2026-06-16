use crate::config::{discover_gguf_models, ModelFileInfo};
use std::path::{Path, PathBuf};

/// Resolve a config path that may be a `.gguf` file or a directory containing models.
pub fn resolve_gguf_path(path: &Path, prefer: ModelPathHint) -> Option<PathBuf> {
    if path.is_file() {
        return path
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| e.eq_ignore_ascii_case("gguf"))
            .map(|_| path.to_path_buf());
    }
    if !path.is_dir() {
        return None;
    }
    pick_best_gguf(&discover_gguf_models(path).unwrap_or_default(), prefer)
}

pub fn resolve_embedding_model_path(
    configured: Option<&Path>,
    models_dir: &Path,
) -> Option<PathBuf> {
    if let Some(path) = configured {
        if let Some(resolved) = resolve_gguf_path(path, ModelPathHint::Embedding) {
            return Some(resolved);
        }
    }
    let embeddings_dir = models_dir.join("embeddings");
    resolve_gguf_path(&embeddings_dir, ModelPathHint::Embedding)
        .or_else(|| resolve_gguf_path(models_dir, ModelPathHint::Embedding))
}

pub fn resolve_reranker_model_path(
    configured: Option<&Path>,
    models_dir: &Path,
) -> Option<PathBuf> {
    if let Some(path) = configured {
        if let Some(resolved) = resolve_gguf_path(path, ModelPathHint::Reranker) {
            return Some(resolved);
        }
    }
    let rerankers_dir = models_dir.join("rerankers");
    resolve_gguf_path(&rerankers_dir, ModelPathHint::Reranker)
}

#[derive(Debug, Clone, Copy)]
pub enum ModelPathHint {
    Embedding,
    Reranker,
    Any,
}

fn pick_best_gguf(models: &[ModelFileInfo], hint: ModelPathHint) -> Option<PathBuf> {
    if models.is_empty() {
        return None;
    }
    let keywords = match hint {
        ModelPathHint::Embedding => &["embed", "nomic", "bge", "e5"][..],
        ModelPathHint::Reranker => &["rerank", "cross", "jina"][..],
        ModelPathHint::Any => &[],
    };
    let lower = |name: &str| name.to_lowercase();
    for kw in keywords {
        if let Some(m) = models.iter().find(|m| lower(&m.filename).contains(kw)) {
            return Some(m.path.clone());
        }
    }
    Some(models[0].path.clone())
}

pub fn normalize_optional_gguf_path(
    path: Option<PathBuf>,
    hint: ModelPathHint,
    models_dir: &Path,
) -> Option<PathBuf> {
    path.as_ref()
        .and_then(|p| resolve_gguf_path(p, hint))
        .or_else(|| match hint {
            ModelPathHint::Embedding => resolve_embedding_model_path(None, models_dir),
            ModelPathHint::Reranker => resolve_reranker_model_path(None, models_dir),
            ModelPathHint::Any => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_embedding_filename_in_directory() {
        let dir = std::env::temp_dir().join(format!("ashkorix-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let chat = dir.join("chat.gguf");
        let embed = dir.join("nomic-embed-text.gguf");
        let _ = std::fs::write(&chat, b"x");
        let _ = std::fs::write(&embed, b"x");
        let picked = resolve_gguf_path(&dir, ModelPathHint::Embedding).unwrap();
        assert!(picked.to_string_lossy().contains("embed"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
