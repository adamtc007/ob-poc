//! T4.5 (`EOP-PLAN-KYCUBO-KIT-001` §T4.5) — RED-first gates for the KYC
//! super-user workbook REPL surface + exact-match entity resolver.
//!
//! Internal integration tests (crate-internal per the repo's test-boundary
//! rule): drives the REAL `ReplOrchestratorV2::process()` end to end for the
//! REPL-level gates, and the resolver module directly (against a real DB
//! connection) for the resolver gates. Requires `DATABASE_URL`; `#[ignore]`d
//! like every other DB-backed test in this directory.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use uuid::Uuid;

use ob_poc_kyc_seam::append_in_scope;
use ob_poc_kyc_store::PgKycEventStore;
use ob_poc_kyc_substrate::{
    assembly_lexicon, preview, AuthorityRef, EdgeId, FoldRegistry, IntentEvent,
    Principal as SubstratePrincipal, SubjectId, TargetBinding, V1FoldImpl,
};

use crate::journey::router::PackRouter;
use crate::repl::kyc_entity_resolver::{resolve_handles, ResolverError};
use crate::repl::kyc_workbook_surface::{parse_kyc_workbook_command, KycWorkbookCommand};
use crate::repl::response_v2::ReplResponseKindV2;
use crate::repl::types_v2::UserInputV2;
use crate::sequencer::{NullDslExecutor, ReplOrchestratorV2};

// ── Shared scaffolding ──────────────────────────────────────────────────────

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn connect() -> PgPool {
    PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

fn orchestrator(pool: PgPool) -> ReplOrchestratorV2 {
    ReplOrchestratorV2::new(PackRouter::new(vec![]), Arc::new(NullDslExecutor)).with_pool(pool)
}

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    r
}

async fn cleanup_subject(pool: &PgPool, subject: SubjectId) {
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_subject_streams WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
}

async fn cleanup_entities(pool: &PgPool, ids: &[Uuid]) {
    for id in ids {
        let _ = sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#)
            .bind(id)
            .execute(pool)
            .await;
    }
}

async fn some_entity_type_id(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(r#"SELECT entity_type_id FROM "ob-poc".entity_types LIMIT 1"#)
        .fetch_one(pool)
        .await
        .expect("at least one entity_type row must exist for this fixture")
}

/// Two DISTINCT entity_type_ids — `entities` has a `(entity_type_id, name)`
/// UNIQUE constraint, so genuine same-name ambiguity requires two different
/// types (e.g. the same legal name registered once as a natural person and
/// once as a legal entity — a realistic real-world collision, not a fixture
/// artifact).
async fn two_entity_type_ids(pool: &PgPool) -> (Uuid, Uuid) {
    let rows: Vec<Uuid> = sqlx::query_scalar(r#"SELECT entity_type_id FROM "ob-poc".entity_types LIMIT 2"#)
        .fetch_all(pool)
        .await
        .expect("query entity_types");
    assert!(rows.len() >= 2, "fixture requires at least 2 distinct entity_type rows");
    (rows[0], rows[1])
}

async fn insert_entity(pool: &PgPool, entity_id: Uuid, entity_type_id: Uuid, name: &str) {
    sqlx::query(
        r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name) VALUES ($1, $2, $3)"#,
    )
    .bind(entity_id)
    .bind(entity_type_id)
    .bind(name)
    .execute(pool)
    .await
    .expect("insert fixture entity");
}

// ── resolver gates ──────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn resolver_resolves_exact() {
    let pool = connect().await;
    let type_id = some_entity_type_id(&pool).await;
    let entity_id = Uuid::new_v4();
    let name = format!("T45 Resolver Exact {}", Uuid::new_v4());
    insert_entity(&pool, entity_id, type_id, &name).await;

    let mut conn = pool.acquire().await.unwrap();
    let text = format!(r#"(kyc_ubo.assert.subject.register :subject-id "@{name}")"#);
    let resolved = resolve_handles(&mut conn, &text).await.expect("exact match must resolve");
    assert!(
        resolved.contains(&entity_id.to_string()),
        "resolved text must carry the UUID literal, got: {resolved}"
    );
    assert!(!resolved.contains(&name), "resolved text must not carry the raw handle text");

    cleanup_entities(&pool, &[entity_id]).await;
}

#[tokio::test]
#[ignore]
async fn resolver_rejects_unknown() {
    let pool = connect().await;
    let mut conn = pool.acquire().await.unwrap();
    let text = r#"(kyc_ubo.assert.subject.register :subject-id "@DefinitelyNotARealEntityHandle12345")"#;
    let err = resolve_handles(&mut conn, text).await.expect_err("unknown handle must error");
    assert!(matches!(err, ResolverError::Unknown { .. }), "got: {err:?}");
}

#[tokio::test]
#[ignore]
async fn resolver_rejects_ambiguous() {
    let pool = connect().await;
    let (type_a, type_b) = two_entity_type_ids(&pool).await;
    let shared_name = format!("T45 Ambiguous {}", Uuid::new_v4());
    let (id_a, id_b) = (Uuid::new_v4(), Uuid::new_v4());
    insert_entity(&pool, id_a, type_a, &shared_name).await;
    insert_entity(&pool, id_b, type_b, &shared_name).await;

    let mut conn = pool.acquire().await.unwrap();
    let text = format!(r#"(kyc_ubo.assert.subject.register :subject-id "@{shared_name}")"#);
    let err = resolve_handles(&mut conn, &text).await.expect_err("ambiguous handle must error");
    match err {
        ResolverError::Ambiguous { candidates, .. } => {
            assert_eq!(candidates.len(), 2, "must name every candidate");
            assert!(candidates.contains(&id_a));
            assert!(candidates.contains(&id_b));
        }
        other => panic!("expected Ambiguous, got: {other:?}"),
    }

    cleanup_entities(&pool, &[id_a, id_b]).await;
}

// ── REPL surface end-to-end gates ───────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn surface_session_end_to_end() {
    let pool = connect().await;
    let type_id = some_entity_type_id(&pool).await;

    // The subject IS an entity row here (a KYC subject is conceptually an
    // entity) — the fixture that lets `subject-id` (one of the five
    // target-binding slots) be resolved BY HANDLE rather than typed as a
    // raw UUID, proving the resolver runs before recognition end-to-end
    // through the real REPL command surface.
    let subject_uuid = Uuid::new_v4();
    let subject = SubjectId(subject_uuid);
    let handle_name = format!("T45 E2E Subject {subject_uuid}");
    insert_entity(&pool, subject_uuid, type_id, &handle_name).await;

    let orch = orchestrator(pool.clone());
    let session_id = orch.create_session().await;

    let edge = Uuid::new_v4();
    let (from, to) = (Uuid::new_v4(), Uuid::new_v4());
    let doc_id = Uuid::new_v4();

    let open_resp = orch
        .process(
            session_id,
            UserInputV2::Message { content: format!("kyc-workbook.open {subject_uuid}") },
        )
        .await
        .expect("open must succeed");
    assert_info_contains(&open_resp.kind, "opened workbook");

    // Move 0: T6.2 (2026-08-12) — `assert-control` now carries
    // `SubjectRegistered` (matrix row 1); register first, same fix as the
    // T4 gate's `session_roundtrip` fixture and `run_prefix_state_
    // matches_intermediate_preview` above. Also staged BY HANDLE, so the
    // resolver is exercised on this move too.
    orch.process(
        session_id,
        UserInputV2::Message {
            content: format!(r#"kyc-workbook.stage (kyc_ubo.assert.subject.register :subject-id "@{handle_name}")"#),
        },
    )
    .await
    .expect("move 0 (register, by handle) must succeed");

    // Move 1, staged BY HANDLE (not raw UUID) — the resolver must rewrite
    // `@<handle>` to the subject's UUID before `KycWorkbook::stage` ever
    // sees the text.
    let stage1 = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: format!(
                    r#"kyc-workbook.stage (kyc_ubo.assert.edge.control :subject-id "@{handle_name}" :edge_id "{edge}" :from_entity_id "{from}" :to_entity_id "{to}" :kind "voting_rights")"#
                ),
            },
        )
        .await
        .expect("stage 1 must succeed");
    let stage1_msg = expect_info(&stage1.kind);
    assert!(
        stage1_msg.contains(&subject_uuid.to_string()),
        "canonical staged text must carry the resolved UUID: {stage1_msg}"
    );
    assert!(!stage1_msg.contains(&handle_name), "canonical staged text must not carry the handle");

    // Move 2: dependent chain, frontier recognition (mirrors the T4 gate's
    // own `session_roundtrip` fixture — a committed-only frontier would
    // wrongly refuse this).
    orch.process(
        session_id,
        UserInputV2::Message {
            content: format!(r#"kyc-workbook.stage (kyc_ubo.assert.edge.evidence :edge-id "{edge}" :doc_id "{doc_id}")"#),
        },
    )
    .await
    .expect("stage 2 must succeed");

    let validate_resp = orch
        .process(session_id, UserInputV2::Message { content: "kyc-workbook.validate".to_string() })
        .await
        .expect("validate must succeed");
    let validate_msg = expect_info(&validate_resp.kind);
    assert!(validate_msg.contains("consequence preview"), "got: {validate_msg}");

    let commit_resp = orch
        .process(session_id, UserInputV2::Message { content: "kyc-workbook.commit".to_string() })
        .await
        .expect("commit must succeed");
    let commit_msg = expect_info(&commit_resp.kind);
    assert!(commit_msg.contains("committed 3 move"), "got: {commit_msg}");

    // Re-open must reproduce the committed state.
    let reopen_resp = orch
        .process(
            session_id,
            UserInputV2::Message { content: format!("kyc-workbook.open {subject_uuid}") },
        )
        .await
        .expect("reopen must succeed");
    let reopen_msg = expect_info(&reopen_resp.kind);
    assert!(reopen_msg.contains("3 committed"), "got: {reopen_msg}");

    let count: i64 =
        sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
            .bind(subject.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 3);

    cleanup_subject(&pool, subject).await;
    cleanup_entities(&pool, &[subject_uuid]).await;
}

#[tokio::test]
#[ignore]
async fn surface_rejection_shows_placement_listing() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let orch = orchestrator(pool.clone());
    let session_id = orch.create_session().await;

    orch.process(session_id, UserInputV2::Message { content: format!("kyc-workbook.open {}", subject.0) })
        .await
        .unwrap();

    // `verify` with nothing asserted yet is illegal against the empty
    // frontier — must be REJECTED with the currently-legal placement
    // listing, never a guess.
    let resp = orch
        .process(
            session_id,
            UserInputV2::Message {
                content: format!(r#"kyc-workbook.stage (kyc_ubo.assert.edge.verification :edge-id "{}")"#, Uuid::new_v4()),
            },
        )
        .await
        .expect("stage call itself must not error at the transport level");
    let msg = expect_info(&resp.kind);
    assert!(msg.contains("REJECTED"), "got: {msg}");
    assert!(msg.contains("currently-legal moves"), "got: {msg}");

    cleanup_subject(&pool, subject).await;
}

#[tokio::test]
#[ignore]
async fn run_stops_at_first_failure_prefix_stands() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let lexicon = assembly_lexicon();
    let as_of = fixed_ts();
    let edge1 = Uuid::new_v4();

    // Pre-existing committed history: a registered subject (T6.2 —
    // `assert-control` now carries `SubjectRegistered`, matrix row 1) with
    // an evidenced edge (edge1).
    {
        let mut scope = crate::sequencer_tx::PgTransactionScope::begin(&pool).await.unwrap();
        let register_event = IntentEvent::new(
            subject,
            "kyc_ubo.assert.subject.register",
            SubstratePrincipal::test_analyst(),
            AuthorityRef("setup.register".into()),
            TargetBinding::for_subject(subject),
            serde_json::json!({ "entity_id": subject.0 }),
            as_of,
        )
        .with_lexicon_hash(lexicon.hash);
        append_in_scope(&mut scope, &registry, &register_event, "(setup-register)", |_, _, _| Ok(())).await.unwrap();

        let assert_event = IntentEvent::new(
            subject,
            "kyc_ubo.assert.edge.control",
            SubstratePrincipal::test_analyst(),
            AuthorityRef("setup.assert-control".into()),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge1, "from_entity_id": Uuid::new_v4(), "to_entity_id": Uuid::new_v4(), "kind": "voting_rights"}),
            as_of,
        )
        .with_lexicon_hash(lexicon.hash);
        append_in_scope(&mut scope, &registry, &assert_event, "(setup-assert)", |_, _, _| Ok(())).await.unwrap();

        let evidence_event = IntentEvent::new(
            subject,
            "kyc_ubo.assert.edge.evidence",
            SubstratePrincipal::test_analyst(),
            AuthorityRef("setup.attach-evidence".into()),
            TargetBinding::for_edge(subject, EdgeId(edge1)),
            serde_json::json!({"doc_id": Uuid::new_v4()}),
            as_of,
        )
        .with_lexicon_hash(lexicon.hash);
        append_in_scope(&mut scope, &registry, &evidence_event, "(setup-evidence)", |_, _, _| Ok(())).await.unwrap();
        scope.commit().await.unwrap();
    }

    let orch = orchestrator(pool.clone());
    let session_id = orch.create_session().await;
    orch.process(session_id, UserInputV2::Message { content: format!("kyc-workbook.open {}", subject.0) })
        .await
        .unwrap();

    let edge2 = Uuid::new_v4();
    let (from2, to2) = (Uuid::new_v4(), Uuid::new_v4());

    // Move A: independent, no precondition — WILL land.
    orch.process(
        session_id,
        UserInputV2::Message {
            content: format!(
                r#"kyc-workbook.stage (kyc_ubo.assert.edge.control :edge_id "{edge2}" :from_entity_id "{from2}" :to_entity_id "{to2}" :kind "voting_rights")"#
            ),
        },
    )
    .await
    .unwrap();

    // Move B: `verify(edge1)` — legal against the CURRENT frontier
    // (edge1 is Evidenced in committed history) at stage time.
    orch.process(
        session_id,
        UserInputV2::Message {
            content: format!(r#"kyc-workbook.stage (kyc_ubo.assert.edge.verification :edge-id "{edge1}")"#),
        },
    )
    .await
    .unwrap();

    // Concurrent connection supersedes edge1 mid-session — move B's
    // precondition will go stale between stage time and run time.
    {
        let mut scope2 = crate::sequencer_tx::PgTransactionScope::begin(&pool).await.unwrap();
        let supersede_event = IntentEvent::new(
            subject,
            "kyc_ubo.assert.edge.supersession",
            SubstratePrincipal::test_analyst(),
            AuthorityRef("concurrent.supersede".into()),
            TargetBinding::for_edge(subject, EdgeId(edge1)),
            serde_json::json!({}),
            as_of,
        )
        .with_lexicon_hash(lexicon.hash);
        append_in_scope(&mut scope2, &registry, &supersede_event, "(concurrent-supersede)", |_, _, _| Ok(()))
            .await
            .unwrap();
        scope2.commit().await.unwrap();
    }

    let run_resp = orch
        .process(session_id, UserInputV2::Message { content: "kyc-workbook.run".to_string() })
        .await
        .expect("run dispatch itself must not error at the transport level");
    let run_msg = expect_info(&run_resp.kind);
    assert!(run_msg.contains("LANDED kyc_ubo.assert.edge.control"), "got: {run_msg}");
    assert!(run_msg.contains("FAILED kyc_ubo.assert.edge.verification"), "got: {run_msg}");
    assert!(run_msg.contains("1 move(s) remain staged"), "got: {run_msg}");

    // Prefix stands: move A's append is a real, separate, already-committed
    // transaction — DB shows exactly 3 setup/A rows (2 setup + 1 concurrent
    // supersede + move A = 4; move B never landed).
    let count: i64 =
        sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
            .bind(subject.0)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 5, "3 setup (register+assert+evidence) + 1 concurrent supersede + move A; move B must NOT have landed");

    cleanup_subject(&pool, subject).await;
}

#[tokio::test]
#[ignore]
async fn run_prefix_state_matches_intermediate_preview() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let orch = orchestrator(pool.clone());
    let session_id = orch.create_session().await;
    orch.process(session_id, UserInputV2::Message { content: format!("kyc-workbook.open {}", subject.0) })
        .await
        .unwrap();

    let edge = Uuid::new_v4();
    let (from, to) = (Uuid::new_v4(), Uuid::new_v4());

    // T6.2 (2026-08-12): `assert-control` now carries `SubjectRegistered`
    // (matrix row 1) — register first, same fix as the T4 gate's
    // `session_roundtrip` fixture.
    orch.process(
        session_id,
        UserInputV2::Message { content: "kyc-workbook.stage (kyc_ubo.assert.subject.register)".to_string() },
    )
    .await
    .unwrap();

    orch.process(
        session_id,
        UserInputV2::Message {
            content: format!(
                r#"kyc-workbook.stage (kyc_ubo.assert.edge.control :edge_id "{edge}" :from_entity_id "{from}" :to_entity_id "{to}" :kind "voting_rights")"#
            ),
        },
    )
    .await
    .unwrap();

    // Intermediate preview: fold(committed=[] ++ [register, the staged
    // assert-control move]) — computed independently via the substrate's
    // own `preview`, exactly what staging-time validate() would have shown
    // after these two moves.
    let lexicon = assembly_lexicon();
    let register_event = IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.register",
        SubstratePrincipal::test_analyst(),
        AuthorityRef("preview-only".into()),
        TargetBinding::for_subject(subject),
        serde_json::json!({ "entity_id": subject.0 }),
        fixed_ts(),
    )
    .with_lexicon_hash(lexicon.hash);
    let expected_event = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.control",
        SubstratePrincipal::test_analyst(),
        AuthorityRef("preview-only".into()),
        TargetBinding::for_subject(subject),
        serde_json::json!({"edge_id": edge, "from_entity_id": from, "to_entity_id": to, "kind": "voting_rights"}),
        fixed_ts(),
    )
    .with_lexicon_hash(lexicon.hash);
    let (expected_control, _, _) =
        preview(&[], &[register_event, expected_event], &lexicon).unwrap();
    let expected_status = expected_control.edges.get(&EdgeId(edge)).map(|e| e.status);

    let run_resp = orch
        .process(session_id, UserInputV2::Message { content: "kyc-workbook.run".to_string() })
        .await
        .expect("run must succeed");
    let run_msg = expect_info(&run_resp.kind);
    assert!(run_msg.contains("LANDED kyc_ubo.assert.subject.register"), "got: {run_msg}");
    assert!(run_msg.contains("LANDED kyc_ubo.assert.edge.control"), "got: {run_msg}");

    let mut conn = pool.acquire().await.unwrap();
    let committed = PgKycEventStore::load_events(&mut conn, subject).await.unwrap();
    let (post_run_control, _, _) = preview(&committed, &[], &lexicon).unwrap();
    let post_run_status = post_run_control.edges.get(&EdgeId(edge)).map(|e| e.status);

    assert_eq!(
        post_run_status, expected_status,
        "the landed prefix's DB state must match the intermediate preview computed at stage time"
    );

    cleanup_subject(&pool, subject).await;
}

fn fixed_ts() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&chrono::Utc)
}

fn expect_info(kind: &ReplResponseKindV2) -> &str {
    match kind {
        ReplResponseKindV2::Info { detail } => detail.as_str(),
        other => panic!("expected ReplResponseKindV2::Info, got: {other:?}"),
    }
}

fn assert_info_contains(kind: &ReplResponseKindV2, needle: &str) {
    let detail = expect_info(kind);
    assert!(detail.contains(needle), "expected {detail:?} to contain {needle:?}");
}

// ── command parsing sanity (not itself a P9 gate — belongs with the module,
// kept here since `parse_kyc_workbook_command`/`KycWorkbookCommand` are only
// `pub(crate)` and this is the natural internal-test home) ─────────────────

#[test]
fn parses_run_distinct_from_repl_command_v2_run() {
    // `kyc-workbook.run` must parse to OUR `KycWorkbookCommand::Run`, never
    // be confused with `ReplCommandV2::Run` ("execute the runbook") — the
    // two are different enums entirely, checked here as a standing
    // reminder/regression guard.
    assert_eq!(parse_kyc_workbook_command("kyc-workbook.run"), Some(Ok(KycWorkbookCommand::Run)));
}
