use std::path::{Path, PathBuf};

use tauri::Manager;
use tokenizers::Tokenizer;

use crate::state::ProviderConfig;

const TOKENIZER_ENV: &str = "HONEYCOMB_TOKENIZER_PATH";

#[derive(Clone, Debug)]
pub struct TokenizerStatus {
    pub path: Option<PathBuf>,
    pub loaded: bool,
    pub reason: Option<String>,
}

pub fn resolve_tokenizer_path(
    app: &tauri::AppHandle,
    embed_config: Option<&ProviderConfig>,
) -> Result<Option<PathBuf>, String> {
    if let Ok(path) = std::env::var(TOKENIZER_ENV) {
        let path = PathBuf::from(path);
        if path.exists() {
            return Ok(Some(path));
        }
    }

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;

    let mut candidates = Vec::new();
    if let Some(config) = embed_config {
        candidates.push(
            app_data_dir
                .join("tokenizers")
                .join(tokenizer_cache_key(config))
                .join("tokenizer.json"),
        );
    }
    candidates.push(
        app_data_dir
            .join("tokenizers")
            .join("default")
            .join("tokenizer.json"),
    );

    Ok(candidates.into_iter().find(|path| path.exists()))
}

pub fn load_tokenizer(path: Option<&Path>) -> TokenizerStatus {
    let Some(path) = path else {
        return TokenizerStatus {
            path: None,
            loaded: false,
            reason: Some("no cached tokenizer.json found".to_string()),
        };
    };

    match Tokenizer::from_file(path) {
        Ok(_) => TokenizerStatus {
            path: Some(path.to_path_buf()),
            loaded: true,
            reason: None,
        },
        Err(error) => TokenizerStatus {
            path: Some(path.to_path_buf()),
            loaded: false,
            reason: Some(error.to_string()),
        },
    }
}

pub fn tokenizer_from_path(path: Option<&Path>) -> Option<Tokenizer> {
    path.and_then(|path| Tokenizer::from_file(path).ok())
}

pub fn tokenizer_cache_key(config: &ProviderConfig) -> String {
    let (kind, model, base_url) = match config {
        ProviderConfig::OpenAi {
            model, base_url, ..
        } => (
            "openai",
            model.as_str(),
            base_url.as_deref().unwrap_or("default"),
        ),
        ProviderConfig::Anthropic { model, .. } => ("anthropic", model.as_str(), "default"),
        ProviderConfig::Ollama { model, base_url } => (
            "ollama",
            model.as_str(),
            base_url.as_deref().unwrap_or("default"),
        ),
        ProviderConfig::OpenRouter { model, .. } => ("openrouter", model.as_str(), "default"),
        ProviderConfig::Custom {
            model, base_url, ..
        } => ("custom", model.as_str(), base_url.as_str()),
    };

    sanitize_tokenizer_cache_key(&format!("{kind}-{base_url}-{model}"))
}

pub fn sanitize_tokenizer_cache_key(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_lowercase()
}
