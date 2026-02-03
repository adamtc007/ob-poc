//! Integration tests for utterance segmentation
//!
//! These tests verify the full pipeline including database resolution.
//!
//! Run with: cargo test --features database --test utterance_integration -- --nocapture --ignored

use sqlx::postgres::PgPoolOptions;

async fn get_pool() -> sqlx::PgPool {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database")
}

/// Test that we can connect and query client_group_alias
#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_client_group_exists() {
    let pool = get_pool().await;
    // Check if there's any data in client_group
    let count: i64 =
        sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!" FROM "ob-poc".client_group"#)
            .fetch_one(&pool)
            .await
            .unwrap();

    println!("client_group count: {}", count);

    // Check if there's any data in client_group_alias
    let alias_count: i64 =
        sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!" FROM "ob-poc".client_group_alias"#)
            .fetch_one(&pool)
            .await
            .unwrap();

    println!("client_group_alias count: {}", alias_count);

    // List all aliases if any exist
    if alias_count > 0 {
        let aliases: Vec<(String, String)> = sqlx::query_as(
            r#"
            SELECT cga.alias_norm, cg.canonical_name
            FROM "ob-poc".client_group_alias cga
            JOIN "ob-poc".client_group cg ON cg.id = cga.group_id
            LIMIT 20
            "#,
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        println!("Aliases:");
        for (alias, name) in aliases {
            println!("  {} -> {}", alias, name);
        }
    }
}

/// Test full segmentation of "work on allianz"
#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_segment_work_on_allianz() {
    let pool = get_pool().await;

    use ob_poc::mcp::utterance::segment_utterance;

    let seg = segment_utterance("work on allianz", &pool).await;

    println!("Input: 'work on allianz'");
    println!(
        "Verb phrase: '{}' (confidence: {})",
        seg.verb_phrase.text, seg.verb_phrase.confidence
    );
    println!(
        "Resolved group: {:?}",
        seg.resolved_group
            .as_ref()
            .map(|g| (&g.canonical_name, g.group_id, g.confidence))
    );
    println!(
        "Scope phrase: {:?}",
        seg.scope_phrase.as_ref().map(|s| &s.text)
    );
    println!("is_likely_typo: {}", seg.is_likely_typo());
    println!("is_likely_garbage: {}", seg.is_likely_garbage());
    println!("Method trace:");
    for step in &seg.method_trace {
        println!(
            "  Pass {}: {} (conf: {:?})",
            step.pass, step.action, step.confidence
        );
    }

    // Assertions
    assert_eq!(seg.verb_phrase.text, "work on", "Verb should be 'work on'");
    assert!(
        seg.verb_phrase.confidence >= 0.5,
        "Verb confidence should be >= 0.5"
    );
    assert!(seg.resolved_group.is_some(), "Group should be resolved");
    assert!(seg.group_id().is_some(), "Group ID should be available");
    assert!(!seg.is_likely_typo(), "Should NOT be detected as typo");
}

/// Test verb search for "work on"
#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_verb_search_work_on() {
    let pool = get_pool().await;

    // Try actual semantic search
    use ob_poc::database::VerbService;
    use std::sync::Arc;

    let verb_service = Arc::new(VerbService::new(pool.clone()));

    // Get embeddings and search
    use ob_poc::agent::learning::embedder::{CandleEmbedder, Embedder};
    let embedder = CandleEmbedder::new().expect("Failed to create embedder");
    let query_emb = embedder
        .embed_query("work on")
        .await
        .expect("Failed to embed");

    let results = verb_service
        .search_verb_patterns_semantic(&query_emb, 10, 0.3)
        .await
        .unwrap();

    println!("Semantic search results for 'work on' (threshold 0.3):");
    for m in &results {
        println!(
            "  {} (score: {:.3}) - matched phrase: '{}'",
            m.verb, m.similarity, m.phrase
        );
    }

    // Check invocation phrases in dsl_verbs
    let invocations: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT unnest(yaml_intent_patterns) as pattern, full_name
        FROM "ob-poc".dsl_verbs
        WHERE full_name LIKE 'session.%'
        LIMIT 50
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    println!("Session verb invocation patterns containing 'work' or 'set':");
    for (pattern, verb) in &invocations {
        if pattern.to_lowercase().contains("work") || pattern.to_lowercase().contains("set") {
            println!("  '{}' -> {}", pattern, verb);
        }
    }

    // Check v_verb_intent_patterns view
    let view_patterns: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT pattern, verb_full_name
        FROM "ob-poc".v_verb_intent_patterns
        WHERE pattern ILIKE '%work%' OR pattern ILIKE '%set client%'
        LIMIT 20
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    println!("\nPatterns from v_verb_intent_patterns:");
    for (pattern, verb) in &view_patterns {
        println!("  '{}' -> {}", pattern, verb);
    }
}

/// Test resolving "aviva" as a client group
#[tokio::test]
#[ignore] // Run with --ignored flag
async fn test_resolve_aviva() {
    let pool = get_pool().await;
    let phrase = "aviva uk";
    let phrase_norm = phrase.to_lowercase();

    let result = sqlx::query!(
        r#"
        SELECT
            cg.id as "group_id!",
            cg.canonical_name as "group_name!",
            CASE
                WHEN cga.alias_norm = $1 THEN 1.0
                WHEN dmetaphone(cga.alias_norm) = dmetaphone($1) THEN 0.9
                ELSE GREATEST(similarity(cga.alias_norm, $1), 0.4)
            END as "confidence!"
        FROM "ob-poc".client_group_alias cga
        JOIN "ob-poc".client_group cg ON cg.id = cga.group_id
        WHERE cga.alias_norm = $1
           OR similarity(cga.alias_norm, $1) > 0.4
           OR dmetaphone(cga.alias_norm) = dmetaphone($1)
        ORDER BY
            (cga.alias_norm = $1) DESC,
            (dmetaphone(cga.alias_norm) = dmetaphone($1)) DESC,
            similarity(cga.alias_norm, $1) DESC
        LIMIT 1
        "#,
        phrase_norm
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    match result {
        Some(r) => {
            println!(
                "Resolved '{}' to: {} (confidence: {})",
                phrase, r.group_name, r.confidence
            );
        }
        None => {
            println!("Could not resolve '{}' - no matching client group", phrase);

            // Try a simpler similarity search
            let similar = sqlx::query!(
                r#"
                SELECT
                    cga.alias_norm,
                    cg.canonical_name,
                    similarity(cga.alias_norm, $1) as "sim!"
                FROM "ob-poc".client_group_alias cga
                JOIN "ob-poc".client_group cg ON cg.id = cga.group_id
                ORDER BY similarity(cga.alias_norm, $1) DESC
                LIMIT 5
                "#,
                phrase_norm
            )
            .fetch_all(&pool)
            .await
            .unwrap();

            println!("Top similar aliases:");
            for s in similar {
                println!(
                    "  {} -> {} (similarity: {})",
                    s.alias_norm, s.canonical_name, s.sim
                );
            }
        }
    }
}
