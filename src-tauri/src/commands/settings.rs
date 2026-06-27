use tauri::{Manager, State};

use crate::app_store;
use crate::db::MEMORY_DB_URL;
use crate::state::{AppSettings, AppState};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppPaths {
    pub app_data_dir: String,
    pub memory_db_url: String,
    pub model_cache_dir: String,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().map_err(|error| error.to_string())?;
    Ok(settings.clone())
}

#[tauri::command]
pub async fn save_settings(
    settings: AppSettings,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AppSettings, String> {
    let mut app_settings = state.settings.lock().map_err(|error| error.to_string())?;
    *app_settings = settings.clone();
    app_store::save_settings(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub async fn get_app_paths(app: tauri::AppHandle) -> Result<AppPaths, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let model_cache_dir = app_data_dir.join("models");

    Ok(AppPaths {
        app_data_dir: app_data_dir.to_string_lossy().into_owned(),
        memory_db_url: MEMORY_DB_URL.to_string(),
        model_cache_dir: model_cache_dir.to_string_lossy().into_owned(),
    })
}
