use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ToolEnvelope {
    Success { result: Value },
    Error { message: String },
    NotFound { path: String },
    PermissionDenied { reason: String },
    Structured { payload: Value },
}

pub struct ToolRegistry;
