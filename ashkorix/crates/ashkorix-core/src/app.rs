use crate::chunking::HeadingHierarchyChunker;
use crate::config::{
    resolve_embedding_model_path, AshkorixConfig, GenerationConfig, ModelFileInfo,
};
use crate::documents::registry::{dedup_result, ImporterRegistry};
use crate::documents::storage::DocumentStore;
use crate::documents::types::{Document, ImportResult, ImportStatus};
use crate::embeddings::LlamaEmbeddingService;
use crate::error::Result;
use crate::llm::LlamaModelService;
use crate::pool::POOL_ID;
use crate::rag::answer::RagAnswerService;
use crate::rag::types::{RagAnswer, RetrievalFilters, RetrievalMode};
use crate::rag::HybridRetrievalService;
use crate::search::indexer::{IndexHealth, PoolIndexer};
use crate::traits::model::{ChatMessage, GenerateParams, LoadOptions, ModelInfo, TokenEvent};
use crate::traits::{EmbeddingService, ModelService, RetrievalService};
use futures::Stream;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct AppState {
    pub config: AshkorixConfig,
    pub store: Arc<DocumentStore>,
    pub model: Arc<Mutex<LlamaModelService>>,
    pub embedding: Arc<Mutex<LlamaEmbeddingService>>,
    pub importers: ImporterRegistry,
    pub retrieval: HybridRetrievalService,
    pub rag: RagAnswerService,
    pub indexer: PoolIndexer,
    chunker: HeadingHierarchyChunker,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let config = AshkorixConfig::load()?;
        let _ = crate::config::init_logging(&config);
        let db_path = config.data_dir.join("ashkorix.db");
        let store = Arc::new(DocumentStore::open(&db_path)?);
        let model = Arc::new(Mutex::new(LlamaModelService::new()?));
        let embedding = Arc::new(Mutex::new(LlamaEmbeddingService::new()?));
        let importers = ImporterRegistry::builtin();
        let retrieval = HybridRetrievalService::new(
            config.clone(),
            store.clone(),
            embedding.clone(),
        );
        let indexer = PoolIndexer::new(config.clone(), store.clone(), embedding.clone());
        let rag = RagAnswerService::new(
            HybridRetrievalService::new(config.clone(), store.clone(), embedding.clone()),
            model.clone(),
        );
        let chunker = HeadingHierarchyChunker::new(config.chunking.clone());

        let mut state = Self {
            config,
            store,
            model,
            embedding,
            importers,
            retrieval,
            rag,
            indexer,
            chunker,
        };

        if let Err(e) = futures::executor::block_on(state.reload_embedding_from_config()) {
            tracing::warn!("embedding model not loaded at startup: {e}");
        }

        Ok(state)
    }

    fn refresh_retrieval_services(&mut self) {
        self.indexer = PoolIndexer::new(
            self.config.clone(),
            self.store.clone(),
            self.embedding.clone(),
        );
        self.retrieval = HybridRetrievalService::new(
            self.config.clone(),
            self.store.clone(),
            self.embedding.clone(),
        );
        self.rag = RagAnswerService::new(
            HybridRetrievalService::new(
                self.config.clone(),
                self.store.clone(),
                self.embedding.clone(),
            ),
            self.model.clone(),
        );
    }

    pub async fn reload_embedding_from_config(&mut self) -> Result<()> {
        let path = resolve_embedding_model_path(
            self.config.embedding_model_path.as_deref(),
            &self.config.models_dir,
        )
        .ok_or_else(|| {
            crate::error::AshkorixError::Config(
                "No embedding model found. Set embedding_model_path to a .gguf file or a \
                 folder containing one (e.g. Data/models/embeddings/)."
                    .into(),
            )
        })?;
        self.embedding.lock().await.load(&path).await?;
        self.config.embedding_model_path = Some(path);
        self.config.save()?;
        Ok(())
    }

    pub fn version() -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    pub fn list_models(&self) -> Result<Vec<ModelFileInfo>> {
        crate::config::discover_gguf_models(&self.config.models_dir)
    }

    pub async fn load_model(&self, path: PathBuf, options: LoadOptions) -> Result<()> {
        self.model.lock().await.load(&path, options).await
    }

    pub async fn unload_model(&self) -> Result<()> {
        self.model.lock().await.unload().await
    }

    pub fn model_info(&self) -> Option<ModelInfo> {
        futures::executor::block_on(async { self.model.lock().await.is_loaded() })
    }

    pub async fn chat_stream(
        &self,
        message: String,
        gen: GenerationConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TokenEvent>> + Send>>> {
        let mut model = self.model.lock().await;
        model.add_message(ChatMessage {
            role: "user".into(),
            content: message,
        });
        let prompt = model.format_conversation()?;
        model.generate_stream(GenerateParams {
            prompt,
            temperature: gen.temperature,
            top_p: gen.top_p,
            top_k: gen.top_k,
            repeat_penalty: gen.repeat_penalty,
            max_tokens: gen.max_tokens,
            seed: gen.seed,
            stop_sequences: gen.stop_sequences.clone(),
        })
    }

    pub async fn cancel_generation(&self) {
        self.model.lock().await.cancel();
    }

    pub fn clear_conversation(&self) {
        futures::executor::block_on(async {
            self.model.lock().await.clear_conversation();
        });
    }

    pub fn get_conversation(&self) -> Vec<ChatMessage> {
        futures::executor::block_on(async { self.model.lock().await.conversation() })
    }

    pub async fn import_files(&self, paths: Vec<PathBuf>) -> Result<Vec<ImportResult>> {
        std::fs::create_dir_all(self.config.documents_dir())?;
        let mut results = Vec::new();
        for path in paths {
            let result = self.import_single(&path).await?;
            results.push(result);
        }
        Ok(results)
    }

    async fn import_single(&self, path: &Path) -> Result<ImportResult> {
        let mut doc = self.importers.import_file(path).await?;
        if let Some(existing) = self.store.find_by_hash(&doc.content_hash)? {
            return Ok(dedup_result(Some(existing), doc));
        }

        let dest_dir = self
            .config
            .documents_dir()
            .join(&doc.id.0);
        std::fs::create_dir_all(&dest_dir)?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin");
        let dest = dest_dir.join(format!("original.{ext}"));
        std::fs::copy(path, &dest)?;
        doc.file_path = dest;
        doc.collection_id = POOL_ID.to_string();
        doc.import_status = ImportStatus::Imported;
        self.store.insert_document(&doc)?;

        let graph = self.chunker.chunk_with_graph(&doc, POOL_ID)?;
        self.store.insert_sections(&graph.sections)?;
        self.store.insert_tables(&graph.tables)?;
        self.store.insert_chunks(&graph.chunks)?;
        self.store.insert_entities(&graph.entities)?;
        self.store.insert_relations(&graph.relations)?;
        doc.chunk_count = graph.chunks.len() as u32;
        doc.import_status = ImportStatus::Chunked;

        Ok(ImportResult {
            document: Some(doc),
            status: ImportStatus::Chunked,
            message: "imported and chunked".into(),
        })
    }

    pub fn list_documents(&self) -> Result<Vec<Document>> {
        self.store.list_documents()
    }

    pub fn delete_document(&self, id: &str) -> Result<()> {
        self.store.delete_document(id)
    }

    pub async fn build_index(&mut self) -> Result<IndexHealth> {
        if !self.embedding.lock().await.is_loaded() {
            self.reload_embedding_from_config().await?;
        }
        self.indexer.build_index().await
    }

    pub async fn rebuild_index(&mut self) -> Result<IndexHealth> {
        if !self.embedding.lock().await.is_loaded() {
            self.reload_embedding_from_config().await?;
        }
        self.indexer.rebuild_index().await
    }

    pub fn index_health(&self) -> Result<IndexHealth> {
        self.indexer.health()
    }

    pub async fn retrieve(
        &self,
        query: &str,
        mode: RetrievalMode,
        exclude: Vec<String>,
        filters: RetrievalFilters,
    ) -> Result<Vec<crate::rag::types::RankedChunk>> {
        self.retrieval
            .retrieve(query, mode, &exclude, &filters)
            .await
    }

    pub async fn ask(
        &self,
        question: &str,
        mode: RetrievalMode,
        gen: GenerationConfig,
        exclude: Vec<String>,
        filters: RetrievalFilters,
    ) -> Result<RagAnswer> {
        let conversation = self.get_conversation();
        self.rag
            .ask(
                question,
                mode,
                GenerateParams {
                    prompt: String::new(),
                    temperature: gen.temperature,
                    top_p: gen.top_p,
                    top_k: gen.top_k,
                    repeat_penalty: gen.repeat_penalty,
                    max_tokens: gen.max_tokens,
                    seed: gen.seed,
                    stop_sequences: gen.stop_sequences,
                },
                &exclude,
                &conversation,
                &filters,
            )
            .await
    }

    pub fn doctor(&self) -> DoctorReport {
        let _ = self.config.ensure_dirs();
        let (resolved_data_dir, source) = crate::config::data_dir_source();
        let gguf_count = crate::config::discover_gguf_models(&self.config.models_dir)
            .map(|m| m.len())
            .unwrap_or(0);

        let mut checks = vec![
            DoctorCheck {
                name: "data_dir_source".into(),
                path: resolved_data_dir.display().to_string(),
                ok: true,
                message: format!("source={}", source.as_str()),
            },
            check_path("data_dir", &self.config.data_dir),
            check_path("models_dir", &self.config.models_dir),
            check_path("documents_dir", &self.config.documents_dir()),
            check_path("index_dir", &self.config.index_dir()),
            DoctorCheck {
                name: "gguf_models".into(),
                path: self.config.models_dir.display().to_string(),
                ok: self.config.models_dir.exists(),
                message: format!("{gguf_count} .gguf file(s) discovered"),
            },
        ];
        let embedding_ok = futures::executor::block_on(async {
            self.embedding.lock().await.is_loaded()
        });
        let embedding_path = self
            .config
            .embedding_model_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not set)".into());
        checks.push(DoctorCheck {
            name: "embedding_model".into(),
            path: embedding_path,
            ok: embedding_ok,
            message: if embedding_ok {
                "loaded".into()
            } else {
                "not loaded — set a .gguf embedding file in Settings".into()
            },
        });
        checks.sort_by(|a, b| a.name.cmp(&b.name));
        DoctorReport {
            local_only: self.config.local_only,
            checks,
        }
    }

    pub async fn load_embedding_model(&mut self, path: PathBuf) -> Result<()> {
        self.embedding.lock().await.load(&path).await?;
        self.config.embedding_model_path = Some(path);
        self.config.save()
    }

    pub async fn update_config(&mut self, mut config: AshkorixConfig) -> Result<()> {
        config.normalize_model_paths();
        config.save()?;
        self.config = config;
        self.refresh_retrieval_services();
        if let Err(e) = self.reload_embedding_from_config().await {
            tracing::warn!("embedding model not reloaded after config save: {e}");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub path: String,
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DoctorReport {
    pub local_only: bool,
    pub checks: Vec<DoctorCheck>,
}

fn check_path(name: &str, path: &Path) -> DoctorCheck {
    let exists = path.exists();
    let writable = if exists {
        path.metadata()
            .map(|m| !m.permissions().readonly())
            .unwrap_or(false)
    } else {
        std::fs::create_dir_all(path).is_ok()
    };
    DoctorCheck {
        name: name.to_string(),
        path: path.display().to_string(),
        ok: exists && writable,
        message: if exists && writable {
            "ok".into()
        } else if !exists {
            "created or missing".into()
        } else {
            "not writable".into()
        },
    }
}

pub fn run_doctor() -> Result<DoctorReport> {
    let state = AppState::new()?;
    Ok(state.doctor())
}
