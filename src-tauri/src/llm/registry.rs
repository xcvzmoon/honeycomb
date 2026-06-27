use crate::llm::anthropic::AnthropicProvider;
use crate::llm::ollama::OllamaProvider;
use crate::llm::openai::OpenAiProvider;
use crate::llm::provider::LlmProvider;
use crate::state::ProviderConfig;

pub fn create_provider(config: ProviderConfig) -> Result<Box<dyn LlmProvider>, String> {
    match config {
        ProviderConfig::OpenAi {
            api_key,
            model,
            base_url,
        } => Ok(Box::new(OpenAiProvider {
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        })),
        ProviderConfig::Anthropic { api_key, model } => {
            Ok(Box::new(AnthropicProvider { api_key, model }))
        }
        ProviderConfig::Ollama { model, base_url } => Ok(Box::new(OllamaProvider {
            model,
            base_url: base_url.ok_or_else(|| "Ollama base URL is required".to_string())?,
        })),
        ProviderConfig::OpenRouter { api_key, model } => Ok(Box::new(OpenAiProvider {
            api_key,
            model,
            base_url: "https://openrouter.ai/api/v1".to_string(),
        })),
        ProviderConfig::Custom {
            api_key,
            model,
            base_url,
        } => Ok(Box::new(OpenAiProvider {
            api_key,
            model,
            base_url,
        })),
    }
}
