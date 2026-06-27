use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_plugin_store::StoreExt;

use crate::state::{AgentDefinition, AppSettings, ProviderConfig};

const STORE_PATH: &str = "honeycomb.store.json";
const PROVIDER_CONFIG_KEY: &str = "providerConfig";
const EMBED_CONFIG_KEY: &str = "embedConfig";
const AGENTS_KEY: &str = "agents";
const SETTINGS_KEY: &str = "settings";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredAppData {
    pub provider_config: Option<ProviderConfig>,
    pub embed_config: Option<ProviderConfig>,
    pub agents: HashMap<String, AgentDefinition>,
    pub settings: AppSettings,
}

pub fn load(app: &tauri::AppHandle) -> Result<StoredAppData, String> {
    Ok(StoredAppData {
        provider_config: get_value(app, PROVIDER_CONFIG_KEY)?,
        embed_config: get_value(app, EMBED_CONFIG_KEY)?,
        agents: get_value(app, AGENTS_KEY)?.unwrap_or_default(),
        settings: get_value(app, SETTINGS_KEY)?.unwrap_or_default(),
    })
}

pub fn hydrate_state(app: &tauri::AppHandle) -> Result<(), String> {
    let data = load(app)?;
    let state = app.state::<crate::state::AppState>();

    *state
        .provider_config
        .lock()
        .map_err(|error| error.to_string())? = data.provider_config;
    *state
        .embed_config
        .lock()
        .map_err(|error| error.to_string())? = data.embed_config;
    *state.agents.lock().map_err(|error| error.to_string())? = data.agents;
    *state.settings.lock().map_err(|error| error.to_string())? = data.settings;

    Ok(())
}

pub fn save_provider_config(app: &tauri::AppHandle, config: &ProviderConfig) -> Result<(), String> {
    set_value(app, PROVIDER_CONFIG_KEY, config)
}

pub fn save_embed_config(app: &tauri::AppHandle, config: &ProviderConfig) -> Result<(), String> {
    set_value(app, EMBED_CONFIG_KEY, config)
}

pub fn save_agents(
    app: &tauri::AppHandle,
    agents: &HashMap<String, AgentDefinition>,
) -> Result<(), String> {
    set_value(app, AGENTS_KEY, agents)
}

pub fn save_settings(app: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    set_value(app, SETTINGS_KEY, settings)
}

fn get_value<T>(app: &tauri::AppHandle, key: &str) -> Result<Option<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    let store = app.store(STORE_PATH).map_err(|error| error.to_string())?;
    store
        .get(key)
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| error.to_string())
}

fn set_value<T>(app: &tauri::AppHandle, key: &str, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let store = app.store(STORE_PATH).map_err(|error| error.to_string())?;
    let value = serde_json::to_value(value).map_err(|error| error.to_string())?;
    store.set(key.to_string(), value);
    store.save().map_err(|error| error.to_string())
}
