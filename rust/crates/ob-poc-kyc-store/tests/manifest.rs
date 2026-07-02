//! W2 lexicon manifest tests (EOP-DD-KYCUBO-001 §8.1).
//!
//! Proves: publish_manifest persists the whole-manifest hash (Q7) and stamps
//! lexicon_hash on every dsl.kyc verb row in dsl_verbs. Idempotent re-publish.
//! K-30 lint: every dsl.kyc verb in dsl_verbs has a non-null lexicon_hash after publish.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use ob_poc_kyc_store::publish_manifest;
use ob_poc_kyc_substrate::phase1_lexicon;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

#[tokio::test]
async fn w2_publish_manifest_stamps_verbs_and_is_idempotent() {
    let pool = pool().await;
    let manifest = phase1_lexicon();

    // Clear any prior state so this test is idempotent across runs.
    sqlx::query(r#"UPDATE "ob-poc".dsl_verbs SET lexicon_hash = NULL WHERE full_name = ANY($1)"#)
        .bind(manifest.entries.keys().cloned().collect::<Vec<_>>())
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(r#"DELETE FROM "ob-poc".kyc_lexicon_manifest WHERE manifest_hash = $1"#)
        .bind(manifest.hash.to_hex())
        .execute(&pool)
        .await
        .unwrap();

    // First publish.
    let mut conn = pool.acquire().await.unwrap();
    let outcome = publish_manifest(&mut conn, Some("w2-test")).await.unwrap();
    assert_eq!(outcome.entry_count, 12, "phase1_lexicon has 12 entries");
    assert!(!outcome.already_existed, "first publish inserts the row");
    assert_eq!(
        outcome.verb_rows_updated, 12,
        "all 12 dsl.kyc verb rows stamped"
    );
    assert_eq!(
        outcome.manifest_hash,
        manifest.hash.to_hex(),
        "hash round-trips"
    );

    // K-30 lint: every dsl.kyc verb now has a non-null lexicon_hash.
    let nulls: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".dsl_verbs
           WHERE full_name = ANY($1) AND lexicon_hash IS NULL"#,
    )
    .bind(manifest.entries.keys().cloned().collect::<Vec<_>>())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        nulls, 0,
        "K-30 lint: every dsl.kyc verb must have lexicon_hash set"
    );

    // Verify the stamped hash matches the substrate entry hash (content-addressed, Q7).
    for (fqn, entry) in &manifest.entries {
        let db_hash: Option<String> = sqlx::query_scalar(
            r#"SELECT lexicon_hash FROM "ob-poc".dsl_verbs WHERE full_name = $1"#,
        )
        .bind(fqn)
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(
            db_hash.as_deref(),
            Some(entry.hash.to_hex().as_str()),
            "dsl_verbs.lexicon_hash for {fqn} must match substrate LexiconEntry.hash (Q7)"
        );
    }

    // Manifest row persisted.
    let row_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_lexicon_manifest WHERE manifest_hash = $1"#,
    )
    .bind(&outcome.manifest_hash)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row_count, 1);

    // Second publish is a no-op (idempotent).
    let mut conn2 = pool.acquire().await.unwrap();
    let outcome2 = publish_manifest(&mut conn2, Some("w2-test")).await.unwrap();
    assert!(
        outcome2.already_existed,
        "second publish with same manifest_hash is a no-op"
    );
    assert_eq!(
        outcome2.manifest_hash, outcome.manifest_hash,
        "hash is stable (Q7)"
    );
}
