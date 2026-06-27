use tauri::State;

use crate::app_store;
use crate::state::{AgentDefinition, AppState, CreateAgentRequest, UpdateAgentRequest, new_id};

#[tauri::command]
pub async fn list_agents(state: State<'_, AppState>) -> Result<Vec<AgentDefinition>, String> {
    let agents = state.agents.lock().map_err(|error| error.to_string())?;
    let mut agents = agents.values().cloned().collect::<Vec<_>>();
    agents.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(agents)
}

#[tauri::command]
pub async fn create_agent(
    request: CreateAgentRequest,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AgentDefinition, String> {
    let agent = AgentDefinition {
        id: new_id("agent"),
        name: request.name,
        description: request.description,
        provider_config_ref: request.provider_config_ref,
        tool_permissions: request.tool_permissions,
        memory_enabled: request.memory_enabled,
        pinned_fact_count: 0,
    };

    let mut agents = state.agents.lock().map_err(|error| error.to_string())?;
    agents.insert(agent.id.clone(), agent.clone());
    app_store::save_agents(&app, &agents)?;
    Ok(agent)
}

#[tauri::command]
pub async fn update_agent(
    agent_id: String,
    update: UpdateAgentRequest,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<AgentDefinition, String> {
    let mut agents = state.agents.lock().map_err(|error| error.to_string())?;
    let agent = agents
        .get_mut(&agent_id)
        .ok_or_else(|| format!("agent not found: {agent_id}"))?;

    if let Some(name) = update.name {
        agent.name = name;
    }
    if let Some(description) = update.description {
        agent.description = Some(description);
    }
    if let Some(provider_config_ref) = update.provider_config_ref {
        agent.provider_config_ref = Some(provider_config_ref);
    }
    if let Some(tool_permissions) = update.tool_permissions {
        agent.tool_permissions = tool_permissions;
    }
    if let Some(memory_enabled) = update.memory_enabled {
        agent.memory_enabled = memory_enabled;
    }

    let updated_agent = agent.clone();
    app_store::save_agents(&app, &agents)?;
    Ok(updated_agent)
}

#[tauri::command]
pub async fn delete_agent(
    agent_id: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut agents = state.agents.lock().map_err(|error| error.to_string())?;
    agents
        .remove(&agent_id)
        .map(|_| ())
        .ok_or_else(|| format!("agent not found: {agent_id}"))?;
    app_store::save_agents(&app, &agents)
}

#[tauri::command]
pub async fn get_agent(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<AgentDefinition, String> {
    let agents = state.agents.lock().map_err(|error| error.to_string())?;
    agents
        .get(&agent_id)
        .cloned()
        .ok_or_else(|| format!("agent not found: {agent_id}"))
}
