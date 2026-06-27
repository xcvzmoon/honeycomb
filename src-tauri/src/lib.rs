#![allow(dead_code, unused_imports)]

use tauri::Manager;

mod agent;
mod app_store;
mod commands;
mod db;
mod llm;
mod memory;
mod secure_store;
mod state;
mod streaming;
mod tools;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(state::AppState::default())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::agent::list_agents,
            commands::agent::create_agent,
            commands::agent::update_agent,
            commands::agent::delete_agent,
            commands::agent::get_agent,
            commands::chat::list_chat_sessions,
            commands::chat::create_chat_session,
            commands::chat::get_chat_messages,
            commands::chat::rename_chat_session,
            commands::chat::delete_chat_session,
            commands::chat::send_message,
            commands::chat::cancel_stream,
            commands::chat::retry_last,
            commands::memory::get_memory_overview,
            commands::memory::list_pinned_facts,
            commands::memory::list_episodes,
            commands::memory::add_identity_override,
            commands::memory::remove_identity_override,
            commands::memory::clear_memory,
            commands::memory::run_consolidation,
            commands::memory::sync_now,
            commands::memory::retrieve_memory,
            commands::memory::import_tokenizer,
            commands::memory::download_tokenizer,
            commands::memory::get_tokenizer_status,
            commands::memory::enqueue_memory_backfill,
            commands::memory::process_embedding_jobs,
            commands::provider::save_provider_config,
            commands::provider::get_provider_config,
            commands::provider::save_embed_config,
            commands::provider::get_embed_config,
            commands::provider::test_provider,
            commands::provider::test_chat_provider,
            commands::provider::test_embed_provider,
            commands::tools::list_tools,
            commands::tools::set_tool_enabled,
            commands::tools::execute_tool,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::get_app_paths,
            commands::dev::get_embedding_job_stats,
            commands::dev::list_database_tables,
            commands::dev::preview_database_rows,
            commands::dev::get_internal_config_snapshot,
        ])
        .setup(|app| {
            app_store::hydrate_state(app.handle())?;
            let pool = tauri::async_runtime::block_on(db::initialize(app.handle()))?;
            app.manage(pool);

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
