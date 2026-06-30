//! W2 — lexicon manifest publishing (EOP-DD-KYCUBO-001 §8.1).
//!
//! Publishes the current `phase1_lexicon()` to:
//!  1. `kyc_lexicon_manifest` — one row per whole-lexicon version (Q7, content-addressed).
//!  2. `dsl_verbs.lexicon_hash` — the per-verb content-address for each dsl.kyc verb.
//!
//! Idempotent: republishing an unchanged lexicon is a no-op (UNIQUE on manifest_hash).

use sqlx::PgConnection;

use ob_poc_kyc_substrate::{phase1_lexicon, LexiconManifest};

use crate::error::StoreError;

/// Outcome of a manifest publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestPublishOutcome {
    pub manifest_hash: String,
    pub entry_count: usize,
    /// True when this manifest hash was already present — no row was inserted.
    pub already_existed: bool,
    /// Number of `dsl_verbs` rows whose `lexicon_hash` was updated.
    pub verb_rows_updated: u64,
}

/// Publish the current `phase1_lexicon()` into the DB.
///
/// Runs in the caller's connection (not necessarily a transaction — the
/// INSERT is idempotent and the UPDATE is convergent, so partial execution
/// is safe to retry).
pub async fn publish_manifest(
    conn: &mut PgConnection,
    published_by: Option<&str>,
) -> Result<ManifestPublishOutcome, StoreError> {
    let manifest = phase1_lexicon();
    publish_manifest_inner(conn, &manifest, published_by).await
}

/// Inner implementation (takes any manifest — testable without hitting the real lexicon).
pub(crate) async fn publish_manifest_inner(
    conn: &mut PgConnection,
    manifest: &LexiconManifest,
    published_by: Option<&str>,
) -> Result<ManifestPublishOutcome, StoreError> {
    let manifest_hash = manifest.hash.to_hex();
    let entry_count = manifest.entries.len();

    // Build the entry_hashes JSON: { fqn -> entry_hash_hex }
    let entry_hashes: serde_json::Value = serde_json::Value::Object(
        manifest
            .entries
            .iter()
            .map(|(fqn, entry)| (fqn.clone(), serde_json::Value::String(entry.hash.to_hex())))
            .collect(),
    );

    // 1. Publish (idempotent: ON CONFLICT DO NOTHING on UNIQUE manifest_hash).
    let inserted = sqlx::query_scalar::<_, i64>(
        r#"WITH ins AS (
             INSERT INTO "ob-poc".kyc_lexicon_manifest
               (manifest_hash, entry_count, entry_hashes, published_by)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (manifest_hash) DO NOTHING
             RETURNING 1
           )
           SELECT count(*) FROM ins"#,
    )
    .bind(&manifest_hash)
    .bind(entry_count as i32)
    .bind(&entry_hashes)
    .bind(published_by)
    .fetch_one(&mut *conn)
    .await?;

    // 2. Stamp dsl_verbs.lexicon_hash for each dsl.kyc verb.
    //    UPDATE is convergent — re-running over unchanged data is a no-op per row.
    let mut total_updated: u64 = 0;
    for (fqn, entry) in &manifest.entries {
        let entry_hash = entry.hash.to_hex();
        let res = sqlx::query(
            r#"UPDATE "ob-poc".dsl_verbs SET lexicon_hash = $1 WHERE full_name = $2"#,
        )
        .bind(&entry_hash)
        .bind(fqn)
        .execute(&mut *conn)
        .await?;
        total_updated += res.rows_affected();
    }

    Ok(ManifestPublishOutcome {
        manifest_hash,
        entry_count,
        already_existed: inserted == 0,
        verb_rows_updated: total_updated,
    })
}
