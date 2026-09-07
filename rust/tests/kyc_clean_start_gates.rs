//! EOP-DD-UBO-CLEANOUT-001 §5 P4 standing gates (2026-09-07), the two that
//! need a live DB (`registry_hash_count_is_one` is a pure crate-internal
//! test, `src/domain_ops/kyc_stream_ops.rs::clean_start_gates`; the
//! `no_retired_fqn_anywhere` gate is `scripts/check_no_retired_kyc_fqn.sh`).
//!
//! Both gates here are vacuously true today — the stream is empty and no
//! archive table exists — and that is the point: C4 makes the clean-out a
//! standing gate rather than a one-off, so a FUTURE write of an
//! unregistered hash, or a FUTURE archive table, goes red here immediately
//! instead of silently accumulating the way the previous three renames did.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

use ob_poc_kyc_substrate::assembly_lexicon;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

/// C3/§5: every event actually committed to the durable stream must carry a
/// lexicon hash the production registry recognises — `assembly_lexicon().hash`
/// is the only one `KYC_REGISTRY` registers (`registry_hash_count_is_one`
/// proves that half). Before the clean start this failed 707 times; after it,
/// the stream is empty and it is trivially true — and `PgKycEventStore::append`
/// already refuses an unregistered hash at write time (`UnregisteredLexiconHash`,
/// no fallback), so this stays true by construction, not by luck.
#[tokio::test]
async fn fold_registry_has_no_unregistered_hashes_in_the_stream() {
    let pool = pool().await;
    let live_hash = assembly_lexicon().hash.to_hex();

    let rows = sqlx::query(
        r#"SELECT DISTINCT lexicon_hash FROM "ob-poc".kyc_intent_events WHERE lexicon_hash != $1"#,
    )
    .bind(&live_hash)
    .fetch_all(&pool)
    .await
    .expect("query kyc_intent_events");

    let stray: Vec<String> = rows.iter().map(|r| r.get::<String, _>("lexicon_hash")).collect();
    assert!(
        stray.is_empty(),
        "kyc_intent_events carries lexicon_hash value(s) the production registry does not \
         recognise as the live hash: {stray:?} — every committed event must fold under the \
         one registered version (EOP-DD-UBO-CLEANOUT-001 C3)"
    );
}

/// C1/§5: no parallel/archive table for the retired KYC/UBO vocabulary.
/// Structural check over `information_schema.tables`, not a content check —
/// C1's ruling is that an archive surface must not exist at all, not that it
/// must be empty.
#[tokio::test]
async fn no_archive_surfaces() {
    let pool = pool().await;

    let rows = sqlx::query(
        r#"SELECT table_name FROM information_schema.tables
           WHERE table_schema = 'ob-poc'
             AND (table_name ~* 'kyc.*archive'
                  OR table_name ~* 'archive.*kyc'
                  OR table_name ~* 'ubo.*archive'
                  OR table_name ~* 'archive.*ubo'
                  OR table_name ~* 'kyc_intent_events_old'
                  OR table_name ~* 'kyc_subject_streams_old')"#,
    )
    .fetch_all(&pool)
    .await
    .expect("query information_schema.tables");

    let found: Vec<String> = rows.iter().map(|r| r.get::<String, _>("table_name")).collect();
    assert!(
        found.is_empty(),
        "found archive/parallel table(s) for retired KYC/UBO vocabulary, which C1 forbids: \
         {found:?} — clear, do not archive"
    );
}
