use std::collections::{HashMap, HashSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use tokenizers::Tokenizer;

use crate::llm::registry;
use crate::memory::tokenizer;
use crate::state::{ProviderConfig, uuid_v7};

const TARGET_TOKENS: usize = 384;
const OVERLAP_TOKENS: usize = 64;
const MAX_CHUNKS_IN_CONTEXT: usize = 8;
const MAX_CONTEXT_CHARS: usize = 8_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RagIngestJob {
    pub agent_id: String,
    pub session_id: Option<String>,
    pub source_type: String,
    pub source_id: String,
    pub role: Option<String>,
    pub content: String,
    pub embed_config: ProviderConfig,
}

#[derive(Clone, Debug)]
pub struct RagIngestRequest {
    pub agent_id: String,
    pub session_id: Option<String>,
    pub source_type: String,
    pub source_id: String,
    pub role: Option<String>,
    pub content: String,
    pub embed_config: Option<ProviderConfig>,
    pub tokenizer: Option<Tokenizer>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievedChunk {
    pub id: String,
    pub source_type: String,
    pub source_id: String,
    pub role: Option<String>,
    pub content: String,
    pub score: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingModelInfo {
    pub id: String,
    pub provider_kind: String,
    pub model: String,
    pub base_url: Option<String>,
    pub dimensions: usize,
    pub metric: String,
}

#[derive(Clone, Debug)]
struct ChunkCandidate {
    id: String,
    source_type: String,
    source_id: String,
    role: Option<String>,
    content: String,
    score: f32,
    vector: Option<Vec<f32>>,
}

pub async fn enqueue_ingest(pool: &SqlitePool, job: RagIngestJob) -> Result<(), String> {
    sqlx::query(
        "INSERT OR IGNORE INTO embedding_jobs
         (id, agent_id, session_id, source_type, source_id, role, content, embed_config)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(uuid_v7())
    .bind(&job.agent_id)
    .bind(&job.session_id)
    .bind(&job.source_type)
    .bind(&job.source_id)
    .bind(&job.role)
    .bind(&job.content)
    .bind(serde_json::to_string(&job.embed_config).map_err(|error| error.to_string())?)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub async fn process_pending_jobs(
    pool: &SqlitePool,
    app: &tauri::AppHandle,
    limit: i64,
) -> Result<u64, String> {
    let rows = sqlx::query(
        "SELECT id, agent_id, session_id, source_type, source_id, role, content, embed_config
         FROM embedding_jobs
         WHERE status IN ('pending', 'failed') AND attempts < 5
         ORDER BY created_at ASC
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|error| error.to_string())?;

    let mut processed = 0;
    for row in rows {
        let job_id: String = row.try_get("id").map_err(|error| error.to_string())?;
        sqlx::query(
            "UPDATE embedding_jobs
             SET status = 'processing', attempts = attempts + 1, updated_at = CURRENT_TIMESTAMP
             WHERE id = $1",
        )
        .bind(&job_id)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;

        let embed_config_json: String = row
            .try_get("embed_config")
            .map_err(|error| error.to_string())?;
        let embed_config: ProviderConfig =
            serde_json::from_str(&embed_config_json).map_err(|error| error.to_string())?;
        let tokenizer_path = tokenizer::resolve_tokenizer_path(app, Some(&embed_config))?;
        let tokenizer = tokenizer::tokenizer_from_path(tokenizer_path.as_deref());

        let request = RagIngestRequest {
            agent_id: row.try_get("agent_id").map_err(|error| error.to_string())?,
            session_id: row
                .try_get("session_id")
                .map_err(|error| error.to_string())?,
            source_type: row
                .try_get("source_type")
                .map_err(|error| error.to_string())?,
            source_id: row
                .try_get("source_id")
                .map_err(|error| error.to_string())?,
            role: row.try_get("role").map_err(|error| error.to_string())?,
            content: row.try_get("content").map_err(|error| error.to_string())?,
            embed_config: Some(embed_config),
            tokenizer,
        };

        match ingest(pool, request).await {
            Ok(()) => {
                sqlx::query(
                    "UPDATE embedding_jobs
                     SET status = 'done', last_error = NULL, updated_at = CURRENT_TIMESTAMP
                     WHERE id = $1",
                )
                .bind(&job_id)
                .execute(pool)
                .await
                .map_err(|error| error.to_string())?;
                processed += 1;
            }
            Err(error) => {
                sqlx::query(
                    "UPDATE embedding_jobs
                     SET status = 'failed', last_error = $1, updated_at = CURRENT_TIMESTAMP
                     WHERE id = $2",
                )
                .bind(error)
                .bind(&job_id)
                .execute(pool)
                .await
                .map_err(|error| error.to_string())?;
            }
        }
    }

    Ok(processed)
}

pub async fn enqueue_backfill(
    pool: &SqlitePool,
    agent_id: Option<String>,
    embed_config: ProviderConfig,
) -> Result<u64, String> {
    let rows = if let Some(agent_id) = agent_id {
        sqlx::query(
            "SELECT id, agent_id, session_id, role, content
             FROM chat_messages
             WHERE agent_id = $1
             ORDER BY created_at ASC",
        )
        .bind(agent_id)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            "SELECT id, agent_id, session_id, role, content
             FROM chat_messages
             ORDER BY created_at ASC",
        )
        .fetch_all(pool)
        .await
    }
    .map_err(|error| error.to_string())?;

    let mut enqueued = 0;
    for row in rows {
        enqueue_ingest(
            pool,
            RagIngestJob {
                agent_id: row.try_get("agent_id").map_err(|error| error.to_string())?,
                session_id: row
                    .try_get("session_id")
                    .map_err(|error| error.to_string())?,
                source_type: "chat_message".to_string(),
                source_id: row.try_get("id").map_err(|error| error.to_string())?,
                role: row.try_get("role").map_err(|error| error.to_string())?,
                content: row.try_get("content").map_err(|error| error.to_string())?,
                embed_config: embed_config.clone(),
            },
        )
        .await?;
        enqueued += 1;
    }

    Ok(enqueued)
}

pub async fn ingest(pool: &SqlitePool, request: RagIngestRequest) -> Result<(), String> {
    if request.content.trim().is_empty() {
        return Ok(());
    }

    let Some(embed_config) = request.embed_config else {
        return Ok(());
    };

    let provider = registry::create_provider(embed_config.clone())?;
    let chunks = chunk_text(&request.content, request.tokenizer.as_ref());
    for chunk in chunks {
        let content_hash = stable_hash(&chunk.content);
        let chunk_id = uuid_v7();

        sqlx::query(
            "INSERT OR IGNORE INTO memory_chunks
             (id, agent_id, session_id, source_type, source_id, role, content, token_start, token_end, token_count, content_hash)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(&chunk_id)
        .bind(&request.agent_id)
        .bind(&request.session_id)
        .bind(&request.source_type)
        .bind(&request.source_id)
        .bind(&request.role)
        .bind(&chunk.content)
        .bind(chunk.token_start as i64)
        .bind(chunk.token_end as i64)
        .bind(chunk.token_count as i64)
        .bind(&content_hash)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;

        let stored_chunk_id = sqlx::query(
            "SELECT id FROM memory_chunks
             WHERE agent_id = $1 AND source_type = $2 AND source_id = $3 AND content_hash = $4",
        )
        .bind(&request.agent_id)
        .bind(&request.source_type)
        .bind(&request.source_id)
        .bind(&content_hash)
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())?
        .try_get::<String, _>("id")
        .map_err(|error| error.to_string())?;

        sqlx::query(
            "INSERT OR IGNORE INTO fts_memory_chunks (chunk_id, agent_id, source_type, content)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(&stored_chunk_id)
        .bind(&request.agent_id)
        .bind(&request.source_type)
        .bind(&chunk.content)
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;

        let already_embedded = sqlx::query(
            "SELECT 1 FROM memory_embeddings me
             JOIN embedding_models em ON em.id = me.embedding_model_id
             WHERE me.chunk_id = $1 AND em.provider_kind = $2 AND em.model = $3 AND COALESCE(em.base_url, '') = $4",
        )
        .bind(&stored_chunk_id)
        .bind(provider_kind(&embed_config))
        .bind(provider_model(&embed_config))
        .bind(provider_base_url(&embed_config).unwrap_or_default())
        .fetch_optional(pool)
        .await
        .map_err(|error| error.to_string())?
        .is_some();

        if already_embedded {
            continue;
        }

        let embedding = provider.embed(chunk.content.clone()).await?;
        if embedding.is_empty() {
            continue;
        }

        let model = upsert_embedding_model(pool, &embed_config, embedding.len()).await?;
        sqlx::query(
            "INSERT OR IGNORE INTO memory_embeddings (id, chunk_id, embedding_model_id, dimensions, vector)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(uuid_v7())
        .bind(&stored_chunk_id)
        .bind(&model.id)
        .bind(embedding.len() as i64)
        .bind(f32_vec_to_blob(&embedding))
        .execute(pool)
        .await
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub async fn retrieve_context(
    pool: &SqlitePool,
    agent_id: &str,
    query: &str,
    embed_config: Option<ProviderConfig>,
) -> Result<String, String> {
    if query.trim().is_empty() || !looks_memory_relevant(query) {
        return Ok(String::new());
    }

    let started_at = Instant::now();
    let retrieved = retrieve(pool, agent_id, query, embed_config).await?;
    if retrieved.is_empty() {
        return Ok(String::new());
    }

    let selected_ids = retrieved
        .iter()
        .map(|chunk| chunk.id.clone())
        .collect::<Vec<_>>();
    sqlx::query(
        "INSERT INTO retrieval_logs (id, agent_id, query, selected_chunk_ids, retrieval_ms)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(uuid_v7())
    .bind(agent_id)
    .bind(query)
    .bind(serde_json::to_string(&selected_ids).map_err(|error| error.to_string())?)
    .bind(started_at.elapsed().as_millis() as i64)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    Ok(format_context(&retrieved))
}

pub async fn retrieve(
    pool: &SqlitePool,
    agent_id: &str,
    query: &str,
    embed_config: Option<ProviderConfig>,
) -> Result<Vec<RetrievedChunk>, String> {
    let mut candidates = bm25_candidates(pool, agent_id, query).await?;

    if let Some(embed_config) = embed_config
        && let Ok(vector_candidates) = vector_candidates(pool, agent_id, query, embed_config).await
    {
        candidates = fuse_candidates(candidates, vector_candidates);
    }

    let selected = mmr_select(candidates, MAX_CHUNKS_IN_CONTEXT, MAX_CONTEXT_CHARS);
    Ok(selected
        .into_iter()
        .map(|candidate| RetrievedChunk {
            id: candidate.id,
            source_type: candidate.source_type,
            source_id: candidate.source_id,
            role: candidate.role,
            content: candidate.content,
            score: candidate.score,
        })
        .collect())
}

async fn bm25_candidates(
    pool: &SqlitePool,
    agent_id: &str,
    query: &str,
) -> Result<Vec<ChunkCandidate>, String> {
    let fts_query = sanitize_fts_query(query);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query(
        "SELECT mc.id, mc.source_type, mc.source_id, mc.role, mc.content, bm25(fts_memory_chunks) AS score
         FROM fts_memory_chunks
         JOIN memory_chunks mc ON mc.id = fts_memory_chunks.chunk_id
         WHERE fts_memory_chunks MATCH $1 AND fts_memory_chunks.agent_id = $2
         ORDER BY score
         LIMIT 32",
    )
    .bind(fts_query)
    .bind(agent_id)
    .fetch_all(pool)
    .await
    .map_err(|error| error.to_string())?;

    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            Ok(ChunkCandidate {
                id: row.try_get("id").map_err(|error| error.to_string())?,
                source_type: row
                    .try_get("source_type")
                    .map_err(|error| error.to_string())?,
                source_id: row
                    .try_get("source_id")
                    .map_err(|error| error.to_string())?,
                role: row.try_get("role").map_err(|error| error.to_string())?,
                content: row.try_get("content").map_err(|error| error.to_string())?,
                score: reciprocal_rank(index),
                vector: None,
            })
        })
        .collect()
}

async fn vector_candidates(
    pool: &SqlitePool,
    agent_id: &str,
    query: &str,
    embed_config: ProviderConfig,
) -> Result<Vec<ChunkCandidate>, String> {
    let provider = registry::create_provider(embed_config.clone())?;
    let query_embedding = provider.embed(query.to_string()).await?;
    if query_embedding.is_empty() {
        return Ok(Vec::new());
    }

    let model = upsert_embedding_model(pool, &embed_config, query_embedding.len()).await?;
    let rows = sqlx::query(
        "SELECT mc.id, mc.source_type, mc.source_id, mc.role, mc.content, me.vector
         FROM memory_embeddings me
         JOIN memory_chunks mc ON mc.id = me.chunk_id
         WHERE mc.agent_id = $1 AND me.embedding_model_id = $2 AND me.dimensions = $3",
    )
    .bind(agent_id)
    .bind(&model.id)
    .bind(query_embedding.len() as i64)
    .fetch_all(pool)
    .await
    .map_err(|error| error.to_string())?;

    let mut candidates = rows
        .into_iter()
        .map(|row| {
            let vector =
                blob_to_f32_vec(row.try_get("vector").map_err(|error| error.to_string())?)?;
            let score = cosine_similarity(&query_embedding, &vector);
            Ok(ChunkCandidate {
                id: row.try_get("id").map_err(|error| error.to_string())?,
                source_type: row
                    .try_get("source_type")
                    .map_err(|error| error.to_string())?,
                source_id: row
                    .try_get("source_id")
                    .map_err(|error| error.to_string())?,
                role: row.try_get("role").map_err(|error| error.to_string())?,
                content: row.try_get("content").map_err(|error| error.to_string())?,
                score,
                vector: Some(vector),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
    candidates.truncate(32);
    for (index, candidate) in candidates.iter_mut().enumerate() {
        candidate.score = reciprocal_rank(index);
    }
    Ok(candidates)
}

async fn upsert_embedding_model(
    pool: &SqlitePool,
    config: &ProviderConfig,
    dimensions: usize,
) -> Result<EmbeddingModelInfo, String> {
    let provider_kind = provider_kind(config);
    let model = provider_model(config);
    let base_url = provider_base_url(config);
    let id = uuid_v7();

    sqlx::query(
        "INSERT OR IGNORE INTO embedding_models (id, provider_kind, model, base_url, dimensions, metric)
         VALUES ($1, $2, $3, $4, $5, 'cosine')",
    )
    .bind(&id)
    .bind(&provider_kind)
    .bind(&model)
    .bind(&base_url)
    .bind(dimensions as i64)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    let row = sqlx::query(
        "SELECT id, provider_kind, model, base_url, dimensions, metric
         FROM embedding_models
         WHERE provider_kind = $1 AND model = $2 AND COALESCE(base_url, '') = $3 AND dimensions = $4 AND metric = 'cosine'",
    )
    .bind(&provider_kind)
    .bind(&model)
    .bind(base_url.clone().unwrap_or_default())
    .bind(dimensions as i64)
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())?;

    Ok(EmbeddingModelInfo {
        id: row.try_get("id").map_err(|error| error.to_string())?,
        provider_kind: row
            .try_get("provider_kind")
            .map_err(|error| error.to_string())?,
        model: row.try_get("model").map_err(|error| error.to_string())?,
        base_url: row.try_get("base_url").map_err(|error| error.to_string())?,
        dimensions: row
            .try_get::<i64, _>("dimensions")
            .map_err(|error| error.to_string())? as usize,
        metric: row.try_get("metric").map_err(|error| error.to_string())?,
    })
}

#[derive(Clone, Debug)]
struct TextChunk {
    content: String,
    token_start: usize,
    token_end: usize,
    token_count: usize,
}

fn chunk_text(text: &str, tokenizer: Option<&Tokenizer>) -> Vec<TextChunk> {
    if let Some(tokenizer) = tokenizer
        && let Ok(chunks) = chunk_with_huggingface_tokenizer(text, tokenizer)
        && !chunks.is_empty()
    {
        return chunks;
    }

    chunk_with_approximate_tokenizer(text)
}

fn chunk_with_huggingface_tokenizer(
    text: &str,
    tokenizer: &Tokenizer,
) -> Result<Vec<TextChunk>, String> {
    let encoding = tokenizer
        .encode(text, false)
        .map_err(|error| error.to_string())?;
    let offsets = encoding.get_offsets();
    if offsets.is_empty() {
        return Ok(Vec::new());
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < offsets.len() {
        let end = usize::min(start + TARGET_TOKENS, offsets.len());
        let byte_start = floor_char_boundary(text, offsets[start].0);
        let byte_end = ceil_char_boundary(text, offsets[end - 1].1);
        let content = text
            .get(byte_start..byte_end)
            .unwrap_or_default()
            .trim()
            .to_string();

        if !content.is_empty() {
            chunks.push(TextChunk {
                content,
                token_start: start,
                token_end: end,
                token_count: end - start,
            });
        }

        if end == offsets.len() {
            break;
        }
        start = end.saturating_sub(OVERLAP_TOKENS);
    }

    Ok(chunks)
}

fn chunk_with_approximate_tokenizer(text: &str) -> Vec<TextChunk> {
    let tokens = tokenize_approximately(text);
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < tokens.len() {
        let end = usize::min(start + TARGET_TOKENS, tokens.len());
        let content = tokens[start..end].join(" ");
        chunks.push(TextChunk {
            content,
            token_start: start,
            token_end: end,
            token_count: end - start,
        });

        if end == tokens.len() {
            break;
        }
        start = end.saturating_sub(OVERLAP_TOKENS);
    }
    chunks
}

fn floor_char_boundary(text: &str, index: usize) -> usize {
    let mut index = usize::min(index, text.len());
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(text: &str, index: usize) -> usize {
    let mut index = usize::min(index, text.len());
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

fn tokenize_approximately(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn fuse_candidates(
    bm25_candidates: Vec<ChunkCandidate>,
    vector_candidates: Vec<ChunkCandidate>,
) -> Vec<ChunkCandidate> {
    let mut by_id = HashMap::<String, ChunkCandidate>::new();
    for candidate in bm25_candidates.into_iter().chain(vector_candidates) {
        by_id
            .entry(candidate.id.clone())
            .and_modify(|existing| {
                existing.score += candidate.score;
                if existing.vector.is_none() {
                    existing.vector = candidate.vector.clone();
                }
            })
            .or_insert(candidate);
    }

    let mut candidates = by_id.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
    candidates
}

fn mmr_select(
    mut candidates: Vec<ChunkCandidate>,
    limit: usize,
    max_chars: usize,
) -> Vec<ChunkCandidate> {
    let mut selected = Vec::new();
    let mut selected_ids = HashSet::new();
    let mut total_chars = 0;

    while selected.len() < limit && !candidates.is_empty() {
        let mut best_index = 0;
        let mut best_score = f32::MIN;
        for (index, candidate) in candidates.iter().enumerate() {
            if selected_ids.contains(&candidate.id) {
                continue;
            }
            let redundancy = selected
                .iter()
                .map(|selected: &ChunkCandidate| {
                    shingle_similarity(&selected.content, &candidate.content)
                })
                .fold(0.0_f32, f32::max);
            let score = 0.75 * candidate.score - 0.25 * redundancy;
            if score > best_score {
                best_score = score;
                best_index = index;
            }
        }

        let candidate = candidates.remove(best_index);
        if total_chars + candidate.content.len() > max_chars && !selected.is_empty() {
            break;
        }
        total_chars += candidate.content.len();
        selected_ids.insert(candidate.id.clone());
        selected.push(candidate);
    }

    selected
}

fn format_context(chunks: &[RetrievedChunk]) -> String {
    if chunks.is_empty() {
        return String::new();
    }

    let mut context = String::from("Relevant persistent memory from SQLite RAG:\n");
    for (index, chunk) in chunks.iter().enumerate() {
        context.push_str(&format!(
            "\n[{}] source={} role={} score={:.3}\n{}\n",
            index + 1,
            chunk.source_type,
            chunk.role.as_deref().unwrap_or("unknown"),
            chunk.score,
            chunk.content
        ));
    }
    context
}

fn looks_memory_relevant(query: &str) -> bool {
    let query = query.to_lowercase();
    let triggers = [
        "remember",
        "last time",
        "we discussed",
        "what did",
        "what was",
        "previous",
        "before",
        "my ",
        "i prefer",
        "i like",
        "i am",
        "who am i",
        "exact",
        "verbatim",
    ];
    triggers.iter().any(|trigger| query.contains(trigger)) || query.split_whitespace().count() > 8
}

fn sanitize_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|term| {
            term.chars()
                .filter(|character| {
                    character.is_alphanumeric() || *character == '_' || *character == '-'
                })
                .collect::<String>()
        })
        .filter(|term| !term.is_empty())
        .take(12)
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn reciprocal_rank(index: usize) -> f32 {
    1.0 / (60.0 + index as f32 + 1.0)
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut left_norm = 0.0;
    let mut right_norm = 0.0;
    for (left, right) in left.iter().zip(right) {
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn shingle_similarity(left: &str, right: &str) -> f32 {
    let left = shingles(left);
    let right = shingles(right);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let intersection = left.intersection(&right).count() as f32;
    let union = left.union(&right).count() as f32;
    intersection / union
}

fn shingles(text: &str) -> HashSet<String> {
    let words = text
        .to_lowercase()
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    words
        .windows(4)
        .map(|window| window.join(" "))
        .collect::<HashSet<_>>()
}

fn f32_vec_to_blob(vector: &[f32]) -> Vec<u8> {
    vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn blob_to_f32_vec(blob: Vec<u8>) -> Result<Vec<f32>, String> {
    if !blob.len().is_multiple_of(4) {
        return Err("invalid vector blob length".to_string());
    }
    Ok(blob
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn stable_hash(content: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn provider_kind(config: &ProviderConfig) -> String {
    match config {
        ProviderConfig::OpenAi { .. } => "openAi",
        ProviderConfig::Anthropic { .. } => "anthropic",
        ProviderConfig::Ollama { .. } => "ollama",
        ProviderConfig::OpenRouter { .. } => "openRouter",
        ProviderConfig::Custom { .. } => "custom",
    }
    .to_string()
}

fn provider_model(config: &ProviderConfig) -> String {
    match config {
        ProviderConfig::OpenAi { model, .. }
        | ProviderConfig::Anthropic { model, .. }
        | ProviderConfig::Ollama { model, .. }
        | ProviderConfig::OpenRouter { model, .. }
        | ProviderConfig::Custom { model, .. } => model.clone(),
    }
}

fn provider_base_url(config: &ProviderConfig) -> Option<String> {
    match config {
        ProviderConfig::OpenAi { base_url, .. } | ProviderConfig::Ollama { base_url, .. } => {
            base_url.clone()
        }
        ProviderConfig::Custom { base_url, .. } => Some(base_url.clone()),
        ProviderConfig::Anthropic { .. } | ProviderConfig::OpenRouter { .. } => None,
    }
}
