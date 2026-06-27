use keyring::Entry;

use crate::state::ProviderConfig;

const SERVICE: &str = "honeycomb";
const DATABASE_KEY_ACCOUNT: &str = "database:sqlcipher-key";

pub fn database_key() -> Result<String, String> {
    let entry = Entry::new(SERVICE, DATABASE_KEY_ACCOUNT).map_err(|error| error.to_string())?;
    match entry.get_password() {
        Ok(key) if !key.is_empty() => Ok(key),
        Ok(_) | Err(_) => {
            let key = format!("hc-db-{}-{}", uuid::Uuid::now_v7(), uuid::Uuid::now_v7());
            entry
                .set_password(&key)
                .map_err(|error| error.to_string())?;
            Ok(key)
        }
    }
}

pub fn store_provider_api_key(
    slot: &str,
    config: &ProviderConfig,
) -> Result<ProviderConfig, String> {
    let mut config = config.clone();
    let Some(api_key) = take_api_key(&mut config) else {
        return Ok(config);
    };

    if api_key.trim().is_empty() || is_redacted_api_key(&api_key) {
        return Ok(config);
    }

    entry(slot)?
        .set_password(&api_key)
        .map_err(|error| error.to_string())?;
    Ok(config)
}

pub fn resolve_provider_api_key(slot: &str, config: ProviderConfig) -> ProviderConfig {
    let existing_key = api_key(&config);
    if existing_key
        .as_deref()
        .is_some_and(|key| !key.trim().is_empty() && !is_redacted_api_key(key))
    {
        return config;
    }

    let Ok(api_key) =
        entry(slot).and_then(|entry| entry.get_password().map_err(|error| error.to_string()))
    else {
        return config;
    };

    with_api_key(config, Some(api_key))
}

fn entry(slot: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, &format!("{slot}:api-key")).map_err(|error| error.to_string())
}

fn is_redacted_api_key(api_key: &str) -> bool {
    api_key.contains("***")
}

fn api_key(config: &ProviderConfig) -> Option<String> {
    match config {
        ProviderConfig::OpenAi { api_key, .. }
        | ProviderConfig::Anthropic { api_key, .. }
        | ProviderConfig::OpenRouter { api_key, .. }
        | ProviderConfig::Custom { api_key, .. } => api_key.clone(),
        ProviderConfig::Ollama { .. } => None,
    }
}

fn take_api_key(config: &mut ProviderConfig) -> Option<String> {
    match config {
        ProviderConfig::OpenAi { api_key, .. }
        | ProviderConfig::Anthropic { api_key, .. }
        | ProviderConfig::OpenRouter { api_key, .. }
        | ProviderConfig::Custom { api_key, .. } => api_key.take(),
        ProviderConfig::Ollama { .. } => None,
    }
}

fn with_api_key(config: ProviderConfig, api_key: Option<String>) -> ProviderConfig {
    match config {
        ProviderConfig::OpenAi {
            model, base_url, ..
        } => ProviderConfig::OpenAi {
            api_key,
            model,
            base_url,
        },
        ProviderConfig::Anthropic { model, .. } => ProviderConfig::Anthropic { api_key, model },
        ProviderConfig::OpenRouter { model, .. } => ProviderConfig::OpenRouter { api_key, model },
        ProviderConfig::Custom {
            model, base_url, ..
        } => ProviderConfig::Custom {
            api_key,
            model,
            base_url,
        },
        ProviderConfig::Ollama { model, base_url } => ProviderConfig::Ollama { model, base_url },
    }
}
