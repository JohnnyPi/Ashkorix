use crate::state::AppStateWrapper;
use ashkorix_core::app::{AppState as CoreState, DoctorReport};
use ashkorix_core::config::{AshkorixConfig, GenerationConfig, ModelFileInfo};
use ashkorix_core::CudaStatus;
use ashkorix_core::documents::registry::ImporterInfo;
use ashkorix_core::documents::types::{Document, ImportResult};
use ashkorix_core::rag::answer::RagAnswerService;
use ashkorix_core::rag::types::{RagAnswer, RankedChunk, RetrievalFilters, RetrievalMode, UnsupportedClaim};
use ashkorix_core::search::indexer::IndexHealth;
use ashkorix_core::traits::model::{LoadOptions, ModelInfo};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPayload {
    pub token: String,
    pub finished: bool,
    pub tokens_generated: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub citations: Option<Vec<ashkorix_core::cite::types::Citation>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncited_warning: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsupported_claims: Option<Vec<UnsupportedClaim>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportProgressPayload {
    pub filename: String,
    pub status: String,
    pub message: String,
}

#[tauri::command]
pub async fn open_data_folder(
    app: tauri::AppHandle,
    state: State<'_, AppStateWrapper>,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let config = state.0.lock().await.config.clone();
    config.ensure_dirs().map_err(|e| e.to_string())?;
    app.opener()
        .reveal_item_in_dir(&config.data_dir)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_version() -> Result<String, String> {
    Ok(CoreState::version().to_string())
}

#[tauri::command]
pub async fn get_cuda_status() -> Result<CudaStatus, String> {
    Ok(ashkorix_core::cuda_status())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppStateWrapper>) -> Result<AshkorixConfig, String> {
    Ok(state.0.lock().await.config.clone())
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppStateWrapper>) -> Result<Vec<ModelFileInfo>, String> {
    state.0.lock().await.list_models().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_model(
    state: State<'_, AppStateWrapper>,
    path: PathBuf,
    options: LoadOptions,
) -> Result<(), String> {
    state
        .0
        .lock()
        .await
        .load_model(path, options)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unload_model(state: State<'_, AppStateWrapper>) -> Result<(), String> {
    state
        .0
        .lock()
        .await
        .unload_model()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_model_info(state: State<'_, AppStateWrapper>) -> Result<Option<ModelInfo>, String> {
    Ok(state.0.lock().await.model_info())
}

#[tauri::command]
pub async fn chat_stream_start(
    app: AppHandle,
    state: State<'_, AppStateWrapper>,
    message: String,
) -> Result<(), String> {
    let gen = state.0.lock().await.config.generation.clone();
    let state_arc = state.0.clone();
    let mut stream = state
        .0
        .lock()
        .await
        .chat_stream(message, gen)
        .await
        .map_err(|e| e.to_string())?;

    tokio::spawn(async move {
        let mut assistant = String::new();
        while let Some(event) = stream.next().await {
            match event {
                Ok(ev) => {
                    assistant.push_str(&ev.token);
                    let _ = app.emit(
                        "token",
                        TokenPayload {
                            token: ev.token,
                            finished: ev.finished,
                            tokens_generated: ev.tokens_generated,
                            citations: None,
                            uncited_warning: None,
                            unsupported_claims: None,
                        },
                    );
                    if ev.finished {
                        break;
                    }
                }
                Err(e) => {
                    let _ = app.emit(
                        "token",
                        TokenPayload {
                            token: format!("[error: {e}]"),
                            finished: true,
                            tokens_generated: 0,
                            citations: None,
                            uncited_warning: None,
                            unsupported_claims: None,
                        },
                    );
                    break;
                }
            }
        }
        state_arc
            .lock()
            .await
            .append_assistant_message(assistant);
    });
    Ok(())
}

#[tauri::command]
pub async fn cancel_generation(state: State<'_, AppStateWrapper>) -> Result<(), String> {
    state.0.lock().await.cancel_generation().await;
    Ok(())
}

#[tauri::command]
pub async fn clear_conversation(state: State<'_, AppStateWrapper>) -> Result<(), String> {
    state.0.lock().await.clear_conversation();
    Ok(())
}

#[tauri::command]
pub async fn get_generation_settings(
    state: State<'_, AppStateWrapper>,
) -> Result<GenerationConfig, String> {
    Ok(state.0.lock().await.config.generation.clone())
}

#[tauri::command]
pub async fn set_generation_settings(
    state: State<'_, AppStateWrapper>,
    settings: GenerationConfig,
) -> Result<(), String> {
    let mut guard = state.0.lock().await;
    guard.config.generation = settings;
    guard.config.save().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_importers() -> Result<Vec<ImporterInfo>, String> {
    Ok(ashkorix_core::documents::registry::ImporterRegistry::builtin().list())
}

#[tauri::command]
pub async fn rag_stream_start(
    app: AppHandle,
    state: State<'_, AppStateWrapper>,
    question: String,
    mode: String,
) -> Result<(), String> {
    let gen = state.0.lock().await.config.generation.clone();
    let (mut stream, meta) = state
        .0
        .lock()
        .await
        .rag_stream(
            question,
            RetrievalMode::from_str(&mode),
            gen,
            vec![],
            RetrievalFilters::default(),
        )
        .await
        .map_err(|e| e.to_string())?;

    let citations = meta.citations;
    let retrieved_chunks = meta.retrieved_chunks;
    let state_arc = state.0.clone();
    tokio::spawn(async move {
        let mut assistant = String::new();
        while let Some(event) = stream.next().await {
            match event {
                Ok(ev) => {
                    assistant.push_str(&ev.token);
                    let payload = TokenPayload {
                        token: ev.token,
                        finished: ev.finished,
                        tokens_generated: ev.tokens_generated,
                        citations: None,
                        uncited_warning: None,
                        unsupported_claims: None,
                    };
                    let _ = app.emit("token", payload);
                    if ev.finished {
                        break;
                    }
                }
                Err(e) => {
                    let _ = app.emit(
                        "token",
                        TokenPayload {
                            token: format!("[error: {e}]"),
                            finished: true,
                            tokens_generated: 0,
                            citations: None,
                            uncited_warning: None,
                            unsupported_claims: None,
                        },
                    );
                    break;
                }
            }
        }

        let verification = {
            let memories = state_arc.lock().await.last_injected_memories();
            RagAnswerService::verify_response(
                &assistant,
                &citations,
                &retrieved_chunks,
                &memories,
            )
        };

        let _ = app.emit(
            "token",
            TokenPayload {
                token: String::new(),
                finished: true,
                tokens_generated: 0,
                citations: Some(verification.resolved_citations),
                uncited_warning: Some(verification.uncited_warning),
                unsupported_claims: if verification.unsupported_claims.is_empty() {
                    None
                } else {
                    Some(verification.unsupported_claims)
                },
            },
        );

        state_arc
            .lock()
            .await
            .append_assistant_message(assistant);
    });
    Ok(())
}

#[tauri::command]
pub async fn import_files(
    app: AppHandle,
    state: State<'_, AppStateWrapper>,
    paths: Vec<PathBuf>,
) -> Result<Vec<ImportResult>, String> {
    let results = state
        .0
        .lock()
        .await
        .import_files(paths)
        .await
        .map_err(|e| e.to_string())?;
    for r in &results {
        let filename = r
            .document
            .as_ref()
            .map(|d| d.original_filename.clone())
            .unwrap_or_else(|| "unknown".into());
        let _ = app.emit(
            "import_progress",
            ImportProgressPayload {
                filename,
                status: format!("{:?}", r.status),
                message: r.message.clone(),
            },
        );
    }
    Ok(results)
}

#[tauri::command]
pub async fn list_documents(state: State<'_, AppStateWrapper>) -> Result<Vec<Document>, String> {
    state
        .0
        .lock()
        .await
        .list_documents()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_document(state: State<'_, AppStateWrapper>, id: String) -> Result<(), String> {
    state
        .0
        .lock()
        .await
        .delete_document(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn build_index(state: State<'_, AppStateWrapper>) -> Result<IndexHealth, String> {
    state
        .0
        .lock()
        .await
        .build_index()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rebuild_index(state: State<'_, AppStateWrapper>) -> Result<IndexHealth, String> {
    state
        .0
        .lock()
        .await
        .rebuild_index()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn index_health(state: State<'_, AppStateWrapper>) -> Result<IndexHealth, String> {
    state
        .0
        .lock()
        .await
        .index_health()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn retrieve(
    state: State<'_, AppStateWrapper>,
    query: String,
    mode: String,
    exclude: Vec<String>,
    filters: Option<RetrievalFilters>,
) -> Result<Vec<RankedChunk>, String> {
    state
        .0
        .lock()
        .await
        .retrieve(
            &query,
            RetrievalMode::from_str(&mode),
            exclude,
            filters.unwrap_or_default(),
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ask(
    state: State<'_, AppStateWrapper>,
    question: String,
    mode: String,
    exclude: Vec<String>,
    filters: Option<RetrievalFilters>,
) -> Result<RagAnswer, String> {
    let gen = state.0.lock().await.config.generation.clone();
    state
        .0
        .lock()
        .await
        .ask(
            &question,
            RetrievalMode::from_str(&mode),
            gen,
            exclude,
            filters.unwrap_or_default(),
        )
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn doctor(state: State<'_, AppStateWrapper>) -> Result<DoctorReport, String> {
    Ok(state.0.lock().await.doctor())
}

#[tauri::command]
pub async fn update_config(
    state: State<'_, AppStateWrapper>,
    config: AshkorixConfig,
) -> Result<(), String> {
    state
        .0
        .lock()
        .await
        .update_config(config)
        .await
        .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationExport {
    pub messages: Vec<ashkorix_core::traits::model::ChatMessage>,
    pub exported_at: String,
}

#[tauri::command]
pub async fn save_conversation(state: State<'_, AppStateWrapper>) -> Result<ConversationExport, String> {
    Ok(ConversationExport {
        messages: state.0.lock().await.get_conversation(),
        exported_at: chrono::Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
pub async fn list_memories(
    state: State<'_, AppStateWrapper>,
    scope_filter: Option<String>,
) -> Result<Vec<ashkorix_core::memory::Memory>, String> {
    state
        .0
        .lock()
        .await
        .list_memories(scope_filter.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_memories(
    state: State<'_, AppStateWrapper>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<ashkorix_core::memory::Memory>, String> {
    state
        .0
        .lock()
        .await
        .search_memories(&query, limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_memory_candidates(
    state: State<'_, AppStateWrapper>,
) -> Result<Vec<ashkorix_core::memory::MemoryCandidate>, String> {
    state
        .0
        .lock()
        .await
        .list_memory_candidates()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn approve_memory_candidate(
    state: State<'_, AppStateWrapper>,
    id: String,
) -> Result<ashkorix_core::memory::Memory, String> {
    state
        .0
        .lock()
        .await
        .approve_memory_candidate(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reject_memory_candidate(
    state: State<'_, AppStateWrapper>,
    id: String,
) -> Result<(), String> {
    state
        .0
        .lock()
        .await
        .reject_memory_candidate(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn edit_and_approve_candidate(
    state: State<'_, AppStateWrapper>,
    id: String,
    edit: ashkorix_core::memory::EditCandidateInput,
) -> Result<ashkorix_core::memory::Memory, String> {
    state
        .0
        .lock()
        .await
        .edit_and_approve_candidate(&id, edit)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_memory(
    state: State<'_, AppStateWrapper>,
    input: ashkorix_core::memory::CreateMemoryInput,
) -> Result<ashkorix_core::memory::Memory, String> {
    state
        .0
        .lock()
        .await
        .create_memory(input)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_memory(
    state: State<'_, AppStateWrapper>,
    id: String,
    input: ashkorix_core::memory::UpdateMemoryInput,
) -> Result<ashkorix_core::memory::Memory, String> {
    state
        .0
        .lock()
        .await
        .update_memory(&id, input)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn deactivate_memory(
    state: State<'_, AppStateWrapper>,
    id: String,
) -> Result<(), String> {
    state
        .0
        .lock()
        .await
        .deactivate_memory(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn extract_memory_candidates(
    state: State<'_, AppStateWrapper>,
) -> Result<Vec<ashkorix_core::memory::MemoryCandidate>, String> {
    state
        .0
        .lock()
        .await
        .extract_memory_candidates()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_last_injected_memories(
    state: State<'_, AppStateWrapper>,
) -> Result<Vec<ashkorix_core::memory::Memory>, String> {
    Ok(state.0.lock().await.last_injected_memories())
}
