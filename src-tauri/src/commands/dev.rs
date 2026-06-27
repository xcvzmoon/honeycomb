use serde::Serialize;
use serde_json::{Value, json};
use sqlx::{Column, Row, SqlitePool, TypeInfo};
use tauri::State;

use crate::app_store;
use crate::db;
use crate::state::AppState;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingJobStats {
    pub pending: i64,
    pub processing: i64,
    pub done: i64,
    pub failed: i64,
    pub total: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseTableInfo {
    pub name: String,
    pub table_type: String,
    pub row_count: Option<i64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseRowsPreview {
    pub table: String,
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InternalConfigSnapshot {
    pub provider_configured: bool,
    pub embed_configured: bool,
    pub agent_count: usize,
    pub settings: Value,
    pub app_store: Value,
}

#[tauri::command]
pub async fn get_embedding_job_stats(app: tauri::AppHandle) -> Result<EmbeddingJobStats, String> {
    let pool = db_pool(&app).await?;
    let rows = sqlx::query(
        "SELECT status, COUNT(*) AS count
         FROM embedding_jobs
         GROUP BY status",
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| error.to_string())?;

    let mut stats = EmbeddingJobStats {
        pending: 0,
        processing: 0,
        done: 0,
        failed: 0,
        total: 0,
    };

    for row in rows {
        let status: String = row.try_get("status").map_err(|error| error.to_string())?;
        let count: i64 = row.try_get("count").map_err(|error| error.to_string())?;
        match status.as_str() {
            "pending" => stats.pending = count,
            "processing" => stats.processing = count,
            "done" => stats.done = count,
            "failed" => stats.failed = count,
            _ => {}
        }
        stats.total += count;
    }

    Ok(stats)
}

#[tauri::command]
pub async fn list_database_tables(app: tauri::AppHandle) -> Result<Vec<DatabaseTableInfo>, String> {
    let pool = db_pool(&app).await?;
    let rows = sqlx::query(
        "SELECT name, type
         FROM sqlite_schema
         WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
         ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| error.to_string())?;

    let mut tables = Vec::new();
    for row in rows {
        let name: String = row.try_get("name").map_err(|error| error.to_string())?;
        let table_type: String = row.try_get("type").map_err(|error| error.to_string())?;
        let row_count = if is_safe_identifier(&name) && !name.starts_with("fts_") {
            let count_sql = format!("SELECT COUNT(*) AS count FROM {name}");
            sqlx::query(&count_sql)
                .fetch_optional(&pool)
                .await
                .ok()
                .flatten()
                .and_then(|row| row.try_get::<i64, _>("count").ok())
        } else {
            None
        };
        tables.push(DatabaseTableInfo {
            name,
            table_type,
            row_count,
        });
    }

    Ok(tables)
}

#[tauri::command]
pub async fn preview_database_rows(
    table: String,
    limit: Option<i64>,
    offset: Option<i64>,
    app: tauri::AppHandle,
) -> Result<DatabaseRowsPreview, String> {
    if !is_safe_identifier(&table) {
        return Err("invalid table name".to_string());
    }

    let limit = limit.unwrap_or(50).clamp(1, 200);
    let offset = offset.unwrap_or(0).max(0);
    let pool = db_pool(&app).await?;
    ensure_table_exists(&pool, &table).await?;

    let sql = format!("SELECT * FROM {table} LIMIT $1 OFFSET $2");
    let rows = sqlx::query(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|error| error.to_string())?;

    let columns = rows
        .first()
        .map(|row| {
            row.columns()
                .iter()
                .map(|column| column.name().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(Vec::new);

    let rows = rows
        .into_iter()
        .map(row_to_json)
        .collect::<Result<Vec<_>, String>>()?;

    Ok(DatabaseRowsPreview {
        table,
        columns,
        rows,
        limit,
        offset,
    })
}

#[tauri::command]
pub async fn get_internal_config_snapshot(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<InternalConfigSnapshot, String> {
    let provider_configured = state
        .provider_config
        .lock()
        .map_err(|error| error.to_string())?
        .is_some();
    let embed_configured = state
        .embed_config
        .lock()
        .map_err(|error| error.to_string())?
        .is_some();
    let agent_count = state
        .agents
        .lock()
        .map_err(|error| error.to_string())?
        .len();
    let settings = state
        .settings
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let app_store = app_store::load(&app)?;

    Ok(InternalConfigSnapshot {
        provider_configured,
        embed_configured,
        agent_count,
        settings: serde_json::to_value(settings).map_err(|error| error.to_string())?,
        app_store: serde_json::to_value(app_store).map_err(|error| error.to_string())?,
    })
}

async fn db_pool(app: &tauri::AppHandle) -> Result<SqlitePool, String> {
    db::pool(app).await
}

async fn ensure_table_exists(pool: &SqlitePool, table: &str) -> Result<(), String> {
    let exists = sqlx::query(
        "SELECT 1
         FROM sqlite_schema
         WHERE type IN ('table', 'view') AND name = $1",
    )
    .bind(table)
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())?
    .is_some();

    if exists {
        Ok(())
    } else {
        Err(format!("table not found: {table}"))
    }
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn row_to_json(row: sqlx::sqlite::SqliteRow) -> Result<Value, String> {
    let mut object = serde_json::Map::new();
    for column in row.columns() {
        let name = column.name();
        let type_name = column.type_info().name().to_uppercase();
        let value = if type_name.contains("INT") {
            row.try_get::<Option<i64>, _>(name)
                .map(|value| value.map_or(Value::Null, |value| json!(value)))
        } else if type_name.contains("REAL")
            || type_name.contains("FLOA")
            || type_name.contains("DOUB")
        {
            row.try_get::<Option<f64>, _>(name)
                .map(|value| value.map_or(Value::Null, |value| json!(value)))
        } else if type_name.contains("BLOB") {
            row.try_get::<Option<Vec<u8>>, _>(name).map(|value| {
                value.map_or(Value::Null, |value| {
                    json!(format!("<blob:{} bytes>", value.len()))
                })
            })
        } else {
            row.try_get::<Option<String>, _>(name)
                .map(|value| value.map_or(Value::Null, Value::String))
        }
        .map_err(|error| error.to_string())?;
        object.insert(name.to_string(), value);
    }
    Ok(Value::Object(object))
}
