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
            commands::get_config,
            commands::list_models,
            commands::load_model,
            commands::unload_model,
            commands::get_model_info,
            commands::chat_stream_start,
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
        ])
        .run(tauri::generate_context!())
        .expect("error running tauri application");
}
