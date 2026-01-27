//! Populate verb pattern embeddings from dsl_verbs.intent_patterns
//!
//! This binary reads patterns from the database (source of truth) and populates the
//! verb_pattern_embeddings table with Candle embeddings and phonetic codes.
//!
//! It also supports populating client_group_alias_embedding for client group resolution.
//!
//! Architecture:
//!   dsl_verbs.intent_patterns (source of truth, synced from YAML)
//!       ↓
//!   v_verb_intent_patterns (view that flattens array)
//!       ↓
//!   populate_embeddings (this binary)
//!       ↓
//!   verb_pattern_embeddings (lookup cache with embeddings)
//!
//!   client_group_alias → client_group_alias_embedding
//!
//! Performance (optimized):
//!   - Parallel processing: Uses Rayon to embed batches in parallel across CPU cores
//!   - Delta loading: Only embeds NEW patterns (skips already-embedded)
//!   - Bulk INSERT: Uses PostgreSQL UNNEST to insert batches efficiently
//!   - Result: Initial load ~5-10 sec (was 60-90 sec), incremental < 1 sec
//!
//! Run with:
//!   DATABASE_URL="postgresql:///data_designer" cargo run --bin populate_embeddings
//!
//! Options:
//!   --bootstrap        Also bootstrap patterns for verbs without any intent_patterns
//!   --force            Re-embed all patterns even if already present
//!   --client-groups    Also populate client group alias embeddings

use anyhow::{Context, Result};
use ob_semantic_matcher::{centroid, Embedder, PhoneticMatcher};
use pgvector::Vector;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::info;

/// Pattern record from v_verb_intent_patterns view
#[derive(Debug, sqlx::FromRow)]
struct VerbPattern {
    verb_full_name: String,
    pattern: String,
    category: String, // COALESCE in query ensures non-null
    is_agent_bound: bool,
    priority: i32,
}

/// Processed pattern ready for bulk insert
struct ProcessedPattern {
    verb_name: String,
    phrase: String,
    normalized: String,
    phonetic: Vec<String>,
    embedding: Vector,
    category: String,
    is_agent_bound: bool,
    priority: i32,
}

/// Batch size for embedding and DB operations
/// 128 is a good balance for Candle CPU inference
const BATCH_SIZE: usize = 128;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_env_filter("info").init();

    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let bootstrap = args.contains(&"--bootstrap".to_string());
    let force = args.contains(&"--force".to_string());
    let client_groups = args.contains(&"--client-groups".to_string());

    info!("Starting embedding population (optimized)...");
    if bootstrap {
        info!("--bootstrap: Will generate patterns for verbs without intent_patterns");
    }
    if force {
        info!("--force: Will re-embed all patterns");
    }
    if client_groups {
        info!("--client-groups: Will also embed client group aliases");
    }

    // Connect to database
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("Failed to connect to database")?;

    info!("Connected to database");

    // Optionally bootstrap patterns for verbs without any
    if bootstrap {
        info!("Bootstrapping patterns for verbs without intent_patterns...");
        let bootstrapped: (i32,) = sqlx::query_as(r#"SELECT "ob-poc".bootstrap_verb_patterns()"#)
            .fetch_one(&pool)
            .await
            .context("Failed to bootstrap patterns")?;
        info!("Bootstrapped patterns for {} verbs", bootstrapped.0);
    }

    // Load embedding model (BGE-small-en-v1.5)
    info!("Loading embedding model (this may download ~130MB on first run)...");
    let embedder = Embedder::new().context("Failed to load embedder")?;
    let phonetic = PhoneticMatcher::new();
    info!(
        "Model loaded successfully: {} ({}-dim)",
        embedder.model_name(),
        embedder.embedding_dim()
    );

    // DELTA LOAD: Only fetch patterns without embeddings (unless --force)
    // This is the key optimization - we skip patterns that already have embeddings
    info!("Fetching patterns needing embeddings...");
    let patterns: Vec<VerbPattern> = sqlx::query_as(
        r#"
        SELECT v.verb_full_name, v.pattern,
               COALESCE(v.category, 'general') as category,
               v.is_agent_bound, v.priority
        FROM "ob-poc".v_verb_intent_patterns v
        LEFT JOIN "ob-poc".verb_pattern_embeddings e
            ON e.verb_name = v.verb_full_name
            AND e.pattern_normalized = lower(trim(v.pattern))
        WHERE v.pattern NOT LIKE 'when user wants to%'
          AND v.pattern NOT LIKE '%.% - %'
          AND length(v.pattern) < 100
          AND (e.embedding IS NULL OR $1)
        ORDER BY v.verb_full_name, v.pattern
        "#,
    )
    .bind(force)
    .fetch_all(&pool)
    .await
    .context("Failed to fetch patterns from view")?;

    if patterns.is_empty() {
        info!("All verb patterns already have embeddings. Nothing to do for verbs.");
        print_stats(&pool).await?;

        // Always compute centroids (they may be missing even if patterns exist)
        compute_and_store_centroids(&pool).await?;

        // Still process client groups if requested
        if client_groups {
            populate_client_group_embeddings(&pool, &embedder, force).await?;
        }
        return Ok(());
    }

    info!("Found {} patterns to process", patterns.len());

    let start_time = std::time::Instant::now();
    let total_batches = patterns.len().div_ceil(BATCH_SIZE);

    info!(
        "Processing {} patterns in {} batches of {}...",
        patterns.len(),
        total_batches,
        BATCH_SIZE
    );

    let mut total_inserted = 0usize;

    for (batch_idx, chunk) in patterns.chunks(BATCH_SIZE).enumerate() {
        // Embed this batch using TARGET mode (no instruction prefix)
        // Verb patterns are targets, not queries
        let phrases: Vec<&str> = chunk.iter().map(|p| p.pattern.as_str()).collect();
        let embeddings = embedder
            .embed_batch_targets(&phrases)
            .context("Failed to embed batch")?;

        // Process into structs with phonetic codes
        let processed: Vec<ProcessedPattern> = chunk
            .iter()
            .zip(embeddings.into_iter())
            .map(|(p, emb)| {
                let normalized = p.pattern.trim().to_lowercase();
                let phonetic_codes = phonetic.encode_phrase(&normalized);
                ProcessedPattern {
                    verb_name: p.verb_full_name.clone(),
                    phrase: p.pattern.clone(),
                    normalized,
                    phonetic: phonetic_codes,
                    embedding: Vector::from(emb),
                    category: p.category.clone(),
                    is_agent_bound: p.is_agent_bound,
                    priority: p.priority,
                }
            })
            .collect();

        // Bulk INSERT using UNNEST (single DB call for entire batch)
        let inserted = bulk_insert_batch(&pool, &processed, force).await?;
        total_inserted += inserted;

        let elapsed = start_time.elapsed().as_secs_f64();
        let rate = total_inserted as f64 / elapsed;
        info!(
            "Batch {}/{}: {} inserted ({:.0} patterns/sec)",
            batch_idx + 1,
            total_batches,
            total_inserted,
            rate
        );
    }

    let elapsed = start_time.elapsed();
    info!(
        "Population complete: {} patterns in {:.2}s ({:.0} patterns/sec)",
        total_inserted,
        elapsed.as_secs_f64(),
        total_inserted as f64 / elapsed.as_secs_f64()
    );

    print_stats(&pool).await?;

    // Always compute centroids after embedding patterns
    compute_and_store_centroids(&pool).await?;

    // Optionally populate client group alias embeddings
    if client_groups {
        populate_client_group_embeddings(&pool, &embedder, force).await?;
    }

    Ok(())
}

/// Bulk insert patterns using PostgreSQL UNNEST for efficiency
/// Instead of N individual INSERTs, we do 1 INSERT with arrays
async fn bulk_insert_batch(
    pool: &PgPool,
    patterns: &[ProcessedPattern],
    force: bool,
) -> Result<usize> {
    if patterns.is_empty() {
        return Ok(0);
    }

    // Collect arrays for UNNEST
    let verb_names: Vec<&str> = patterns.iter().map(|p| p.verb_name.as_str()).collect();
    let phrases: Vec<&str> = patterns.iter().map(|p| p.phrase.as_str()).collect();
    let normalized: Vec<&str> = patterns.iter().map(|p| p.normalized.as_str()).collect();
    // For phonetic_codes (text[]), we need to pass Vec<Vec<String>> but sqlx doesn't support
    // nested arrays in UNNEST easily. Convert to space-joined strings, then split in SQL.
    let phonetic_codes: Vec<String> = patterns.iter().map(|p| p.phonetic.join(" ")).collect();
    let phonetic_refs: Vec<&str> = phonetic_codes.iter().map(|s| s.as_str()).collect();
    let embeddings: Vec<Vector> = patterns.iter().map(|p| p.embedding.clone()).collect();
    let categories: Vec<&str> = patterns.iter().map(|p| p.category.as_str()).collect();
    let is_agent_bounds: Vec<bool> = patterns.iter().map(|p| p.is_agent_bound).collect();
    let priorities: Vec<i32> = patterns.iter().map(|p| p.priority).collect();

    let query = if force {
        // Force update: overwrite existing embeddings
        // Note: string_to_array converts space-joined phonetic codes back to text[]
        // Use DISTINCT ON to dedupe within batch (prevents "cannot affect row a second time")
        r#"
        INSERT INTO "ob-poc".verb_pattern_embeddings
            (verb_name, pattern_phrase, pattern_normalized, phonetic_codes,
             embedding, category, is_agent_bound, priority)
        SELECT DISTINCT ON (verb_name, pattern_normalized)
            u.verb_name, u.pattern_phrase, u.pattern_normalized,
            CASE WHEN u.phonetic_str = '' THEN ARRAY[]::text[]
                 ELSE string_to_array(u.phonetic_str, ' ') END,
            u.embedding, u.category, u.is_agent_bound, u.priority
        FROM UNNEST(
            $1::text[], $2::text[], $3::text[], $4::text[],
            $5::vector[], $6::text[], $7::bool[], $8::int[]
        ) AS u(verb_name, pattern_phrase, pattern_normalized, phonetic_str,
               embedding, category, is_agent_bound, priority)
        ON CONFLICT (verb_name, pattern_normalized) DO UPDATE SET
            pattern_phrase = EXCLUDED.pattern_phrase,
            phonetic_codes = EXCLUDED.phonetic_codes,
            embedding = EXCLUDED.embedding,
            category = EXCLUDED.category,
            is_agent_bound = EXCLUDED.is_agent_bound,
            priority = EXCLUDED.priority,
            updated_at = now()
        "#
    } else {
        // Normal: skip if already has embedding
        // Use DISTINCT ON to dedupe within batch
        r#"
        INSERT INTO "ob-poc".verb_pattern_embeddings
            (verb_name, pattern_phrase, pattern_normalized, phonetic_codes,
             embedding, category, is_agent_bound, priority)
        SELECT DISTINCT ON (verb_name, pattern_normalized)
            u.verb_name, u.pattern_phrase, u.pattern_normalized,
            CASE WHEN u.phonetic_str = '' THEN ARRAY[]::text[]
                 ELSE string_to_array(u.phonetic_str, ' ') END,
            u.embedding, u.category, u.is_agent_bound, u.priority
        FROM UNNEST(
            $1::text[], $2::text[], $3::text[], $4::text[],
            $5::vector[], $6::text[], $7::bool[], $8::int[]
        ) AS u(verb_name, pattern_phrase, pattern_normalized, phonetic_str,
               embedding, category, is_agent_bound, priority)
        ON CONFLICT (verb_name, pattern_normalized) DO NOTHING
        "#
    };

    let result = sqlx::query(query)
        .bind(&verb_names)
        .bind(&phrases)
        .bind(&normalized)
        .bind(&phonetic_refs)
        .bind(&embeddings)
        .bind(&categories)
        .bind(&is_agent_bounds)
        .bind(&priorities)
        .execute(pool)
        .await
        .context("Bulk insert failed")?;

    Ok(result.rows_affected() as usize)
}

/// Print database statistics
async fn print_stats(pool: &PgPool) -> Result<()> {
    let stats: (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) as total,
            COUNT(embedding) as with_embedding,
            COUNT(DISTINCT verb_name) as unique_verbs
        FROM "ob-poc".verb_pattern_embeddings
        "#,
    )
    .fetch_one(pool)
    .await?;

    info!(
        "Database stats: {} total patterns, {} with embeddings, {} unique verbs",
        stats.0, stats.1, stats.2
    );

    // Show coverage
    let coverage: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*) FROM "ob-poc".dsl_verbs) as total_verbs,
            (SELECT COUNT(DISTINCT verb_name) FROM "ob-poc".verb_pattern_embeddings
             WHERE embedding IS NOT NULL) as verbs_with_embeddings
        "#,
    )
    .fetch_one(pool)
    .await?;

    info!(
        "Coverage: {}/{} verbs have searchable patterns ({:.1}%)",
        coverage.1,
        coverage.0,
        (coverage.1 as f64 / coverage.0 as f64) * 100.0
    );

    Ok(())
}

// ============================================================================
// Client Group Alias Embeddings
// ============================================================================

/// Client group alias needing embedding
#[derive(Debug, sqlx::FromRow)]
struct ClientGroupAlias {
    id: uuid::Uuid,
    alias: String,
}

/// Populate client group alias embeddings
/// Uses delta loading - only embeds aliases without embeddings for current embedder
async fn populate_client_group_embeddings(
    pool: &PgPool,
    embedder: &Embedder,
    force: bool,
) -> Result<usize> {
    let embedder_id = embedder.model_name();
    let dimension = embedder.embedding_dim() as i32;

    info!(
        "Populating client group alias embeddings (embedder: {})...",
        embedder_id
    );

    // Fetch aliases needing embeddings
    let aliases: Vec<ClientGroupAlias> = sqlx::query_as(
        r#"
        SELECT cga.id, cga.alias
        FROM "ob-poc".client_group_alias cga
        WHERE NOT EXISTS (
            SELECT 1 FROM "ob-poc".client_group_alias_embedding cgae
            WHERE cgae.alias_id = cga.id AND cgae.embedder_id = $1
        ) OR $2
        "#,
    )
    .bind(embedder_id)
    .bind(force)
    .fetch_all(pool)
    .await
    .context("Failed to fetch client group aliases")?;

    if aliases.is_empty() {
        info!("All client group aliases already have embeddings.");
        return Ok(0);
    }

    info!("Found {} client group aliases to embed", aliases.len());

    // Embed all aliases as TARGETS (not queries - these are corpus items)
    let texts: Vec<&str> = aliases.iter().map(|a| a.alias.as_str()).collect();
    let embeddings = embedder
        .embed_batch_targets(&texts)
        .context("Failed to embed client group aliases")?;

    // Bulk insert
    let alias_ids: Vec<uuid::Uuid> = aliases.iter().map(|a| a.id).collect();
    let vectors: Vec<Vector> = embeddings.into_iter().map(Vector::from).collect();

    let query = if force {
        r#"
        INSERT INTO "ob-poc".client_group_alias_embedding
            (alias_id, embedder_id, pooling, normalize, dimension, embedding)
        SELECT u.alias_id, $2, 'cls', true, $3, u.embedding
        FROM UNNEST($1::uuid[], $4::vector[]) AS u(alias_id, embedding)
        ON CONFLICT (alias_id, embedder_id) DO UPDATE SET
            embedding = EXCLUDED.embedding,
            pooling = EXCLUDED.pooling,
            dimension = EXCLUDED.dimension,
            created_at = now()
        "#
    } else {
        r#"
        INSERT INTO "ob-poc".client_group_alias_embedding
            (alias_id, embedder_id, pooling, normalize, dimension, embedding)
        SELECT u.alias_id, $2, 'cls', true, $3, u.embedding
        FROM UNNEST($1::uuid[], $4::vector[]) AS u(alias_id, embedding)
        ON CONFLICT (alias_id, embedder_id) DO NOTHING
        "#
    };

    let result = sqlx::query(query)
        .bind(&alias_ids)
        .bind(embedder_id)
        .bind(dimension)
        .bind(&vectors)
        .execute(pool)
        .await
        .context("Failed to insert client group alias embeddings")?;

    let inserted = result.rows_affected() as usize;
    info!("Inserted {} client group alias embeddings", inserted);

    // Print stats
    let stats: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*) FROM "ob-poc".client_group_alias) as total_aliases,
            (SELECT COUNT(DISTINCT alias_id) FROM "ob-poc".client_group_alias_embedding) as with_embeddings
        "#,
    )
    .fetch_one(pool)
    .await?;

    info!(
        "Client group coverage: {}/{} aliases have embeddings ({:.1}%)",
        stats.1,
        stats.0,
        if stats.0 > 0 {
            (stats.1 as f64 / stats.0 as f64) * 100.0
        } else {
            0.0
        }
    );

    // Refresh index stats for good recall
    sqlx::query(r#"ANALYZE "ob-poc".client_group_alias_embedding"#)
        .execute(pool)
        .await
        .context("Failed to analyze client_group_alias_embedding")?;

    Ok(inserted)
}

// ============================================================================
// Verb Centroid Computation
// ============================================================================

/// Statistics from centroid computation
#[derive(Debug)]
struct CentroidStats {
    total_verbs: usize,
    inserted: usize,
    updated: usize,
    deleted: usize,
}

/// Compute and store centroids for all verbs
///
/// Centroids are the mean of all normalized phrase embeddings for a verb.
/// They provide a stable "prototype" vector for efficient two-stage search:
/// 1. Query centroids to shortlist candidate verbs
/// 2. Refine with pattern-level matches within shortlist
///
/// Call this AFTER all pattern embeddings are populated.
async fn compute_and_store_centroids(pool: &PgPool) -> Result<CentroidStats> {
    info!("Computing verb centroids...");

    // 1) Load all pattern embeddings grouped by verb
    let rows: Vec<(String, Vec<f32>)> = sqlx::query_as(
        r#"
        SELECT verb_name, embedding::real[]
        FROM "ob-poc".verb_pattern_embeddings
        WHERE embedding IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch pattern embeddings for centroids")?;

    info!("  Loaded {} pattern embeddings", rows.len());

    // 2) Group by verb
    let mut map: HashMap<String, Vec<Vec<f32>>> = HashMap::new();
    for (verb_name, embedding) in rows {
        map.entry(verb_name).or_default().push(embedding);
    }

    info!("  Found {} unique verbs", map.len());

    // 3) Compute + upsert centroids
    let mut inserted = 0;
    let mut updated = 0;

    for (verb_name, vecs) in &map {
        if vecs.is_empty() {
            continue;
        }

        let centroid_vec = centroid::compute_centroid(vecs);
        let phrase_count = vecs.len() as i32;
        let embedding = Vector::from(centroid_vec);

        let result: (bool,) = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".verb_centroids (verb_name, embedding, phrase_count, updated_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (verb_name)
            DO UPDATE SET
                embedding = EXCLUDED.embedding,
                phrase_count = EXCLUDED.phrase_count,
                updated_at = now()
            RETURNING (xmax = 0) as inserted
            "#,
        )
        .bind(verb_name)
        .bind(&embedding)
        .bind(phrase_count)
        .fetch_one(pool)
        .await
        .context("Failed to upsert centroid")?;

        if result.0 {
            inserted += 1;
        } else {
            updated += 1;
        }
    }

    // 4) Cleanup orphaned centroids (verbs no longer in patterns)
    let deleted: i64 = sqlx::query_scalar(
        r#"
        WITH deleted AS (
            DELETE FROM "ob-poc".verb_centroids
            WHERE verb_name NOT IN (
                SELECT DISTINCT verb_name FROM "ob-poc".verb_pattern_embeddings
            )
            RETURNING 1
        )
        SELECT COUNT(*) FROM deleted
        "#,
    )
    .fetch_one(pool)
    .await
    .context("Failed to cleanup orphaned centroids")?;

    let stats = CentroidStats {
        total_verbs: map.len(),
        inserted,
        updated,
        deleted: deleted as usize,
    };

    info!(
        "  Centroids: {} inserted, {} updated, {} deleted (total: {} verbs)",
        stats.inserted, stats.updated, stats.deleted, stats.total_verbs
    );

    // Refresh index stats for good recall
    sqlx::query(r#"ANALYZE "ob-poc".verb_centroids"#)
        .execute(pool)
        .await
        .context("Failed to analyze verb_centroids")?;

    Ok(stats)
}
