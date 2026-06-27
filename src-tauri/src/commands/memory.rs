use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use tauri::{Manager, State};

use crate::app_store;
use crate::db;
use crate::memory::rag::{self, RagIngestJob, RetrievedChunk};
use crate::memory::tokenizer;
use crate::secure_store;
use crate::state::{AppState, Pagination, ProviderConfig, uuid_v7};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOverview {
    pub agent_id: String,
    pub identity_block: String,
    pub pinned_fact_count: u32,
    pub episode_count: u32,
    pub database_size_bytes: u64,
    pub last_consolidation_timestamp: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedFact {
    pub id: String,
    pub content: String,
    pub salience: f32,
    pub use_count: u32,
    pub last_used_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeSummary {
    pub id: String,
    pub summary: String,
    pub topics: Vec<String>,
    pub salience: f32,
    pub session_date: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizerStatusResponse {
    pub path: Option<String>,
    pub loaded: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTokenizerRequest {
    pub source_path: String,
    pub cache_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTokenizerRequest {
    pub url: String,
    pub cache_key: Option<String>,
}

#[tauri::command]
pub async fn get_memory_overview(
    agent_id: String,
    app: tauri::AppHandle,
) -> Result<MemoryOverview, String> {
    let pool = db_pool(&app).await?;
    let identity_block = sqlx::query("SELECT content FROM identity WHERE agent_id = $1")
        .bind(&agent_id)
        .fetch_optional(&pool)
        .await
        .map_err(|error| error.to_string())?
        .and_then(|row| row.try_get::<String, _>("content").ok())
        .unwrap_or_default();
    let pinned_fact_count = count_for_agent(&pool, "pinned_facts", &agent_id).await? as u32;
    let episode_count = count_for_agent(&pool, "episodes", &agent_id).await? as u32;
    let last_consolidation_timestamp =
        sqlx::query("SELECT last_consolidated_at FROM memory_config WHERE agent_id = $1")
            .bind(&agent_id)
            .fetch_optional(&pool)
            .await
            .map_err(|error| error.to_string())?
            .and_then(|row| {
                row.try_get::<Option<String>, _>("last_consolidated_at")
                    .ok()
            })
            .flatten();
    let database_size_bytes = std::fs::metadata(db_path(&app)?)
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    Ok(MemoryOverview {
        agent_id,
        identity_block,
        pinned_fact_count,
        episode_count,
        database_size_bytes,
        last_consolidation_timestamp,
    })
}

#[tauri::command]
pub async fn list_pinned_facts(
    agent_id: String,
    pagination: Option<Pagination>,
    app: tauri::AppHandle,
) -> Result<Vec<PinnedFact>, String> {
    let pool = db_pool(&app).await?;
    let limit = pagination
        .as_ref()
        .and_then(|page| page.limit)
        .unwrap_or(50)
        .min(200);
    let offset = pagination
        .as_ref()
        .and_then(|page| page.offset)
        .unwrap_or(0);
    let rows = sqlx::query(
        "SELECT id, content, salience, use_count, last_used_at
         FROM pinned_facts WHERE agent_id = $1
         ORDER BY salience DESC, updated_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(agent_id)
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(&pool)
    .await
    .map_err(|error| error.to_string())?;

    rows.into_iter()
        .map(|row| {
            Ok(PinnedFact {
                id: row.try_get("id").map_err(|error| error.to_string())?,
                content: row.try_get("content").map_err(|error| error.to_string())?,
                salience: row
                    .try_get::<f64, _>("salience")
                    .map_err(|error| error.to_string())? as f32,
                use_count: row
                    .try_get::<i64, _>("use_count")
                    .map_err(|error| error.to_string())? as u32,
                last_used_at: row
                    .try_get("last_used_at")
                    .map_err(|error| error.to_string())?,
            })
        })
        .collect()
}

#[tauri::command]
pub async fn list_episodes(
    agent_id: String,
    pagination: Option<Pagination>,
    app: tauri::AppHandle,
) -> Result<Vec<EpisodeSummary>, String> {
    let pool = db_pool(&app).await?;
    let limit = pagination
        .as_ref()
        .and_then(|page| page.limit)
        .unwrap_or(50)
        .min(200);
    let offset = pagination
        .as_ref()
        .and_then(|page| page.offset)
        .unwrap_or(0);
    let rows = sqlx::query(
        "SELECT id, summary, topics, salience, session_date
         FROM episodes WHERE agent_id = $1
         ORDER BY session_date DESC LIMIT $2 OFFSET $3",
    )
    .bind(agent_id)
    .bind(limit as i64)
    .bind(offset as i64)
    .fetch_all(&pool)
    .await
    .map_err(|error| error.to_string())?;

    rows.into_iter()
        .map(|row| {
            let topics = row
                .try_get::<String, _>("topics")
                .map_err(|error| error.to_string())?
                .split(',')
                .map(str::trim)
                .filter(|topic| !topic.is_empty())
                .map(ToString::to_string)
                .collect();
            Ok(EpisodeSummary {
                id: row.try_get("id").map_err(|error| error.to_string())?,
                summary: row.try_get("summary").map_err(|error| error.to_string())?,
                topics,
                salience: row
                    .try_get::<f64, _>("salience")
                    .map_err(|error| error.to_string())? as f32,
                session_date: row
                    .try_get("session_date")
                    .map_err(|error| error.to_string())?,
            })
        })
        .collect()
}

#[tauri::command]
pub async fn add_identity_override(
    agent_id: String,
    fact: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let fact = fact.trim();
    if fact.is_empty() {
        return Err("identity override cannot be empty".to_string());
    }
    let pool = db_pool(&app).await?;
    let existing = identity_overrides(&pool, &agent_id).await?;
    let mut overrides = existing;
    overrides.push(fact.to_string());
    save_identity_overrides(&pool, &agent_id, &overrides).await?;
    enqueue_text_memory(&pool, &agent_id, "identity", &agent_id, fact, &app, &state).await?;
    Ok(overrides)
}

#[tauri::command]
pub async fn remove_identity_override(
    agent_id: String,
    override_index: u32,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let pool = db_pool(&app).await?;
    let mut overrides = identity_overrides(&pool, &agent_id).await?;
    if (override_index as usize) < overrides.len() {
        overrides.remove(override_index as usize);
        save_identity_overrides(&pool, &agent_id, &overrides).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn clear_memory(agent_id: String, app: tauri::AppHandle) -> Result<(), String> {
    let pool = db_pool(&app).await?;
    for table in [
        "identity",
        "pinned_facts",
        "episodes",
        "transcript",
        "memory_chunks",
        "embedding_jobs",
        "retrieval_logs",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE agent_id = $1"))
            .bind(&agent_id)
            .execute(&pool)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn run_consolidation(
    agent_id: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let pool = db_pool(&app).await?;
    let agent_ids = if let Some(agent_id) = agent_id {
        vec![agent_id]
    } else {
        sqlx::query("SELECT DISTINCT agent_id FROM transcript")
            .fetch_all(&pool)
            .await
            .map_err(|error| error.to_string())?
            .into_iter()
            .filter_map(|row| row.try_get::<String, _>("agent_id").ok())
            .collect()
    };

    for agent_id in agent_ids {
        consolidate_agent(&pool, &agent_id, &app, &state).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_now(
    agent_id: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    run_consolidation(agent_id.clone(), app.clone(), state).await?;
    let pool = db_pool(&app).await?;
    let embed_config = resolve_embed_config(&app).await?;
    rag::enqueue_backfill(&pool, agent_id, embed_config).await?;
    Ok(())
}

#[tauri::command]
pub async fn retrieve_memory(
    agent_id: String,
    query: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<RetrievedChunk>, String> {
    let embed_config = state
        .embed_config
        .lock()
        .map_err(|error| error.to_string())?
        .clone()
        .or_else(|| {
            app_store::load(&app)
                .ok()
                .and_then(|store| store.embed_config)
        })
        .map(|config| secure_store::resolve_provider_api_key("embed", config));
    let pool = db::pool(&app).await?;
    rag::retrieve(&pool, &agent_id, &query, embed_config).await
}

#[tauri::command]
pub async fn import_tokenizer(
    request: ImportTokenizerRequest,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<TokenizerStatusResponse, String> {
    let target = tokenizer_target_path(&app, &state, request.cache_key)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::copy(&request.source_path, &target).map_err(|error| error.to_string())?;
    let status = tokenizer::load_tokenizer(Some(&target));
    if !status.loaded {
        let _ = std::fs::remove_file(&target);
    }
    Ok(TokenizerStatusResponse {
        path: status.path.map(|path| path.to_string_lossy().into_owned()),
        loaded: status.loaded,
        reason: status.reason,
    })
}

#[tauri::command]
pub async fn download_tokenizer(
    request: DownloadTokenizerRequest,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<TokenizerStatusResponse, String> {
    let target = tokenizer_target_path(&app, &state, request.cache_key)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let bytes = reqwest::get(&request.url)
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    std::fs::write(&target, bytes).map_err(|error| error.to_string())?;
    let status = tokenizer::load_tokenizer(Some(&target));
    if !status.loaded {
        let _ = std::fs::remove_file(&target);
    }
    Ok(TokenizerStatusResponse {
        path: status.path.map(|path| path.to_string_lossy().into_owned()),
        loaded: status.loaded,
        reason: status.reason,
    })
}

#[tauri::command]
pub async fn get_tokenizer_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<TokenizerStatusResponse, String> {
    let embed_config = state
        .embed_config
        .lock()
        .map_err(|error| error.to_string())?
        .clone()
        .or_else(|| {
            app_store::load(&app)
                .ok()
                .and_then(|store| store.embed_config)
        });
    let path = tokenizer::resolve_tokenizer_path(&app, embed_config.as_ref())?;
    let status = tokenizer::load_tokenizer(path.as_deref());
    Ok(TokenizerStatusResponse {
        path: status.path.map(|path| path.to_string_lossy().into_owned()),
        loaded: status.loaded,
        reason: status.reason,
    })
}

#[tauri::command]
pub async fn enqueue_memory_backfill(
    agent_id: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let embed_config = state
        .embed_config
        .lock()
        .map_err(|error| error.to_string())?
        .clone()
        .or_else(|| {
            app_store::load(&app)
                .ok()
                .and_then(|store| store.embed_config)
        })
        .map(|config| secure_store::resolve_provider_api_key("embed", config))
        .ok_or_else(|| "No embedding provider configured".to_string())?;
    let pool = db::pool(&app).await?;
    rag::enqueue_backfill(&pool, agent_id, embed_config).await
}

#[tauri::command]
pub async fn process_embedding_jobs(
    limit: Option<i64>,
    app: tauri::AppHandle,
) -> Result<u64, String> {
    let pool = db::pool(&app).await?;
    rag::process_pending_jobs(&pool, &app, limit.unwrap_or(32)).await
}

async fn db_pool(app: &tauri::AppHandle) -> Result<SqlitePool, String> {
    db::pool(app).await
}

fn db_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("honeycomb.db"))
}

async fn count_for_agent(pool: &SqlitePool, table: &str, agent_id: &str) -> Result<i64, String> {
    let row = sqlx::query(&format!(
        "SELECT COUNT(*) AS count FROM {table} WHERE agent_id = $1"
    ))
    .bind(agent_id)
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())?;
    row.try_get("count").map_err(|error| error.to_string())
}

async fn identity_overrides(pool: &SqlitePool, agent_id: &str) -> Result<Vec<String>, String> {
    let row = sqlx::query("SELECT overrides FROM identity WHERE agent_id = $1")
        .bind(agent_id)
        .fetch_optional(pool)
        .await
        .map_err(|error| error.to_string())?;
    let Some(row) = row else {
        return Ok(Vec::new());
    };
    let value = row
        .try_get::<String, _>("overrides")
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&value).map_err(|error| error.to_string())
}

async fn save_identity_overrides(
    pool: &SqlitePool,
    agent_id: &str,
    overrides: &[String],
) -> Result<(), String> {
    let overrides_json = serde_json::to_string(overrides).map_err(|error| error.to_string())?;
    let content = overrides.join("\n");
    sqlx::query(
        "INSERT INTO identity (agent_id, overrides, content, updated_at)
         VALUES ($1, $2, $3, CURRENT_TIMESTAMP)
         ON CONFLICT(agent_id) DO UPDATE SET
           overrides = excluded.overrides,
           content = excluded.content,
           updated_at = CURRENT_TIMESTAMP",
    )
    .bind(agent_id)
    .bind(overrides_json)
    .bind(content)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

async fn resolve_embed_config(app: &tauri::AppHandle) -> Result<ProviderConfig, String> {
    app_store::load(app)?
        .embed_config
        .map(|config| secure_store::resolve_provider_api_key("embed", config))
        .ok_or_else(|| "No embedding provider configured".to_string())
}

async fn enqueue_text_memory(
    pool: &SqlitePool,
    agent_id: &str,
    source_type: &str,
    source_id: &str,
    content: &str,
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
) -> Result<(), String> {
    let embed_config = state
        .embed_config
        .lock()
        .map_err(|error| error.to_string())?
        .clone()
        .or_else(|| {
            app_store::load(app)
                .ok()
                .and_then(|store| store.embed_config)
        })
        .map(|config| secure_store::resolve_provider_api_key("embed", config));
    if let Some(embed_config) = embed_config {
        rag::enqueue_ingest(
            pool,
            RagIngestJob {
                agent_id: agent_id.to_string(),
                session_id: None,
                source_type: source_type.to_string(),
                source_id: source_id.to_string(),
                role: None,
                content: content.to_string(),
                embed_config,
            },
        )
        .await?;
    }
    Ok(())
}

async fn consolidate_agent(
    pool: &SqlitePool,
    agent_id: &str,
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT session_id, role, content, created_at FROM transcript
         WHERE agent_id = $1
         ORDER BY created_at DESC LIMIT 80",
    )
    .bind(agent_id)
    .fetch_all(pool)
    .await
    .map_err(|error| error.to_string())?;

    if rows.is_empty() {
        return Ok(());
    }

    let mut transcript_lines = Vec::new();
    let mut facts = Vec::new();
    let mut session_id = None;
    for row in rows.into_iter().rev() {
        let role: String = row.try_get("role").map_err(|error| error.to_string())?;
        let content: String = row.try_get("content").map_err(|error| error.to_string())?;
        session_id = session_id.or_else(|| row.try_get::<String, _>("session_id").ok());
        transcript_lines.push(format!("{role}: {content}"));
        if role == "user" {
            let lower = content.to_lowercase();
            if lower.contains("remember")
                || lower.contains("my name is")
                || lower.contains("i prefer")
                || lower.contains("i like")
            {
                facts.push(content);
            }
        }
    }

    let session_id = session_id.unwrap_or_else(uuid_v7);
    let summary = summarize_lines(&transcript_lines);
    let episode_id = uuid_v7();
    sqlx::query(
        "INSERT INTO episodes (id, agent_id, session_id, summary, topics, entities, decisions, action_items, salience)
         VALUES ($1, $2, $3, $4, $5, '', '', '', 0.55)",
    )
    .bind(&episode_id)
    .bind(agent_id)
    .bind(&session_id)
    .bind(&summary)
    .bind(extract_topics(&summary).join(", "))
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    enqueue_text_memory(pool, agent_id, "episode", &episode_id, &summary, app, state).await?;

    for fact in facts {
        let fact_id = uuid_v7();
        sqlx::query(
            "INSERT INTO pinned_facts (id, agent_id, content, salience, source_count)
             VALUES ($1, $2, $3, 0.7, 1)",
        )
        .bind(&fact_id)
        .bind(agent_id)
        .bind(&fact)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
        enqueue_text_memory(pool, agent_id, "pinned_fact", &fact_id, &fact, app, state).await?;
    }

    sqlx::query(
        "INSERT INTO memory_config (agent_id, last_consolidated_at)
         VALUES ($1, CURRENT_TIMESTAMP)
         ON CONFLICT(agent_id) DO UPDATE SET last_consolidated_at = CURRENT_TIMESTAMP",
    )
    .bind(agent_id)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn summarize_lines(lines: &[String]) -> String {
    let summary = lines.join("\n");
    if summary.chars().count() > 1800 {
        summary.chars().take(1800).collect::<String>()
    } else {
        summary
    }
}

fn extract_topics(summary: &str) -> Vec<String> {
    summary
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.len() > 4)
        .take(8)
        .map(|word| word.to_lowercase())
        .collect()
}

fn tokenizer_target_path(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    cache_key: Option<String>,
) -> Result<std::path::PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let key = if let Some(cache_key) = cache_key.filter(|key| !key.trim().is_empty()) {
        tokenizer::sanitize_tokenizer_cache_key(&cache_key)
    } else {
        let embed_config = state
            .embed_config
            .lock()
            .map_err(|error| error.to_string())?
            .clone()
            .or_else(|| {
                app_store::load(app)
                    .ok()
                    .and_then(|store| store.embed_config)
            });
        embed_config
            .as_ref()
            .map(tokenizer::tokenizer_cache_key)
            .unwrap_or_else(|| "default".to_string())
    };
    Ok(app_data_dir
        .join("tokenizers")
        .join(key)
        .join("tokenizer.json"))
}
