use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default)]
pub struct AppState {
    pub agents: Mutex<HashMap<String, AgentDefinition>>,
    pub provider_config: Mutex<Option<ProviderConfig>>,
    pub embed_config: Mutex<Option<ProviderConfig>>,
    pub settings: Mutex<AppSettings>,
    pub active_streams: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefinition {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub provider_config_ref: Option<String>,
    pub tool_permissions: Vec<String>,
    pub memory_enabled: bool,
    pub pinned_fact_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentRequest {
    pub name: String,
    pub description: Option<String>,
    pub provider_config_ref: Option<String>,
    #[serde(default)]
    pub tool_permissions: Vec<String>,
    #[serde(default = "default_true")]
    pub memory_enabled: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAgentRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub provider_config_ref: Option<String>,
    pub tool_permissions: Option<Vec<String>>,
    pub memory_enabled: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ProviderConfig {
    OpenAi {
        #[serde(rename = "apiKey", alias = "api_key")]
        api_key: Option<String>,
        model: String,
        #[serde(rename = "baseUrl", alias = "base_url")]
        base_url: Option<String>,
    },
    Anthropic {
        #[serde(rename = "apiKey", alias = "api_key")]
        api_key: Option<String>,
        model: String,
    },
    Ollama {
        model: String,
        #[serde(rename = "baseUrl", alias = "base_url")]
        base_url: Option<String>,
    },
    OpenRouter {
        #[serde(rename = "apiKey", alias = "api_key")]
        api_key: Option<String>,
        model: String,
    },
    Custom {
        #[serde(rename = "apiKey", alias = "api_key")]
        api_key: Option<String>,
        model: String,
        #[serde(rename = "baseUrl", alias = "base_url")]
        base_url: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub theme: String,
    pub default_agent_id: Option<String>,
    pub working_folder_bookmark: Option<String>,
    pub memory_budget_tokens: u32,
    pub consolidation_interval_hours: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            default_agent_id: None,
            working_folder_bookmark: None,
            memory_budget_tokens: 800,
            consolidation_interval_hours: 24,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

fn default_true() -> bool {
    true
}

pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::now_v7())
}

pub fn uuid_v7() -> String {
    uuid::Uuid::now_v7().to_string()
}
