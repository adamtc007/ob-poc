//! W2 lexicon manifest tests (EOP-DD-KYCUBO-001 §8.1), extended TS.6 P1 for
//! the two-pack split (EOP-DD-KYCUBO-TS.6 §1/§8).
//!
//! Proves, in one sequenced test (`w2_manifest_publishing` — see its own
//! comment for why this isn't three separate `#[tokio::test]` fns):
//! `publish_assembly_manifest`/`publish_evaluation_manifest` each persist
//! their own whole-manifest hash (Q7) and stamp `lexicon_hash` on every verb
//! row in `dsl_verbs`. Idempotent re-publish. K-30 lint: every dsl.kyc verb
//! in `dsl_verbs` has a non-null `lexicon_hash` after publish.
//! `packs_have_independent_hashes` (TS.6 §8): the two packs' hashes are
//! distinct and neither publish disturbs the other pack's stamped rows.

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use ob_poc_kyc_store::{publish_assembly_manifest, publish_evaluation_manifest, ManifestPublishOutcome};
use ob_poc_kyc_substrate::{assembly_lexicon, evaluation_lexicon, LexiconManifest};

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

async fn clear_prior_state(pool: &PgPool, manifest: &LexiconManifest) {
    sqlx::query(r#"UPDATE "ob-poc".dsl_verbs SET lexicon_hash = NULL WHERE full_name = ANY($1)"#)
        .bind(manifest.entries.keys().cloned().collect::<Vec<_>>())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(r#"DELETE FROM "ob-poc".kyc_lexicon_manifest WHERE manifest_hash = $1"#)
        .bind(manifest.hash.to_hex())
        .execute(pool)
        .await
        .unwrap();
}

/// Publish `manifest` via `publish` and prove: full coverage, idempotent
/// re-publish, K-30 lint, and content-addressed hash round-trip (Q7).
/// Shared by both packs — the only difference between Assembly and
/// Evaluation here is which manifest/publish fn is passed in.
async fn assert_publish_roundtrip<F, Fut>(pool: &PgPool, manifest: &LexiconManifest, publish: F)
where
    F: Fn(sqlx::pool::PoolConnection<sqlx::Postgres>) -> Fut,
    Fut: std::future::Future<Output = ManifestPublishOutcome>,
{
    let expected_entries = manifest.entries.len();

    let conn = pool.acquire().await.unwrap();
    let outcome = publish(conn).await;
    assert_eq!(
        outcome.entry_count, expected_entries,
        "publish covers the whole lexicon"
    );
    assert!(!outcome.already_existed, "first publish inserts the row");
    assert_eq!(
        outcome.verb_rows_updated, expected_entries as u64,
        "all verb rows stamped"
    );
    assert_eq!(outcome.manifest_hash, manifest.hash.to_hex(), "hash round-trips");

    // K-30 lint: every verb now has a non-null lexicon_hash.
    let nulls: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".dsl_verbs
           WHERE full_name = ANY($1) AND lexicon_hash IS NULL"#,
    )
    .bind(manifest.entries.keys().cloned().collect::<Vec<_>>())
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(nulls, 0, "K-30 lint: every verb must have lexicon_hash set");

    // Verify the stamped hash matches the substrate entry hash (content-addressed, Q7).
    for (fqn, entry) in &manifest.entries {
        let db_hash: Option<String> = sqlx::query_scalar(
            r#"SELECT lexicon_hash FROM "ob-poc".dsl_verbs WHERE full_name = $1"#,
        )
        .bind(fqn)
        .fetch_optional(pool)
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
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(row_count, 1);

    // Second publish is a no-op (idempotent).
    let conn2 = pool.acquire().await.unwrap();
    let outcome2 = publish(conn2).await;
    assert!(outcome2.already_existed, "second publish with same manifest_hash is a no-op");
    assert_eq!(outcome2.manifest_hash, outcome.manifest_hash, "hash is stable (Q7)");
}

// The three checks below are ONE `#[tokio::test]` rather than three,
// deliberately: they all mutate the SAME global tables (`dsl_verbs.
// lexicon_hash`, `kyc_lexicon_manifest`) with no per-test isolation key
// (unlike every other KYC test in this workspace, which scopes by a fresh
// random `subject_root` UUID) — `dsl_verbs`/`kyc_lexicon_manifest` are
// genuinely global, not subject-scoped, so running these as separate
// `#[tokio::test]` fns races under cargo's default parallel-within-binary
// execution (confirmed: splitting them produced flaky cross-test
// interference on `lexicon_hash IS NULL` counts). Sequencing them inside
// one test function is the correct fix, not a `--test-threads=1` workaround.

#[tokio::test]
async fn w2_manifest_publishing() {
    let pool = pool().await;

    // ── Assembly pack ────────────────────────────────────────────────────
    let assembly = assembly_lexicon();
    // Universe pin: 14 `assembly_lexicon()` entries (15 post-P1/P2, minus
    // `type-correction` DISSOLVED, T2 2026-08-27 §8 Q1 P3; 16 post-D2.0,
    // minus `register`+`type` MERGED into `place`; 19 post-TS.6-P1, minus
    // `creation`/`satisfaction` DISSOLVED and `waiver` MOVED to
    // evaluation_lexicon(), D2.0 §5, 2026-08-22). This
    // counts `LexiconEntry::build()` calls in lexicon.rs — NOT the same
    // number as `declared_verb_universe()` (17, YAML-scanned across BOTH
    // packs, `kyc_pack_closure.rs`): lexicon coverage of the declared verb
    // set is intentionally partial (K-G6), so the two counts are expected
    // to differ, not drift-checked against each other. Bump consciously
    // alongside `kyc_pack_closure.rs`'s own lexicon-entry-count assertion.
    assert_eq!(assembly.entries.len(), 14, "assembly_lexicon universe (see kyc_pack_closure)");
    clear_prior_state(&pool, &assembly).await;
    assert_publish_roundtrip(&pool, &assembly, |mut conn| async move {
        publish_assembly_manifest(&mut conn, Some("w2-test")).await.unwrap()
    })
    .await;

    // ── Evaluation pack ──────────────────────────────────────────────────
    let evaluation = evaluation_lexicon();
    // Universe pin: 3 `evaluation_lexicon()` entries (kyc_ubo.decide.subject.approve,
    // kyc_ubo.decide.subject.reject — TS.6 P1/P2 — plus kyc_ubo.decide.obligation.waiver,
    // moved here from assembly_lexicon() D2.0 §5, 2026-08-22).
    assert_eq!(evaluation.entries.len(), 3, "evaluation_lexicon universe (see kyc_pack_closure)");
    clear_prior_state(&pool, &evaluation).await;
    assert_publish_roundtrip(&pool, &evaluation, |mut conn| async move {
        publish_evaluation_manifest(&mut conn, Some("w2-test")).await.unwrap()
    })
    .await;

    // ── TS.6 §8 packs_have_independent_hashes ───────────────────────────
    // A change to one pack's entries does not move the other's manifest
    // hash — the two-clocks property, made testable. Proven structurally
    // (the hashes are computed from disjoint entry sets, `LexiconManifest::
    // new` hashes only its own `entries`) and then confirmed live: starting
    // from both packs freshly cleared, publishing ONLY assembly must leave
    // evaluation's rows and manifest row completely untouched.
    assert_ne!(
        assembly.hash.to_hex(),
        evaluation.hash.to_hex(),
        "the two packs must never share a manifest hash"
    );

    // No FQN is a member of both packs — a verb belongs to exactly one
    // capability (TS.6 §1).
    for fqn in evaluation.entries.keys() {
        assert!(
            assembly.get(fqn).is_none(),
            "{fqn} must not appear in assembly_lexicon() (it is an evaluation_lexicon() verb)"
        );
    }
    for fqn in assembly.entries.keys() {
        assert!(
            evaluation.get(fqn).is_none(),
            "{fqn} must not appear in evaluation_lexicon() (it is an assembly_lexicon() verb)"
        );
    }

    // Publishing one pack does not touch the other's dsl_verbs.lexicon_hash
    // rows or kyc_lexicon_manifest row (live-DB proof, not just the
    // in-memory disjointness above). Both packs were just published above
    // by the roundtrip checks — clear both back to a known-fresh state
    // first so this section's assertions aren't reading stale success from
    // the earlier publishes.
    clear_prior_state(&pool, &assembly).await;
    clear_prior_state(&pool, &evaluation).await;

    let mut conn = pool.acquire().await.unwrap();
    publish_assembly_manifest(&mut conn, Some("independent-hash-test"))
        .await
        .unwrap();

    // Evaluation's manifest row must NOT exist yet — only assembly was published.
    let eval_row_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_lexicon_manifest WHERE manifest_hash = $1"#,
    )
    .bind(evaluation.hash.to_hex())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(eval_row_count, 0, "publishing assembly must not publish evaluation");

    // Evaluation's own verb rows must still be unstamped (lexicon_hash NULL).
    let eval_nulls: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".dsl_verbs
           WHERE full_name = ANY($1) AND lexicon_hash IS NULL"#,
    )
    .bind(evaluation.entries.keys().cloned().collect::<Vec<_>>())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        eval_nulls,
        evaluation.entries.len() as i64,
        "publishing assembly must not stamp evaluation's verb rows"
    );
}
