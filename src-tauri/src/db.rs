use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Connection, Executor, SqliteConnection, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;
use tauri::Manager;

use crate::secure_store;

pub const MEMORY_DB_URL: &str = "sqlite:honeycomb.db";

pub struct Migration {
    pub version: i64,
    pub description: &'static str,
    pub sql: &'static str,
}

pub fn migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "create_honeycomb_core_schema",
            sql: CORE_SCHEMA,
        },
        Migration {
            version: 2,
            description: "create_chat_persistence_schema",
            sql: CHAT_SCHEMA,
        },
        Migration {
            version: 3,
            description: "create_rag_memory_schema",
            sql: RAG_SCHEMA,
        },
        Migration {
            version: 4,
            description: "create_embedding_job_queue",
            sql: EMBEDDING_JOB_SCHEMA,
        },
    ]
}

pub async fn initialize(app: &tauri::AppHandle) -> Result<SqlitePool, String> {
    let pool = encrypted_pool(app).await?;
    configure_database(&pool).await?;
    match migrate(&pool).await {
        Ok(()) => Ok(pool),
        Err(error) if should_reset_plaintext_database(&error) => {
            pool.close().await;
            backup_existing_database(app)?;
            let pool = encrypted_pool(app).await?;
            configure_database(&pool).await?;
            migrate(&pool).await?;
            Ok(pool)
        }
        Err(error) => Err(error),
    }
}

pub async fn pool(app: &tauri::AppHandle) -> Result<SqlitePool, String> {
    app.try_state::<SqlitePool>()
        .map(|pool| pool.inner().clone())
        .ok_or_else(|| "database pool is not initialized".to_string())
}

pub fn sqlite_url(app: &tauri::AppHandle) -> Result<String, String> {
    let path = sqlite_path(app)?;
    Ok(format!("sqlite://{}?mode=rwc", path.to_string_lossy()))
}

fn sqlite_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?
        .join("honeycomb.db");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    Ok(path)
}

async fn migrate(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS honeycomb_schema_migrations (
          version INTEGER PRIMARY KEY,
          description TEXT NOT NULL,
          applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    for migration in migrations() {
        let applied = sqlx::query("SELECT 1 FROM honeycomb_schema_migrations WHERE version = $1")
            .bind(migration.version)
            .fetch_optional(pool)
            .await
            .map_err(|error| error.to_string())?
            .is_some();
        if applied {
            continue;
        }

        let mut transaction = pool.begin().await.map_err(|error| error.to_string())?;
        sqlx::raw_sql(migration.sql)
            .execute(&mut *transaction)
            .await
            .map_err(|error| error.to_string())?;
        sqlx::query(
            "INSERT INTO honeycomb_schema_migrations (version, description) VALUES ($1, $2)",
        )
        .bind(migration.version)
        .bind(migration.description)
        .execute(&mut *transaction)
        .await
        .map_err(|error| error.to_string())?;
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

async fn configure_database(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query("PRAGMA journal_mode = WAL")
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;

    Ok(())
}

async fn encrypted_pool(app: &tauri::AppHandle) -> Result<SqlitePool, String> {
    let key = secure_store::database_key()?;
    let url = sqlite_url(app)?;
    let options = SqliteConnectOptions::from_str(&url)
        .map_err(|error| error.to_string())?
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(8)
        .after_connect(move |connection, _meta| {
            let key = key.clone();
            Box::pin(async move {
                key_connection(connection, &key)
                    .await
                    .map_err(sqlx::Error::Protocol)
            })
        })
        .connect_with(options)
        .await
        .map_err(|error| error.to_string())
}

async fn key_connection(connection: &mut SqliteConnection, key: &str) -> Result<(), String> {
    let escaped = key.replace('\'', "''");
    connection
        .execute(format!("PRAGMA key = '{escaped}'").as_str())
        .await
        .map_err(|error| error.to_string())?;
    connection
        .execute("PRAGMA cipher_page_size = 4096")
        .await
        .map_err(|error| error.to_string())?;
    connection
        .execute("PRAGMA kdf_iter = 256000")
        .await
        .map_err(|error| error.to_string())?;
    connection
        .execute("PRAGMA foreign_keys = ON")
        .await
        .map_err(|error| error.to_string())?;
    connection
        .execute("SELECT count(*) FROM sqlite_master")
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn should_reset_plaintext_database(error: &str) -> bool {
    error.contains("file is not a database")
        || error.contains("not an error")
        || error.contains("database disk image is malformed")
}

fn backup_existing_database(app: &tauri::AppHandle) -> Result<(), String> {
    let path = sqlite_path(app)?;
    if !path.exists() {
        return Ok(());
    }
    let backup = path.with_extension(format!("db.plaintext-backup-{}", uuid::Uuid::now_v7()));
    std::fs::rename(path, backup).map_err(|error| error.to_string())
}

const CORE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS identity (
  agent_id TEXT PRIMARY KEY,
  overrides TEXT NOT NULL DEFAULT '[]',
  content TEXT NOT NULL DEFAULT '',
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS pinned_facts (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  content TEXT NOT NULL,
  salience REAL NOT NULL DEFAULT 0.5 CHECK (salience >= 0 AND salience <= 1),
  source_count INTEGER NOT NULL DEFAULT 1,
  use_count INTEGER NOT NULL DEFAULT 0,
  last_used_at TEXT,
  embedding BLOB,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS episodes (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  summary TEXT NOT NULL,
  topics TEXT NOT NULL DEFAULT '',
  entities TEXT NOT NULL DEFAULT '',
  decisions TEXT NOT NULL DEFAULT '',
  action_items TEXT NOT NULL DEFAULT '',
  salience REAL NOT NULL DEFAULT 0.5 CHECK (salience >= 0 AND salience <= 1),
  session_date TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS transcript (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS memory_config (
  agent_id TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 1,
  embedding_backend TEXT NOT NULL DEFAULT 'ollama',
  extraction_mode TEXT NOT NULL DEFAULT 'session_end',
  relevance_gate_mode TEXT NOT NULL DEFAULT 'heuristic',
  memory_budget_tokens INTEGER NOT NULL DEFAULT 800,
  summary_debounce_seconds INTEGER NOT NULL DEFAULT 60,
  consolidation_interval_hours INTEGER NOT NULL DEFAULT 24,
  salience_floor REAL NOT NULL DEFAULT 0.2,
  episode_retention_days INTEGER NOT NULL DEFAULT 365,
  last_consolidated_at TEXT
);

CREATE VIRTUAL TABLE IF NOT EXISTS fts_pinned USING fts5(
  doc_id UNINDEXED,
  content,
  tokenize='unicode61 remove_diacritics 2'
);

CREATE VIRTUAL TABLE IF NOT EXISTS fts_episodes USING fts5(
  doc_id UNINDEXED,
  summary,
  topics,
  entities,
  tokenize='unicode61 remove_diacritics 2'
);

CREATE VIRTUAL TABLE IF NOT EXISTS fts_transcript USING fts5(
  doc_id UNINDEXED,
  content,
  tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS pinned_facts_ai AFTER INSERT ON pinned_facts BEGIN
  INSERT INTO fts_pinned(doc_id, content) VALUES (new.id, new.content);
END;
CREATE TRIGGER IF NOT EXISTS pinned_facts_ad AFTER DELETE ON pinned_facts BEGIN
  DELETE FROM fts_pinned WHERE doc_id = old.id;
END;
CREATE TRIGGER IF NOT EXISTS pinned_facts_au AFTER UPDATE ON pinned_facts BEGIN
  DELETE FROM fts_pinned WHERE doc_id = old.id;
  INSERT INTO fts_pinned(doc_id, content) VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS episodes_ai AFTER INSERT ON episodes BEGIN
  INSERT INTO fts_episodes(doc_id, summary, topics, entities) VALUES (new.id, new.summary, new.topics, new.entities);
END;
CREATE TRIGGER IF NOT EXISTS episodes_ad AFTER DELETE ON episodes BEGIN
  DELETE FROM fts_episodes WHERE doc_id = old.id;
END;
CREATE TRIGGER IF NOT EXISTS episodes_au AFTER UPDATE ON episodes BEGIN
  DELETE FROM fts_episodes WHERE doc_id = old.id;
  INSERT INTO fts_episodes(doc_id, summary, topics, entities) VALUES (new.id, new.summary, new.topics, new.entities);
END;

CREATE TRIGGER IF NOT EXISTS transcript_ai AFTER INSERT ON transcript BEGIN
  INSERT INTO fts_transcript(doc_id, content) VALUES (new.id, new.content);
END;
CREATE TRIGGER IF NOT EXISTS transcript_ad AFTER DELETE ON transcript BEGIN
  DELETE FROM fts_transcript WHERE doc_id = old.id;
END;
CREATE TRIGGER IF NOT EXISTS transcript_au AFTER UPDATE ON transcript BEGIN
  DELETE FROM fts_transcript WHERE doc_id = old.id;
  INSERT INTO fts_transcript(doc_id, content) VALUES (new.id, new.content);
END;
"#;

const CHAT_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS chat_sessions (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  title TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  archived_at TEXT
);

CREATE TABLE IF NOT EXISTS chat_messages (
  id TEXT PRIMARY KEY,
  session_id TEXT NOT NULL,
  agent_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool')),
  content TEXT NOT NULL,
  metadata TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (session_id) REFERENCES chat_sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_agent_updated ON chat_sessions(agent_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_chat_messages_session_created ON chat_messages(session_id, created_at ASC);

CREATE TRIGGER IF NOT EXISTS chat_messages_ai_update_session AFTER INSERT ON chat_messages BEGIN
  UPDATE chat_sessions SET updated_at = CURRENT_TIMESTAMP WHERE id = new.session_id;
END;
"#;

const RAG_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS embedding_models (
  id TEXT PRIMARY KEY,
  provider_kind TEXT NOT NULL,
  model TEXT NOT NULL,
  base_url TEXT,
  dimensions INTEGER NOT NULL,
  metric TEXT NOT NULL DEFAULT 'cosine',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_embedding_models_unique
ON embedding_models(provider_kind, model, COALESCE(base_url, ''), dimensions, metric);

CREATE TABLE IF NOT EXISTS memory_chunks (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  session_id TEXT,
  source_type TEXT NOT NULL CHECK (source_type IN ('identity', 'pinned_fact', 'episode', 'transcript', 'chat_message')),
  source_id TEXT NOT NULL,
  role TEXT,
  content TEXT NOT NULL,
  token_start INTEGER NOT NULL DEFAULT 0,
  token_end INTEGER NOT NULL DEFAULT 0,
  token_count INTEGER NOT NULL DEFAULT 0,
  content_hash TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(agent_id, source_type, source_id, content_hash)
);

CREATE TABLE IF NOT EXISTS memory_embeddings (
  id TEXT PRIMARY KEY,
  chunk_id TEXT NOT NULL,
  embedding_model_id TEXT NOT NULL,
  dimensions INTEGER NOT NULL,
  vector BLOB NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (chunk_id) REFERENCES memory_chunks(id) ON DELETE CASCADE,
  FOREIGN KEY (embedding_model_id) REFERENCES embedding_models(id) ON DELETE CASCADE,
  UNIQUE(chunk_id, embedding_model_id)
);

CREATE VIRTUAL TABLE IF NOT EXISTS fts_memory_chunks USING fts5(
  chunk_id UNINDEXED,
  agent_id UNINDEXED,
  source_type UNINDEXED,
  content,
  tokenize='unicode61 remove_diacritics 2'
);

CREATE TABLE IF NOT EXISTS retrieval_logs (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  query TEXT NOT NULL,
  selected_chunk_ids TEXT NOT NULL,
  retrieval_ms INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_memory_chunks_agent_source ON memory_chunks(agent_id, source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_memory_embeddings_model ON memory_embeddings(embedding_model_id, dimensions);
"#;

const EMBEDDING_JOB_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS embedding_jobs (
  id TEXT PRIMARY KEY,
  agent_id TEXT NOT NULL,
  session_id TEXT,
  source_type TEXT NOT NULL,
  source_id TEXT NOT NULL,
  role TEXT,
  content TEXT NOT NULL,
  embed_config TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'done', 'failed')),
  attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(agent_id, source_type, source_id, content)
);

CREATE INDEX IF NOT EXISTS idx_embedding_jobs_status_created ON embedding_jobs(status, created_at ASC);
"#;
