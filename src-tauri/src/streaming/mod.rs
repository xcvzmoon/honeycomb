pub mod mapper;
pub mod sentinel;

use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum ChatEvent {
    Text {
        content: String,
    },
    Reasoning {
        content: String,
    },
    ToolStart {
        name: String,
        call_id: String,
    },
    ToolArgs {
        call_id: String,
        fragment: String,
    },
    ToolResult {
        call_id: String,
        name: String,
        result: Value,
    },
    MemoryInjected {
        preview: String,
    },
    Clarify {
        question: String,
        options: Option<Vec<String>>,
        allow_multiple: bool,
    },
    TodoUpdate {
        markdown: String,
    },
    Complete {
        summary: String,
    },
    Done {
        session_id: String,
        input_tokens: u32,
        output_tokens: u32,
        duration_ms: u64,
    },
    Error {
        code: String,
        message: String,
    },
}
