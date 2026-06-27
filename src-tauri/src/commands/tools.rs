use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::State;

use crate::app_store;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub schema: Value,
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteToolRequest {
    pub name: String,
    pub arguments: Value,
    pub agent_id: Option<String>,
}

#[tauri::command]
pub async fn list_tools(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ToolDefinition>, String> {
    let allowed = state
        .agents
        .lock()
        .map_err(|error| error.to_string())?
        .get(&agent_id)
        .map(|agent| agent.tool_permissions.clone())
        .unwrap_or_default();
    Ok(tool_definitions(&allowed))
}

#[tauri::command]
pub async fn set_tool_enabled(
    agent_id: String,
    tool_name: String,
    enabled: bool,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut agents = state.agents.lock().map_err(|error| error.to_string())?;
    let agent = agents
        .get_mut(&agent_id)
        .ok_or_else(|| format!("agent not found: {agent_id}"))?;
    if enabled && !agent.tool_permissions.iter().any(|name| name == &tool_name) {
        agent.tool_permissions.push(tool_name);
    } else if !enabled {
        agent.tool_permissions.retain(|name| name != &tool_name);
    }
    app_store::save_agents(&app, &agents)
}

#[tauri::command]
pub async fn execute_tool(
    request: ExecuteToolRequest,
    state: State<'_, AppState>,
) -> Result<Value, String> {
    if let Some(agent_id) = &request.agent_id {
        enforce_tool_permission(&state, agent_id, &request.name)?;
    }
    execute_builtin_tool(&request.name, &request.arguments)
}

pub fn execute_builtin_tool(name: &str, arguments: &Value) -> Result<Value, String> {
    match name {
        "todo" => Ok(json!({
            "kind": "todoUpdate",
            "markdown": required_string(arguments, "markdown")?,
        })),
        "complete" => Ok(json!({
            "kind": "complete",
            "summary": required_string(arguments, "summary")?,
        })),
        "clarify" => Ok(json!({
            "kind": "clarify",
            "question": required_string(arguments, "question")?,
            "options": arguments.get("options").cloned().unwrap_or(Value::Null),
            "allowMultiple": arguments.get("allowMultiple").and_then(Value::as_bool).unwrap_or(false),
        })),
        "echo" => Ok(json!({ "kind": "success", "result": arguments })),
        _ => Err(format!("unknown tool: {name}")),
    }
}

pub fn enforce_tool_permission(
    state: &State<'_, AppState>,
    agent_id: &str,
    tool_name: &str,
) -> Result<(), String> {
    let agents = state.agents.lock().map_err(|error| error.to_string())?;
    let Some(agent) = agents.get(agent_id) else {
        return Err(format!("agent not found: {agent_id}"));
    };
    if agent.tool_permissions.iter().any(|name| name == tool_name) {
        Ok(())
    } else {
        Err(format!("tool '{tool_name}' is not enabled for this agent"))
    }
}

fn tool_definitions(allowed: &[String]) -> Vec<ToolDefinition> {
    [
        ToolDefinition {
            name: "todo".to_string(),
            description: "Replace the active task checklist.".to_string(),
            schema: json!({ "type": "object", "properties": { "markdown": { "type": "string" } }, "required": ["markdown"] }),
            enabled: allowed.iter().any(|name| name == "todo"),
        },
        ToolDefinition {
            name: "complete".to_string(),
            description: "Complete the current task with a useful summary.".to_string(),
            schema: json!({ "type": "object", "properties": { "summary": { "type": "string" } }, "required": ["summary"] }),
            enabled: allowed.iter().any(|name| name == "complete"),
        },
        ToolDefinition {
            name: "clarify".to_string(),
            description: "Ask the user a clarifying question.".to_string(),
            schema: json!({ "type": "object", "properties": { "question": { "type": "string" }, "options": { "type": "array", "items": { "type": "string" } }, "allowMultiple": { "type": "boolean" } }, "required": ["question"] }),
            enabled: allowed.iter().any(|name| name == "clarify"),
        },
        ToolDefinition {
            name: "echo".to_string(),
            description: "Return the provided JSON arguments.".to_string(),
            schema: json!({ "type": "object" }),
            enabled: allowed.iter().any(|name| name == "echo"),
        },
    ].to_vec()
}

fn required_string(arguments: &Value, key: &str) -> Result<String, String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| format!("missing required string argument: {key}"))
}
