use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc::Sender;

use crate::llm::types::{LlmChunk, LlmMessage, LlmTool};

pub type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

pub trait LlmProvider: Send + Sync {
    fn stream_chat<'a>(
        &'a self,
        messages: Vec<LlmMessage>,
        tools: Vec<LlmTool>,
        sender: Sender<LlmChunk>,
    ) -> ProviderFuture<'a, ()>;

    fn embed<'a>(&'a self, text: String) -> ProviderFuture<'a, Vec<f32>>;

    fn test_connection<'a>(&'a self) -> ProviderFuture<'a, ()>;
}
