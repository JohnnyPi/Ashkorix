use crate::chunking::HeadingHierarchyChunker;
use crate::config::{
    resolve_embedding_model_path, resolve_reranker_model_path, AshkorixConfig, GenerationConfig,
    ModelFileInfo,
};
use crate::documents::registry::{dedup_result, ImporterRegistry};
use crate::documents::storage::DocumentStore;
use crate::documents::types::{Document, ImportResult, ImportStatus};
use crate::embeddings::LlamaEmbeddingService;
use crate::error::Result;
use crate::llm::LlamaModelService;
use crate::memory::{
    augment_last_user_message, build_chat_memory_system_prompt, build_extraction_prompt,
    instant_text_stream, parse_extraction_response, try_direct_memory_answer, CreateMemoryInput,
    EditCandidateInput, Memory, MemoryCandidate, MemoryRetriever, MemoryStore, UpdateMemoryInput,
};
use crate::pool::POOL_ID;
use crate::rag::answer::RagAnswerService;
use crate::rag::types::{RagAnswer, RetrievalFilters, RetrievalMode};
use crate::rag::HybridRetrievalService;
use crate::rerank::LlamaRerankerService;
use crate::search::indexer::{IndexHealth, PoolIndexer};
use crate::traits::model::{ChatMessage, GenerateParams, LoadOptions, ModelInfo, TokenEvent};
use crate::traits::{EmbeddingService, ModelService, RetrievalService};
use futures::Stream;
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

pub struct AppState {
    pub config: AshkorixConfig,
    pub store: Arc<DocumentStore>,
    pub memory: Arc<MemoryStore>,
    pub model: Arc<AsyncMutex<LlamaModelService>>,
    pub embedding: Arc<AsyncMutex<LlamaEmbeddingService>>,
    pub reranker: Arc<AsyncMutex<LlamaRerankerService>>,
    pub importers: ImporterRegistry,
    pub indexer: Arc<PoolIndexer>,
    pub retrieval: Arc<HybridRetrievalService>,
    pub rag: RagAnswerService,
    chunker: HeadingHierarchyChunker,
    session_id: Mutex<String>,
    last_injected_memories: Mutex<Vec<Memory>>,
}

impl AppState {
    pub fn new() -> Result<Self> {
        let config = AshkorixConfig::load()?;
        let _ = crate::config::init_logging(&config);
        let db_path = config.data_dir.join("ashkorix.db");
        let store = Arc::new(DocumentStore::open(&db_path)?);
        let memory = Arc::new(MemoryStore::open(&db_path)?);
        memory.seed_if_empty()?;
        let model = Arc::new(AsyncMutex::new(LlamaModelService::new()?));
        let embedding = Arc::new(AsyncMutex::new(LlamaEmbeddingService::new()?));
        let reranker = Arc::new(AsyncMutex::new(LlamaRerankerService::new(
            i32::try_from(config.generation.threads).unwrap_or(4),
        )?));
        let importers = ImporterRegistry::builtin();
        let indexer = Arc::new(PoolIndexer::new(
            config.clone(),
            store.clone(),
            embedding.clone(),
        ));
        let retrieval = Arc::new(HybridRetrievalService::new(
            config.clone(),
            store.clone(),
            embedding.clone(),
            indexer.clone(),
            reranker.clone(),
        ));
        let rag = RagAnswerService::new(retrieval.clone(), model.clone());
        let chunker = HeadingHierarchyChunker::new(config.chunking.clone());

        let mut state = Self {
            config,
            store,
            memory,
            model,
            embedding,
            reranker,
            importers,
            indexer,
            retrieval,
            rag,
            chunker,
            session_id: Mutex::new(new_session_id()),
            last_injected_memories: Mutex::new(Vec::new()),
        };

        if let Err(e) = futures::executor::block_on(state.reload_embedding_from_config()) {
            tracing::warn!("embedding model not loaded at startup: {e}");
        }
        if let Err(e) = futures::executor::block_on(state.reload_reranker_from_config()) {
            tracing::warn!("reranker model not loaded at startup: {e}");
        }

        Ok(state)
    }

    fn refresh_retrieval_services(&mut self) {
        self.indexer = Arc::new(PoolIndexer::new(
            self.config.clone(),
            self.store.clone(),
            self.embedding.clone(),
        ));
        self.retrieval = Arc::new(HybridRetrievalService::new(
            self.config.clone(),
            self.store.clone(),
            self.embedding.clone(),
            self.indexer.clone(),
            self.reranker.clone(),
        ));
        self.rag = RagAnswerService::new(self.retrieval.clone(), self.model.clone());
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

    pub async fn reload_reranker_from_config(&mut self) -> Result<()> {
        let path = resolve_reranker_model_path(
            self.config.reranker_model_path.as_deref(),
            &self.config.models_dir,
        );

        match path {
            Some(path) => {
                self.reranker.lock().await.load(&path).await?;
                self.config.reranker_model_path = Some(path);
                self.config.save()?;
            }
            None => {
                self.reranker.lock().await.unload();
            }
        }
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

    pub fn session_id(&self) -> String {
        self.session_id.lock().unwrap().clone()
    }

    pub fn last_injected_memories(&self) -> Vec<Memory> {
        self.last_injected_memories.lock().unwrap().clone()
    }

    fn set_last_injected_memories(&self, memories: Vec<Memory>) {
        *self.last_injected_memories.lock().unwrap() = memories;
    }

    fn new_session(&self) {
        *self.session_id.lock().unwrap() = new_session_id();
    }

    pub async fn retrieve_memories(&self, query: &str) -> Result<Vec<Memory>> {
        let memories = MemoryRetriever::retrieve(
            &self.memory,
            &self.store,
            self.embedding.clone(),
            &self.config.memory,
            &self.session_id(),
            query,
        )
        .await?;
        self.set_last_injected_memories(memories.clone());
        Ok(memories)
    }

    pub fn append_assistant_message(&self, content: String) {
        if content.is_empty() {
            return;
        }
        futures::executor::block_on(async {
            self.model.lock().await.add_message(ChatMessage {
                role: "assistant".into(),
                content,
            });
        });
    }

    pub fn append_user_message(&self, content: String) {
        futures::executor::block_on(async {
            self.model.lock().await.add_message(ChatMessage {
                role: "user".into(),
                content,
            });
        });
    }

    pub async fn chat_stream(
        &self,
        message: String,
        gen: GenerationConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TokenEvent>> + Send>>> {
        let memories = self.retrieve_memories(&message).await?;

        if let Some(answer) = try_direct_memory_answer(&message, &memories) {
            let model = self.model.lock().await;
            model.add_message(ChatMessage {
                role: "user".into(),
                content: message,
            });
            return Ok(instant_text_stream(answer));
        }

        let memory_block = build_chat_memory_system_prompt(&memories);

        let mut model = self.model.lock().await;
        model.add_message(ChatMessage {
            role: "user".into(),
            content: message,
        });

        let mut messages = model.conversation();
        if !memory_block.is_empty() {
            messages.insert(
                0,
                ChatMessage {
                    role: "system".into(),
                    content: memory_block,
                },
            );
            augment_last_user_message(&mut messages, &memories);
        }
        let prompt = model.format_messages(&messages)?;
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

    pub async fn rag_stream(
        &self,
        question: String,
        mode: RetrievalMode,
        gen: GenerationConfig,
        exclude: Vec<String>,
        filters: RetrievalFilters,
    ) -> Result<(
        Pin<Box<dyn Stream<Item = Result<TokenEvent>> + Send>>,
        crate::rag::answer::RagStreamMeta,
    )> {
        let memories = self.retrieve_memories(&question).await?;

        self.append_user_message(question.clone());
        let conversation = self.get_conversation();
        self.rag
            .stream_answer(
                &question,
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
                &memories,
            )
            .await
    }

    pub async fn cancel_generation(&self) {
        self.model.lock().await.cancel();
    }

    pub fn clear_conversation(&self) {
        futures::executor::block_on(async {
            self.model.lock().await.clear_conversation();
        });
        self.new_session();
        self.set_last_injected_memories(Vec::new());
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

    pub async fn delete_document(&self, id: &str) -> Result<()> {
        let chunk_ids: Vec<String> = self
            .store
            .list_chunks_for_document(id)?
            .into_iter()
            .map(|c| c.id.0)
            .collect();
        let dim = self.embedding.lock().await.dimension();
        if dim > 0 && !chunk_ids.is_empty() {
            self.indexer.remove_chunks(&chunk_ids, dim)?;
        }
        self.store.delete_document(id)?;
        Ok(())
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
        let memories = self.retrieve_memories(question).await?;
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
                &memories,
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
        let reranker_ok = futures::executor::block_on(async { self.reranker.lock().await.is_loaded() });
        let reranker_path = self
            .config
            .reranker_model_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(not set — heuristic rerank)".into());
        checks.push(DoctorCheck {
            name: "reranker_model".into(),
            path: reranker_path,
            ok: self.config.reranker_model_path.is_none() || reranker_ok,
            message: if self.config.reranker_model_path.is_none() {
                "not configured — using heuristic rerank".into()
            } else if reranker_ok {
                "loaded".into()
            } else {
                "configured but not loaded — check path in Settings".into()
            },
        });
        let cuda = crate::llm::cuda_status();
        checks.push(DoctorCheck {
            name: "cuda".into(),
            path: cuda
                .device_name
                .clone()
                .unwrap_or_else(|| "(none)".into()),
            ok: cuda.available,
            message: if !cuda.compiled {
                "CPU-only build (cuda feature not enabled)".into()
            } else if cuda.available {
                format!(
                    "CUDA active — {}",
                    cuda.device_name.as_deref().unwrap_or("GPU detected")
                )
            } else {
                "CUDA compiled but no compatible GPU found — using CPU".into()
            },
        });
        checks.sort_by(|a, b| a.name.cmp(&b.name));
        DoctorReport {
            local_only: self.config.local_only,
            checks,
        }
    }

    pub async fn update_config(&mut self, mut config: AshkorixConfig) -> Result<()> {
        config.normalize_model_paths();
        config.save()?;
        self.config = config;
        self.refresh_retrieval_services();
        if let Err(e) = self.reload_embedding_from_config().await {
            tracing::warn!("embedding model not reloaded after config save: {e}");
        }
        if let Err(e) = self.reload_reranker_from_config().await {
            tracing::warn!("reranker model not reloaded after config save: {e}");
        }
        Ok(())
    }

    pub fn list_memories(&self, scope_filter: Option<&str>) -> Result<Vec<Memory>> {
        self.memory.list_active(scope_filter)
    }

    pub fn search_memories(&self, query: &str, limit: usize) -> Result<Vec<Memory>> {
        self.memory.search(query, limit)
    }

    pub fn list_memory_candidates(&self) -> Result<Vec<MemoryCandidate>> {
        self.memory.list_pending_candidates()
    }

    pub fn approve_memory_candidate(&self, id: &str) -> Result<Memory> {
        self.memory.approve_candidate(id)
    }

    pub fn reject_memory_candidate(&self, id: &str) -> Result<()> {
        self.memory.reject_candidate(id)
    }

    pub fn edit_and_approve_candidate(
        &self,
        id: &str,
        edit: EditCandidateInput,
    ) -> Result<Memory> {
        self.memory.edit_and_approve_candidate(id, &edit)
    }

    pub fn create_memory(&self, input: CreateMemoryInput) -> Result<Memory> {
        self.memory.insert(&input)
    }

    pub fn update_memory(&self, id: &str, input: UpdateMemoryInput) -> Result<Memory> {
        self.memory.update(id, &input)
    }

    pub fn deactivate_memory(&self, id: &str) -> Result<()> {
        self.memory.deactivate(id)
    }

    pub fn supersede_memory(&self, old_id: &str, new_id: &str) -> Result<()> {
        self.memory.mark_superseded(old_id, new_id)
    }

    pub async fn extract_memory_candidates(&self) -> Result<Vec<MemoryCandidate>> {
        let conversation = self.get_conversation();
        if conversation.is_empty() {
            return Ok(vec![]);
        }

        let transcript: String = conversation
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let project_scope = self.config.memory.project_scope();
        let prompt = build_extraction_prompt(
            &transcript,
            &project_scope,
            self.config.memory.extraction_min_confidence,
        );

        let gen = self.config.generation.clone();
        let mut model = self.model.lock().await;
        let mut stream = model.generate_stream(GenerateParams {
            prompt,
            temperature: gen.temperature,
            top_p: gen.top_p,
            top_k: gen.top_k,
            repeat_penalty: gen.repeat_penalty,
            max_tokens: gen.max_tokens,
            seed: gen.seed,
            stop_sequences: gen.stop_sequences,
        })?;

        let mut response = String::new();
        while let Some(event) = stream.next().await {
            let event = event?;
            response.push_str(&event.token);
            if event.finished {
                break;
            }
        }
        drop(model);

        let extracted = parse_extraction_response(&response)?;
        let min_conf = self.config.memory.extraction_min_confidence;
        let session = self.session_id();
        let mut created = Vec::new();

        for item in extracted {
            if item.confidence < min_conf {
                continue;
            }
            if self
                .memory
                .has_active_duplicate(&item.proposed_scope, &item.proposed_content)?
            {
                continue;
            }
            if self
                .memory
                .pending_candidate_exists(&item.proposed_scope, &item.proposed_content)?
            {
                continue;
            }
            let candidate = self.memory.insert_candidate(
                item.proposed_type,
                &item.proposed_scope,
                &item.proposed_title,
                &item.proposed_content,
                item.importance,
                item.confidence,
                item.reason,
                Some("conversation".into()),
                Some(session.clone()),
            )?;
            created.push(candidate);
        }

        Ok(created)
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

fn new_session_id() -> String {
    Uuid::new_v4().to_string()
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
