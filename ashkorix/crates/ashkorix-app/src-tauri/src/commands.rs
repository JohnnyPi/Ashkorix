use crate::state::AppStateWrapper;
use ashkorix_core::app::{AppState as CoreState, DoctorReport};
use ashkorix_core::config::{AshkorixConfig, GenerationConfig, ModelFileInfo};
use ashkorix_core::documents::registry::ImporterInfo;
use ashkorix_core::documents::types::{Document, ImportResult};
use ashkorix_core::rag::types::{RagAnswer, RankedChunk, RetrievalFilters, RetrievalMode};
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
    let mut stream = state
        .0
        .lock()
        .await
        .chat_stream(message, gen)
        .await
        .map_err(|e| e.to_string())?;

    tokio::spawn(async move {
        while let Some(event) = stream.next().await {
            match event {
                Ok(ev) => {
                    let _ = app.emit(
                        "token",
                        TokenPayload {
                            token: ev.token,
                            finished: ev.finished,
                            tokens_generated: ev.tokens_generated,
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
                        },
                    );
                    break;
                }
            }
        }
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
pub async fn import_files(
    app: AppHandle,
    state: State<'_, AppStateWrapper>,
    paths: Vec<PathBuf>,
) -> Result<Vec<ImportResult>, String> {
    let mut results = Vec::new();
    for path in paths {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        let result = state
            .0
            .lock()
            .await
            .import_files(vec![path])
            .await
            .map_err(|e| e.to_string())?;
        for r in &result {
            let _ = app.emit(
                "import_progress",
                ImportProgressPayload {
                    filename: filename.clone(),
                    status: format!("{:?}", r.status),
                    message: r.message.clone(),
                },
            );
        }
        results.extend(result);
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
