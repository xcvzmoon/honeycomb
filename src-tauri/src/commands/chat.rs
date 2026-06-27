use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use tauri::{Emitter, State};

use crate::app_store;
use crate::commands::tools::{enforce_tool_permission, execute_builtin_tool};
use crate::db;
use crate::llm::registry;
use crate::llm::types::{LlmChunk, LlmMessage, LlmTool};
use crate::memory::rag::{self, RagIngestJob};
use crate::secure_store;
use crate::state::{AppState, ChatMessage, ProviderConfig, uuid_v7};
use crate::streaming::ChatEvent;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub agent_id: String,
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
    pub tools_enabled: bool,
    pub provider_config: Option<ProviderConfig>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateChatSessionRequest {
    pub agent_id: String,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatSessionRecord {
    pub id: String,
    pub agent_id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageRecord {
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    pub role: String,
    pub content: String,
    pub metadata: Option<Value>,
    pub created_at: String,
}

#[tauri::command]
pub async fn list_chat_sessions(
    agent_id: Option<String>,
    app: tauri::AppHandle,
) -> Result<Vec<ChatSessionRecord>, String> {
    let pool = db_pool(&app).await?;
    let rows = if let Some(agent_id) = agent_id {
        sqlx::query(
            "SELECT id, agent_id, title, created_at, updated_at, archived_at
             FROM chat_sessions
             WHERE agent_id = $1 AND archived_at IS NULL
             ORDER BY updated_at DESC",
        )
        .bind(agent_id)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query(
            "SELECT id, agent_id, title, created_at, updated_at, archived_at
             FROM chat_sessions
             WHERE archived_at IS NULL
             ORDER BY updated_at DESC",
        )
        .fetch_all(&pool)
        .await
    }
    .map_err(|error| error.to_string())?;

    rows.into_iter().map(row_to_chat_session).collect()
}

#[tauri::command]
pub async fn create_chat_session(
    request: CreateChatSessionRequest,
    app: tauri::AppHandle,
) -> Result<ChatSessionRecord, String> {
    let pool = db_pool(&app).await?;
    let id = uuid_v7();
    let title = request.title.unwrap_or_else(|| "New chat".to_string());

    sqlx::query(
        "INSERT INTO chat_sessions (id, agent_id, title)
         VALUES ($1, $2, $3)",
    )
    .bind(&id)
    .bind(&request.agent_id)
    .bind(&title)
    .execute(&pool)
    .await
    .map_err(|error| error.to_string())?;

    get_chat_session(id, app).await
}

#[tauri::command]
pub async fn get_chat_messages(
    session_id: String,
    app: tauri::AppHandle,
) -> Result<Vec<ChatMessageRecord>, String> {
    let pool = db_pool(&app).await?;
    let rows = sqlx::query(
        "SELECT id, session_id, agent_id, role, content, metadata, created_at
         FROM chat_messages
         WHERE session_id = $1
         ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(&pool)
    .await
    .map_err(|error| error.to_string())?;

    rows.into_iter().map(row_to_chat_message).collect()
}

#[tauri::command]
pub async fn rename_chat_session(
    session_id: String,
    title: String,
    app: tauri::AppHandle,
) -> Result<ChatSessionRecord, String> {
    let pool = db_pool(&app).await?;
    sqlx::query(
        "UPDATE chat_sessions
         SET title = $1, updated_at = CURRENT_TIMESTAMP
         WHERE id = $2",
    )
    .bind(title.trim())
    .bind(&session_id)
    .execute(&pool)
    .await
    .map_err(|error| error.to_string())?;

    get_chat_session(session_id, app).await
}

#[tauri::command]
pub async fn delete_chat_session(session_id: String, app: tauri::AppHandle) -> Result<(), String> {
    let pool = db_pool(&app).await?;
    sqlx::query("DELETE FROM chat_sessions WHERE id = $1")
        .bind(session_id)
        .execute(&pool)
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn send_message(
    request: SendMessageRequest,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let started_at = Instant::now();
    let provider_config = resolve_provider_config(&request, &app, &state)?;
    let embed_config = resolve_embed_config(&app, &state)?;
    let pool = db_pool(&app).await?;
    ensure_chat_session(
        &pool,
        &request.session_id,
        &request.agent_id,
        &request.messages,
    )
    .await?;

    let latest_user_message = request
        .messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .cloned();

    if let Some(user_message) = latest_user_message.as_ref() {
        let user_message_id =
            persist_chat_message(&pool, &request.session_id, &request.agent_id, user_message)
                .await?;
        if let Some(embed_config) = embed_config.clone() {
            rag::enqueue_ingest(
                &pool,
                RagIngestJob {
                    agent_id: request.agent_id.clone(),
                    session_id: Some(request.session_id.clone()),
                    source_type: "chat_message".to_string(),
                    source_id: user_message_id,
                    role: Some(user_message.role.clone()),
                    content: user_message.content.clone(),
                    embed_config,
                },
            )
            .await?;
        }
    }

    let memory_context = if let Some(user_message) = latest_user_message.as_ref() {
        rag::retrieve_context(
            &pool,
            &request.agent_id,
            &user_message.content,
            embed_config.clone(),
        )
        .await?
    } else {
        String::new()
    };

    if !memory_context.is_empty() {
        app.emit(
            "chat:event",
            ChatEvent::MemoryInjected {
                preview: memory_context.chars().take(100).collect(),
            },
        )
        .map_err(|error| error.to_string())?;
    }

    let provider = registry::create_provider(provider_config)?;
    let cancel_flag = Arc::new(AtomicBool::new(false));
    state
        .active_streams
        .lock()
        .map_err(|error| error.to_string())?
        .insert(request.session_id.clone(), cancel_flag.clone());
    let mut messages = request
        .messages
        .iter()
        .map(|message| LlmMessage {
            role: message.role.clone(),
            content: message.content.clone(),
        })
        .collect::<Vec<_>>();

    if !memory_context.is_empty() {
        messages.insert(
            0,
            LlmMessage {
                role: "system".to_string(),
                content: memory_context,
            },
        );
    }
    let tools = if request.tools_enabled {
        enabled_llm_tools(&state, &request.agent_id)?
    } else {
        Vec::new()
    };
    let (sender, receiver) = mpsc::channel();
    let provider_task =
        tauri::async_runtime::spawn(
            async move { provider.stream_chat(messages, tools, sender).await },
        );

    let mut assistant_content = String::new();
    while let Ok(chunk) = receiver.recv() {
        if cancel_flag.load(Ordering::SeqCst) {
            break;
        }

        match chunk {
            LlmChunk::Text { content } => {
                assistant_content.push_str(&content);
                app.emit("chat:event", ChatEvent::Text { content })
            }
            LlmChunk::Reasoning { content } => {
                app.emit("chat:event", ChatEvent::Reasoning { content })
            }
            LlmChunk::ToolCall { name, arguments } => {
                let call_id = uuid_v7();
                app.emit(
                    "chat:event",
                    ChatEvent::ToolStart {
                        name: name.clone(),
                        call_id: call_id.clone(),
                    },
                )
                .map_err(|error| error.to_string())?;
                let result = if request.tools_enabled {
                    enforce_tool_permission(&state, &request.agent_id, &name)
                        .and_then(|()| execute_builtin_tool(&name, &arguments))
                        .unwrap_or_else(|message| serde_json::json!({ "kind": "error", "message": message }))
                } else {
                    serde_json::json!({ "kind": "error", "message": "tools are disabled for this run" })
                };
                app.emit(
                    "chat:event",
                    ChatEvent::ToolResult {
                        call_id,
                        name: name.clone(),
                        result: result.clone(),
                    },
                )
                .map_err(|error| error.to_string())?;
                match result.get("kind").and_then(serde_json::Value::as_str) {
                    Some("todoUpdate") => app.emit(
                        "chat:event",
                        ChatEvent::TodoUpdate {
                            markdown: result
                                .get("markdown")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                        },
                    ),
                    Some("complete") => app.emit(
                        "chat:event",
                        ChatEvent::Complete {
                            summary: result
                                .get("summary")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                        },
                    ),
                    Some("clarify") => app.emit(
                        "chat:event",
                        ChatEvent::Clarify {
                            question: result
                                .get("question")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                            options: result.get("options").and_then(|options| {
                                options.as_array().map(|values| {
                                    values
                                        .iter()
                                        .filter_map(|value| value.as_str().map(ToString::to_string))
                                        .collect::<Vec<_>>()
                                })
                            }),
                            allow_multiple: result
                                .get("allowMultiple")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false),
                        },
                    ),
                    _ => Ok(()),
                }
            }
            LlmChunk::Done => break,
        }
        .map_err(|error| error.to_string())?;
    }

    if cancel_flag.load(Ordering::SeqCst) {
        provider_task.abort();
        state
            .active_streams
            .lock()
            .map_err(|error| error.to_string())?
            .remove(&request.session_id);
        app.emit(
            "chat:event",
            ChatEvent::Error {
                code: "cancelled".to_string(),
                message: "stream cancelled".to_string(),
            },
        )
        .map_err(|error| error.to_string())?;
        return Ok(());
    }

    let provider_result = provider_task.await.map_err(|error| error.to_string())?;

    state
        .active_streams
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&request.session_id);

    if let Err(error) = provider_result {
        app.emit(
            "chat:event",
            ChatEvent::Error {
                code: "provider_error".to_string(),
                message: error.clone(),
            },
        )
        .map_err(|error| error.to_string())?;
        return Err(error);
    }

    if !assistant_content.trim().is_empty() {
        let assistant_message = ChatMessage {
            role: "assistant".to_string(),
            content: assistant_content,
            metadata: None,
        };
        let assistant_message_id = persist_chat_message(
            &pool,
            &request.session_id,
            &request.agent_id,
            &assistant_message,
        )
        .await?;
        if let Some(embed_config) = embed_config.clone() {
            rag::enqueue_ingest(
                &pool,
                RagIngestJob {
                    agent_id: request.agent_id.clone(),
                    session_id: Some(request.session_id.clone()),
                    source_type: "chat_message".to_string(),
                    source_id: assistant_message_id,
                    role: Some(assistant_message.role),
                    content: assistant_message.content,
                    embed_config,
                },
            )
            .await?;
        }
    }

    trigger_embedding_worker(app.clone());

    app.emit(
        "chat:event",
        ChatEvent::Done {
            session_id: request.session_id,
            input_tokens: 0,
            output_tokens: 0,
            duration_ms: started_at.elapsed().as_millis() as u64,
        },
    )
    .map_err(|error| error.to_string())?;

    Ok(())
}

fn resolve_provider_config(
    request: &SendMessageRequest,
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
) -> Result<ProviderConfig, String> {
    if let Some(config) = request.provider_config.clone() {
        return Ok(secure_store::resolve_provider_api_key("chat", config));
    }

    let in_memory_config = state
        .provider_config
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    if let Some(config) = in_memory_config {
        return Ok(secure_store::resolve_provider_api_key("chat", config));
    }

    if let Some(config) = app_store::load(app)?.provider_config {
        return Ok(secure_store::resolve_provider_api_key("chat", config));
    }

    Err("No chat provider configured. Open Settings and save a chat provider first.".to_string())
}

fn resolve_embed_config(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
) -> Result<Option<ProviderConfig>, String> {
    let in_memory_config = state
        .embed_config
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    if let Some(config) = in_memory_config {
        return Ok(Some(secure_store::resolve_provider_api_key(
            "embed", config,
        )));
    }

    Ok(app_store::load(app)?
        .embed_config
        .map(|config| secure_store::resolve_provider_api_key("embed", config)))
}

async fn db_pool(app: &tauri::AppHandle) -> Result<SqlitePool, String> {
    db::pool(app).await
}

fn trigger_embedding_worker(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Ok(pool) = db_pool(&app).await else {
            return;
        };
        let _ = rag::process_pending_jobs(&pool, &app, 16).await;
    });
}

async fn get_chat_session(
    session_id: String,
    app: tauri::AppHandle,
) -> Result<ChatSessionRecord, String> {
    let pool = db_pool(&app).await?;
    let row = sqlx::query(
        "SELECT id, agent_id, title, created_at, updated_at, archived_at
         FROM chat_sessions
         WHERE id = $1",
    )
    .bind(&session_id)
    .fetch_optional(&pool)
    .await
    .map_err(|error| error.to_string())?
    .ok_or_else(|| format!("chat session not found: {session_id}"))?;

    row_to_chat_session(row)
}

async fn ensure_chat_session(
    pool: &SqlitePool,
    session_id: &str,
    agent_id: &str,
    messages: &[ChatMessage],
) -> Result<(), String> {
    let exists = sqlx::query("SELECT 1 FROM chat_sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| error.to_string())?
        .is_some();

    if exists {
        return Ok(());
    }

    let title = messages
        .iter()
        .find(|message| message.role == "user")
        .map(|message| title_from_content(&message.content))
        .unwrap_or_else(|| "New chat".to_string());

    sqlx::query(
        "INSERT INTO chat_sessions (id, agent_id, title)
         VALUES ($1, $2, $3)",
    )
    .bind(session_id)
    .bind(agent_id)
    .bind(title)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    Ok(())
}

async fn persist_chat_message(
    pool: &SqlitePool,
    session_id: &str,
    agent_id: &str,
    message: &ChatMessage,
) -> Result<String, String> {
    let metadata = message
        .metadata
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| error.to_string())?;
    let message_id = uuid_v7();

    sqlx::query(
        "INSERT INTO chat_messages (id, session_id, agent_id, role, content, metadata)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&message_id)
    .bind(session_id)
    .bind(agent_id)
    .bind(&message.role)
    .bind(&message.content)
    .bind(metadata)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    sqlx::query(
        "INSERT INTO transcript (id, agent_id, session_id, role, content)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(uuid_v7())
    .bind(agent_id)
    .bind(session_id)
    .bind(&message.role)
    .bind(&message.content)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    Ok(message_id)
}

fn row_to_chat_session(row: sqlx::sqlite::SqliteRow) -> Result<ChatSessionRecord, String> {
    Ok(ChatSessionRecord {
        id: row.try_get("id").map_err(|error| error.to_string())?,
        agent_id: row.try_get("agent_id").map_err(|error| error.to_string())?,
        title: row.try_get("title").map_err(|error| error.to_string())?,
        created_at: row
            .try_get("created_at")
            .map_err(|error| error.to_string())?,
        updated_at: row
            .try_get("updated_at")
            .map_err(|error| error.to_string())?,
        archived_at: row
            .try_get("archived_at")
            .map_err(|error| error.to_string())?,
    })
}

fn row_to_chat_message(row: sqlx::sqlite::SqliteRow) -> Result<ChatMessageRecord, String> {
    let metadata = row
        .try_get::<Option<String>, _>("metadata")
        .map_err(|error| error.to_string())?
        .map(|metadata| serde_json::from_str(&metadata))
        .transpose()
        .map_err(|error| error.to_string())?;

    Ok(ChatMessageRecord {
        id: row.try_get("id").map_err(|error| error.to_string())?,
        session_id: row
            .try_get("session_id")
            .map_err(|error| error.to_string())?,
        agent_id: row.try_get("agent_id").map_err(|error| error.to_string())?,
        role: row.try_get("role").map_err(|error| error.to_string())?,
        content: row.try_get("content").map_err(|error| error.to_string())?,
        metadata,
        created_at: row
            .try_get("created_at")
            .map_err(|error| error.to_string())?,
    })
}

fn enabled_llm_tools(state: &State<'_, AppState>, agent_id: &str) -> Result<Vec<LlmTool>, String> {
    let agents = state.agents.lock().map_err(|error| error.to_string())?;
    let Some(agent) = agents.get(agent_id) else {
        return Ok(Vec::new());
    };
    let mut tools = Vec::new();
    for name in &agent.tool_permissions {
        match name.as_str() {
            "todo" => tools.push(LlmTool {
                name: "todo".to_string(),
                description: "Replace the active task checklist markdown.".to_string(),
                schema: serde_json::json!({ "type": "object", "properties": { "markdown": { "type": "string" } }, "required": ["markdown"] }),
            }),
            "complete" => tools.push(LlmTool {
                name: "complete".to_string(),
                description: "Complete the current task with a useful summary.".to_string(),
                schema: serde_json::json!({ "type": "object", "properties": { "summary": { "type": "string" } }, "required": ["summary"] }),
            }),
            "clarify" => tools.push(LlmTool {
                name: "clarify".to_string(),
                description: "Ask the user a clarifying question.".to_string(),
                schema: serde_json::json!({ "type": "object", "properties": { "question": { "type": "string" }, "options": { "type": "array", "items": { "type": "string" } }, "allowMultiple": { "type": "boolean" } }, "required": ["question"] }),
            }),
            "echo" => tools.push(LlmTool {
                name: "echo".to_string(),
                description: "Return provided JSON arguments.".to_string(),
                schema: serde_json::json!({ "type": "object" }),
            }),
            _ => {}
        }
    }
    Ok(tools)
}

fn title_from_content(content: &str) -> String {
    let title = content.trim().replace('\n', " ");
    if title.chars().count() > 60 {
        format!("{}…", title.chars().take(60).collect::<String>())
    } else if title.is_empty() {
        "New chat".to_string()
    } else {
        title
    }
}

#[tauri::command]
pub async fn cancel_stream(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let active_streams = state
        .active_streams
        .lock()
        .map_err(|error| error.to_string())?;
    if let Some(cancel_flag) = active_streams.get(&session_id) {
        cancel_flag.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub async fn retry_last(_session_id: String) -> Result<(), String> {
    Ok(())
}
