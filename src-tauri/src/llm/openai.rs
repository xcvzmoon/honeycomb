use std::collections::HashMap;
use std::sync::mpsc::Sender;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::llm::provider::{LlmProvider, ProviderFuture};
use crate::llm::types::{LlmChunk, LlmMessage, LlmTool};

#[derive(Clone, Debug)]
pub struct OpenAiProvider {
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: String,
}

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<LlmMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
}

#[derive(Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    kind: String,
    function: OpenAiToolFunction,
}

#[derive(Serialize)]
struct OpenAiToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    error: Option<OpenAiError>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Deserialize)]
struct ChatChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatCompletionStreamResponse {
    choices: Vec<ChatStreamChoice>,
    error: Option<OpenAiError>,
}

#[derive(Deserialize)]
struct ChatStreamChoice {
    delta: ChatStreamDelta,
}

#[derive(Deserialize)]
struct ChatStreamDelta {
    content: Option<String>,
    reasoning: Option<String>,
    tool_calls: Option<Vec<ChatStreamToolCall>>,
}

#[derive(Deserialize)]
struct ChatStreamToolCall {
    index: usize,
    function: Option<ChatStreamToolFunction>,
}

#[derive(Deserialize)]
struct ChatStreamToolFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiError {
    message: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    error: Option<OpenAiError>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

impl LlmProvider for OpenAiProvider {
    fn stream_chat<'a>(
        &'a self,
        messages: Vec<LlmMessage>,
        tools: Vec<LlmTool>,
        sender: Sender<LlmChunk>,
    ) -> ProviderFuture<'a, ()> {
        Box::pin(async move {
            let response = self
                .authorize(reqwest::Client::new().post(format!(
                    "{}/chat/completions",
                    self.base_url.trim_end_matches('/')
                )))
                .json(&ChatCompletionRequest {
                    model: self.model.clone(),
                    messages,
                    stream: true,
                    tool_choice: (!tools.is_empty()).then(|| "auto".to_string()),
                    tools: tools
                        .into_iter()
                        .map(|tool| OpenAiTool {
                            kind: "function".to_string(),
                            function: OpenAiToolFunction {
                                name: tool.name,
                                description: tool.description,
                                parameters: tool.schema,
                            },
                        })
                        .collect(),
                })
                .send()
                .await
                .map_err(|error| error.to_string())?;

            if !response.status().is_success() {
                return Err(response.text().await.map_err(|error| error.to_string())?);
            }

            let mut stream = response.bytes_stream();
            let mut pending = String::new();
            let mut tool_calls: HashMap<usize, (String, String)> = HashMap::new();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|error| error.to_string())?;
                pending.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(newline_index) = pending.find('\n') {
                    let line = pending[..newline_index].trim().to_string();
                    pending = pending[newline_index + 1..].to_string();
                    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                        continue;
                    };
                    if data == "[DONE]" {
                        flush_tool_calls(&sender, &mut tool_calls)?;
                        sender
                            .send(LlmChunk::Done)
                            .map_err(|error| error.to_string())?;
                        return Ok(());
                    }

                    let payload = serde_json::from_str::<ChatCompletionStreamResponse>(data)
                        .map_err(|error| error.to_string())?;
                    if let Some(error) = payload.error {
                        return Err(error.message);
                    }
                    for choice in payload.choices {
                        if let Some(content) =
                            choice.delta.content.filter(|content| !content.is_empty())
                        {
                            sender
                                .send(LlmChunk::Text { content })
                                .map_err(|error| error.to_string())?;
                        }
                        if let Some(content) =
                            choice.delta.reasoning.filter(|content| !content.is_empty())
                        {
                            sender
                                .send(LlmChunk::Reasoning { content })
                                .map_err(|error| error.to_string())?;
                        }
                        for tool_call in choice.delta.tool_calls.unwrap_or_default() {
                            let entry = tool_calls
                                .entry(tool_call.index)
                                .or_insert_with(|| (String::new(), String::new()));
                            if let Some(function) = tool_call.function {
                                if let Some(name) = function.name {
                                    entry.0.push_str(&name);
                                }
                                if let Some(arguments) = function.arguments {
                                    entry.1.push_str(&arguments);
                                }
                            }
                        }
                    }
                }
            }

            flush_tool_calls(&sender, &mut tool_calls)?;
            sender
                .send(LlmChunk::Done)
                .map_err(|error| error.to_string())?;
            Ok(())
        })
    }

    fn embed<'a>(&'a self, text: String) -> ProviderFuture<'a, Vec<f32>> {
        Box::pin(async move {
            let response = self
                .authorize(reqwest::Client::new().post(format!(
                    "{}/embeddings",
                    self.base_url.trim_end_matches('/')
                )))
                .json(&json!({
                    "model": self.model,
                    "input": text,
                }))
                .send()
                .await
                .map_err(|error| error.to_string())?;

            if !response.status().is_success() {
                return Err(response.text().await.map_err(|error| error.to_string())?);
            }

            let payload = response
                .json::<EmbeddingResponse>()
                .await
                .map_err(|error| error.to_string())?;

            if let Some(error) = payload.error {
                return Err(error.message);
            }

            Ok(payload
                .data
                .into_iter()
                .next()
                .map(|data| data.embedding)
                .unwrap_or_default())
        })
    }

    fn test_connection<'a>(&'a self) -> ProviderFuture<'a, ()> {
        Box::pin(async move {
            if self.base_url.trim().is_empty() {
                return Err("provider base URL cannot be empty".to_string());
            }

            let response = self
                .authorize(
                    reqwest::Client::new()
                        .get(format!("{}/models", self.base_url.trim_end_matches('/'))),
                )
                .send()
                .await
                .map_err(|error| error.to_string())?;

            if response.status().is_success() {
                Ok(())
            } else {
                Err(response.text().await.map_err(|error| error.to_string())?)
            }
        })
    }
}

fn flush_tool_calls(
    sender: &Sender<LlmChunk>,
    tool_calls: &mut HashMap<usize, (String, String)>,
) -> Result<(), String> {
    let calls = std::mem::take(tool_calls);
    for (_index, (name, arguments)) in calls {
        if name.is_empty() {
            continue;
        }
        let arguments =
            serde_json::from_str(&arguments).unwrap_or_else(|_| json!({ "raw": arguments }));
        sender
            .send(LlmChunk::ToolCall { name, arguments })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

impl OpenAiProvider {
    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(api_key) = self.api_key.as_ref().filter(|api_key| !api_key.is_empty()) {
            request.bearer_auth(api_key)
        } else {
            request
        }
    }
}
