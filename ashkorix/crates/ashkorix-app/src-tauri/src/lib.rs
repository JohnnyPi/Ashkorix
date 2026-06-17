mod commands;
mod state;

use state::AppStateWrapper;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppStateWrapper::new().expect("failed to init app state");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::open_data_folder,
            commands::get_version,
            commands::get_cuda_status,
            commands::get_config,
            commands::list_models,
            commands::load_model,
            commands::unload_model,
            commands::get_model_info,
            commands::chat_stream_start,
            commands::rag_stream_start,
            commands::cancel_generation,
            commands::clear_conversation,
            commands::get_generation_settings,
            commands::set_generation_settings,
            commands::list_importers,
            commands::import_files,
            commands::list_documents,
            commands::delete_document,
            commands::build_index,
            commands::rebuild_index,
            commands::index_health,
            commands::retrieve,
            commands::ask,
            commands::doctor,
            commands::update_config,
            commands::save_conversation,
            commands::list_memories,
            commands::search_memories,
            commands::list_memory_candidates,
            commands::approve_memory_candidate,
            commands::reject_memory_candidate,
            commands::edit_and_approve_candidate,
            commands::create_memory,
            commands::update_memory,
            commands::deactivate_memory,
            commands::extract_memory_candidates,
            commands::get_last_injected_memories,
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
