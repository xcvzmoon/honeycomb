use std::sync::mpsc::Sender;

use crate::llm::provider::{LlmProvider, ProviderFuture};
use crate::llm::types::{LlmChunk, LlmMessage, LlmTool};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct AnthropicProvider {
    pub api_key: Option<String>,
    pub model: String,
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<LlmMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicErrorEnvelope {
    error: Option<AnthropicError>,
}

#[derive(Deserialize)]
struct AnthropicError {
    message: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum AnthropicStreamEvent {
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { delta: AnthropicDelta },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "error")]
    Error { error: AnthropicError },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum AnthropicDelta {
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(rename = "thinking_delta")]
    Thinking { thinking: String },
    #[serde(other)]
    Other,
}

impl LlmProvider for AnthropicProvider {
    fn stream_chat<'a>(
        &'a self,
        messages: Vec<LlmMessage>,
        _tools: Vec<LlmTool>,
        sender: Sender<LlmChunk>,
    ) -> ProviderFuture<'a, ()> {
        Box::pin(async move {
            let (system, messages) = split_system_messages(messages);
            let response = self
                .authorize(reqwest::Client::new().post("https://api.anthropic.com/v1/messages"))
                .json(&AnthropicRequest {
                    model: self.model.clone(),
                    max_tokens: 4096,
                    messages,
                    stream: true,
                    system,
                })
                .send()
                .await
                .map_err(|error| error.to_string())?;

            if !response.status().is_success() {
                let text = response.text().await.map_err(|error| error.to_string())?;
                if let Ok(payload) = serde_json::from_str::<AnthropicErrorEnvelope>(&text)
                    && let Some(error) = payload.error
                {
                    return Err(error.message);
                }
                return Err(text);
            }

            let mut stream = response.bytes_stream();
            let mut pending = String::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|error| error.to_string())?;
                pending.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(newline_index) = pending.find('\n') {
                    let line = pending[..newline_index].trim().to_string();
                    pending = pending[newline_index + 1..].to_string();
                    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                        continue;
                    };
                    if data.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<AnthropicStreamEvent>(data)
                        .map_err(|error| error.to_string())?
                    {
                        AnthropicStreamEvent::ContentBlockDelta { delta } => match delta {
                            AnthropicDelta::Text { text } if !text.is_empty() => sender
                                .send(LlmChunk::Text { content: text })
                                .map_err(|error| error.to_string())?,
                            AnthropicDelta::Thinking { thinking } if !thinking.is_empty() => sender
                                .send(LlmChunk::Reasoning { content: thinking })
                                .map_err(|error| error.to_string())?,
                            _ => {}
                        },
                        AnthropicStreamEvent::MessageStop => {
                            sender
                                .send(LlmChunk::Done)
                                .map_err(|error| error.to_string())?;
                            return Ok(());
                        }
                        AnthropicStreamEvent::Error { error } => return Err(error.message),
                        AnthropicStreamEvent::Other => {}
                    }
                }
            }

            sender
                .send(LlmChunk::Done)
                .map_err(|error| error.to_string())?;
            Ok(())
        })
    }

    fn embed<'a>(&'a self, _text: String) -> ProviderFuture<'a, Vec<f32>> {
        Box::pin(async move {
            Err(
                "Anthropic does not provide embeddings; configure a separate embedding provider"
                    .to_string(),
            )
        })
    }

    fn test_connection<'a>(&'a self) -> ProviderFuture<'a, ()> {
        Box::pin(async move {
            let (sender, receiver) = std::sync::mpsc::channel();
            self.stream_chat(
                vec![LlmMessage {
                    role: "user".to_string(),
                    content: "Reply with only: ok".to_string(),
                }],
                Vec::new(),
                sender,
            )
            .await?;
            if receiver
                .try_iter()
                .any(|chunk| matches!(chunk, LlmChunk::Text { .. }))
            {
                Ok(())
            } else {
                Err("Anthropic returned no content".to_string())
            }
        })
    }
}

impl AnthropicProvider {
    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let request = request
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json");
        if let Some(api_key) = self.api_key.as_ref().filter(|api_key| !api_key.is_empty()) {
            request.header("x-api-key", api_key)
        } else {
            request
        }
    }
}

fn split_system_messages(messages: Vec<LlmMessage>) -> (Option<String>, Vec<LlmMessage>) {
    let mut system = Vec::new();
    let mut chat = Vec::new();
    for message in messages {
        if message.role == "system" {
            system.push(message.content);
        } else {
            chat.push(message);
        }
    }
    (
        Some(system.join("\n\n")).filter(|value| !value.is_empty()),
        chat,
    )
}
