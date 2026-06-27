use std::sync::mpsc::Sender;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use crate::llm::provider::{LlmProvider, ProviderFuture};
use crate::llm::types::{LlmChunk, LlmMessage, LlmTool};

#[derive(Clone, Debug)]
pub struct OllamaProvider {
    pub model: String,
    pub base_url: String,
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Deserialize, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaMessage>,
    response: Option<String>,
    error: Option<String>,
    done: Option<bool>,
}

#[derive(Serialize)]
struct OllamaEmbeddingRequest {
    model: String,
    prompt: String,
}

#[derive(Deserialize)]
struct OllamaEmbeddingResponse {
    embedding: Option<Vec<f32>>,
    error: Option<String>,
}

impl LlmProvider for OllamaProvider {
    fn stream_chat<'a>(
        &'a self,
        messages: Vec<LlmMessage>,
        _tools: Vec<LlmTool>,
        sender: Sender<LlmChunk>,
    ) -> ProviderFuture<'a, ()> {
        Box::pin(async move {
            let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
            let request = OllamaChatRequest {
                model: self.model.clone(),
                messages: messages
                    .into_iter()
                    .map(|message| OllamaMessage {
                        role: message.role,
                        content: message.content,
                    })
                    .collect(),
                stream: true,
            };

            let response = reqwest::Client::new()
                .post(url)
                .json(&request)
                .send()
                .await
                .map_err(|error| error.to_string())?;

            if !response.status().is_success() {
                return Err(response.text().await.map_err(|error| error.to_string())?);
            }

            let mut stream = response.bytes_stream();
            let mut pending = String::new();

            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|error| error.to_string())?;
                pending.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(newline_index) = pending.find('\n') {
                    let line = pending[..newline_index].trim().to_string();
                    pending = pending[newline_index + 1..].to_string();
                    if line.is_empty() {
                        continue;
                    }

                    let payload = serde_json::from_str::<OllamaChatResponse>(&line)
                        .map_err(|error| error.to_string())?;
                    if let Some(error) = payload.error {
                        return Err(error);
                    }

                    if let Some(content) = payload
                        .message
                        .map(|message| message.content)
                        .or(payload.response)
                        .filter(|content| !content.is_empty())
                    {
                        sender
                            .send(LlmChunk::Text { content })
                            .map_err(|error| error.to_string())?;
                    }

                    if payload.done.unwrap_or(false) {
                        sender
                            .send(LlmChunk::Done)
                            .map_err(|error| error.to_string())?;
                        return Ok(());
                    }
                }
            }

            sender
                .send(LlmChunk::Done)
                .map_err(|error| error.to_string())?;
            Ok(())
        })
    }

    fn embed<'a>(&'a self, text: String) -> ProviderFuture<'a, Vec<f32>> {
        Box::pin(async move {
            let url = format!("{}/api/embeddings", self.base_url.trim_end_matches('/'));
            let response = reqwest::Client::new()
                .post(url)
                .json(&OllamaEmbeddingRequest {
                    model: self.model.clone(),
                    prompt: text,
                })
                .send()
                .await
                .map_err(|error| error.to_string())?;

            if !response.status().is_success() {
                return Err(response.text().await.map_err(|error| error.to_string())?);
            }

            let payload = response
                .json::<OllamaEmbeddingResponse>()
                .await
                .map_err(|error| error.to_string())?;

            if let Some(error) = payload.error {
                return Err(error);
            }

            Ok(payload.embedding.unwrap_or_default())
        })
    }

    fn test_connection<'a>(&'a self) -> ProviderFuture<'a, ()> {
        Box::pin(async move {
            let url = format!("{}/api/tags", self.base_url.trim_end_matches('/'));
            let response = reqwest::Client::new()
                .get(url)
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
