mod model_paths;
mod paths;
mod settings;

pub use model_paths::*;
pub use paths::*;
pub use settings::*;

use crate::error::{AshkorixError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FORBIDDEN_KEYS: &[&str] = &[
    "api_url",
    "openai_api_key",
    "anthropic_api_key",
    "remote_endpoint",
    "cloud_api",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AshkorixConfig {
    pub data_dir: PathBuf,
    pub models_dir: PathBuf,
    pub embedding_model_path: Option<PathBuf>,
    pub reranker_model_path: Option<PathBuf>,
    pub default_retrieval_mode: String,
    pub log_level: String,
    pub local_only: bool,
    pub generation: GenerationConfig,
    pub chunking: ChunkingConfig,
    pub retrieval: RetrievalConfig,
}

impl Default for AshkorixConfig {
    fn default() -> Self {
        let data_dir = default_data_dir();
        Self {
            models_dir: data_dir.join("models"),
            data_dir: data_dir.clone(),
            embedding_model_path: None,
            reranker_model_path: None,
            default_retrieval_mode: "balanced".to_string(),
            log_level: "info".to_string(),
            local_only: true,
            generation: GenerationConfig::default(),
            chunking: ChunkingConfig::default(),
            retrieval: RetrievalConfig::default(),
        }
    }
}

impl AshkorixConfig {
    pub fn load() -> Result<Self> {
        let data_dir = resolve_data_dir();
        let config_path = data_dir.join("config.toml");
        if config_path.exists() {
            let raw = std::fs::read_to_string(&config_path)?;
            validate_no_remote_keys(&raw)?;
            let mut config: Self = toml::from_str(&raw)
                .map_err(|e| AshkorixError::Config(e.to_string()))?;
            let old_data = config.data_dir.clone();
            let old_models = config.models_dir.clone();
            sync_data_paths(&mut config, &data_dir);
            config.local_only = true;
            config.normalize_model_paths();
            if config.data_dir != old_data || config.models_dir != old_models {
                config.ensure_dirs()?;
                config.save()?;
            }
            Ok(config)
        } else {
            let mut config = Self::default();
            sync_data_paths(&mut config, &data_dir);
            config.ensure_dirs()?;
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        self.ensure_dirs()?;
        let config_path = self.data_dir.join("config.toml");
        let content = toml::to_string_pretty(self)
            .map_err(|e| AshkorixError::Config(e.to_string()))?;
        validate_no_remote_keys(&content)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            &self.data_dir,
            &self.models_dir,
            &self.models_dir.join("embeddings"),
            &self.documents_dir(),
            &self.index_dir(),
            &self.logs_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn documents_dir(&self) -> PathBuf {
        self.data_dir.join("documents")
    }

    pub fn index_dir(&self) -> PathBuf {
        self.data_dir.join("index")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.data_dir.join("logs")
    }

    /// Resolve directory paths to concrete `.gguf` files where possible.
    pub fn normalize_model_paths(&mut self) {
        self.embedding_model_path = normalize_optional_gguf_path(
            self.embedding_model_path.clone(),
            ModelPathHint::Embedding,
            &self.models_dir,
        );
        self.reranker_model_path = normalize_optional_gguf_path(
            self.reranker_model_path.clone(),
            ModelPathHint::Reranker,
            &self.models_dir,
        );
    }
}

fn sync_data_paths(config: &mut AshkorixConfig, data_dir: &Path) {
    config.data_dir = data_dir.to_path_buf();
    let default_models = data_dir.join("models");
    if config.models_dir.as_os_str().is_empty() || !config.models_dir.starts_with(data_dir) {
        config.models_dir = default_models;
    }
}

fn validate_no_remote_keys(content: &str) -> Result<()> {
    let lower = content.to_lowercase();
    for key in FORBIDDEN_KEYS {
        if lower.contains(key) {
            return Err(AshkorixError::Config(format!(
                "remote/cloud config key '{key}' is not allowed in local-only mode"
            )));
        }
    }
    Ok(())
}

pub fn init_logging(config: &AshkorixConfig) -> Result<()> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let file_appender = tracing_appender::rolling::daily(config.logs_dir(), "ashkorix.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    Box::leak(Box::new(_guard));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    Ok(())
}

pub fn discover_gguf_models(models_dir: &Path) -> Result<Vec<ModelFileInfo>> {
    let mut models = Vec::new();
    if !models_dir.exists() {
        return Ok(models);
    }
    for entry in walkdir::WalkDir::new(models_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("gguf") {
            let metadata = std::fs::metadata(path)?;
            models.push(ModelFileInfo {
                path: path.to_path_buf(),
                filename: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                size_bytes: metadata.len(),
            });
        }
    }
    models.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(models)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFileInfo {
    pub path: PathBuf,
    pub filename: String,
    pub size_bytes: u64,
}
