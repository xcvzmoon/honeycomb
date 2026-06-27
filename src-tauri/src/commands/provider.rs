use tauri::State;

use crate::app_store;
use crate::llm::registry;
use crate::secure_store;
use crate::state::{AppState, ProviderConfig};

#[tauri::command]
pub async fn save_provider_config(
    config: ProviderConfig,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let config = secure_store::store_provider_api_key("chat", &config)?;
    let mut provider_config = state
        .provider_config
        .lock()
        .map_err(|error| error.to_string())?;
    *provider_config = Some(config.clone());
    app_store::save_provider_config(&app, &config)
}

#[tauri::command]
pub async fn get_provider_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<ProviderConfig>, String> {
    let provider_config = state
        .provider_config
        .lock()
        .map_err(|error| error.to_string())?;
    if let Some(config) = provider_config.clone() {
        return Ok(Some(redact_provider_config(
            secure_store::resolve_provider_api_key("chat", config),
        )));
    }

    let stored_config = app_store::load(&app)?.provider_config;
    if let Some(config) = stored_config.clone() {
        drop(provider_config);
        let mut provider_config = state
            .provider_config
            .lock()
            .map_err(|error| error.to_string())?;
        *provider_config = Some(config.clone());
    }
    Ok(stored_config.map(|config| {
        redact_provider_config(secure_store::resolve_provider_api_key("chat", config))
    }))
}

#[tauri::command]
pub async fn save_embed_config(
    config: ProviderConfig,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let config = secure_store::store_provider_api_key("embed", &config)?;
    let mut embed_config = state
        .embed_config
        .lock()
        .map_err(|error| error.to_string())?;
    *embed_config = Some(config.clone());
    app_store::save_embed_config(&app, &config)
}

#[tauri::command]
pub async fn get_embed_config(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<ProviderConfig>, String> {
    let embed_config = state
        .embed_config
        .lock()
        .map_err(|error| error.to_string())?;
    if let Some(config) = embed_config.clone() {
        return Ok(Some(redact_provider_config(
            secure_store::resolve_provider_api_key("embed", config),
        )));
    }

    let stored_config = app_store::load(&app)?.embed_config;
    if let Some(config) = stored_config.clone() {
        drop(embed_config);
        let mut embed_config = state
            .embed_config
            .lock()
            .map_err(|error| error.to_string())?;
        *embed_config = Some(config.clone());
    }
    Ok(stored_config.map(|config| {
        redact_provider_config(secure_store::resolve_provider_api_key("embed", config))
    }))
}

#[tauri::command]
pub async fn test_provider(config: ProviderConfig) -> Result<(), String> {
    let provider =
        registry::create_provider(secure_store::resolve_provider_api_key("chat", config))?;
    provider.test_connection().await
}

#[tauri::command]
pub async fn test_chat_provider(config: ProviderConfig) -> Result<(), String> {
    use std::sync::mpsc;

    use crate::llm::types::LlmMessage;

    let provider =
        registry::create_provider(secure_store::resolve_provider_api_key("chat", config))?;
    let (sender, receiver) = mpsc::channel();
    provider
        .stream_chat(
            vec![LlmMessage {
                role: "user".to_string(),
                content: "Reply with only: ok".to_string(),
            }],
            Vec::new(),
            sender,
        )
        .await?;

    if receiver.try_iter().next().is_some() {
        Ok(())
    } else {
        Err("chat provider returned no content".to_string())
    }
}

#[tauri::command]
pub async fn test_embed_provider(config: ProviderConfig) -> Result<(), String> {
    let provider =
        registry::create_provider(secure_store::resolve_provider_api_key("embed", config))?;
    let embedding = provider.embed("hello".to_string()).await?;
    if embedding.is_empty() {
        Err("embedding provider returned an empty vector".to_string())
    } else {
        Ok(())
    }
}

fn redact_provider_config(config: ProviderConfig) -> ProviderConfig {
    match config {
        ProviderConfig::OpenAi {
            api_key,
            model,
            base_url,
        } => ProviderConfig::OpenAi {
            api_key: api_key.map(redact_key),
            model,
            base_url,
        },
        ProviderConfig::Anthropic { api_key, model } => ProviderConfig::Anthropic {
            api_key: api_key.map(redact_key),
            model,
        },
        ProviderConfig::OpenRouter { api_key, model } => ProviderConfig::OpenRouter {
            api_key: api_key.map(redact_key),
            model,
        },
        ProviderConfig::Custom {
            api_key,
            model,
            base_url,
        } => ProviderConfig::Custom {
            api_key: api_key.map(redact_key),
            model,
            base_url,
        },
        ProviderConfig::Ollama { model, base_url } => ProviderConfig::Ollama { model, base_url },
    }
}

fn redact_key(key: String) -> String {
    if key.is_empty() {
        return key;
    }
    let prefix: String = key.chars().take(3).collect();
    format!("{prefix}-***")
}
