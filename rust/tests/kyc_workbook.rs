//! T4 gate tests — `EOP-PLAN-KYCUBO-KIT-001` §T4 (closes KIT-10/11).
//!
//! Five gates per the design doc's §8 (v0.2): `session_roundtrip` (dependent
//! chain, frontier recognition, canonical source_text), `new_ubo_from_baseplate`
//! (empty-history path needs no special-casing), `invalid_workbook_blocks_commit`
//! (both `validate()` and `commit()` independently reject, zero rows land),
//! `zero_inference_assertion` (import allowlist over `kyc_workbook.rs`, fails
//! closed on anything new), `stale_snapshot_recovers` (two real connections,
//! a genuine mid-session interleave — the fail-fast re-check must catch a
//! precondition that went stale between stage-time and commit-time).

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc::domain_ops::kyc_workbook::{open_workbook, StagedMove};
use ob_poc_kyc_seam::{append_in_scope, canonical_event_shape};
use ob_poc_kyc_substrate::{
    assembly_lexicon, enumerate_placement_set, render_intent_event_to_sexpr, AuthorityRef, EdgeId,
    EntityId, FoldRegistry, IntentEvent, LexiconManifest, MoveId, Principal, SubjectId,
    TargetBinding, V1FoldImpl,
};
use ob_poc_types::TransactionScopeId;
use sem_os_core::principal::Principal as RuntimePrincipal;

// ── Shared test scaffolding (mirrors ob-poc-kyc-seam/tests/seam.rs) ────────────

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

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    r
}

fn runtime_principal(actor_id: &str) -> RuntimePrincipal {
    RuntimePrincipal {
        actor_id: actor_id.to_string(),
        roles: vec!["analyst".to_string()],
        claims: std::collections::HashMap::new(),
        tenancy: None,
    }
}

fn fixed_ts() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

struct TestScope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
}
impl TestScope {
    async fn begin(pool: &PgPool) -> Self {
        Self {
            tx: pool.begin().await.unwrap(),
            pool: pool.clone(),
            id: TransactionScopeId::new(),
        }
    }
    async fn commit(self) {
        self.tx.commit().await.unwrap();
    }
    async fn rollback(self) {
        self.tx.rollback().await.unwrap();
    }
}
impl TransactionScope for TestScope {
    fn scope_id(&self) -> TransactionScopeId {
        self.id
    }
    fn transaction(&mut self) -> &mut Transaction<'static, Postgres> {
        &mut self.tx
    }
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}

async fn count(pool: &PgPool, subject: SubjectId) -> i64 {
    sqlx::query_scalar(r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
        .bind(subject.0)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn cleanup(pool: &PgPool, subject: SubjectId) {
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_subject_streams WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
}

/// Build a `StagedMove` directly (bypassing `KycWorkbook::stage`'s own
/// frontier-legality gate) — the only way to get an illegal move into
/// `workbook.staged` at all, since `stage()` structurally cannot admit one.
/// Mirrors what a workbook that's gone stale between stage-time and
/// commit-time looks like from the outside.
fn manual_staged_move(
    subject: SubjectId,
    verb_fqn: &str,
    target: TargetBinding,
    payload: serde_json::Value,
    as_of: chrono::DateTime<chrono::Utc>,
    lexicon: &LexiconManifest,
) -> StagedMove {
    let event = IntentEvent::new(
        subject,
        verb_fqn,
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        target,
        payload,
        as_of,
    )
    .with_lexicon_hash(lexicon.hash);
    let source_text = render_intent_event_to_sexpr(&event, lexicon.get(verb_fqn));
    StagedMove {
        event,
        source_text,
        legal_move: MoveId(format!("{verb_fqn}::manual-test-fixture")),
    }
}

// ── session_roundtrip ────────────────────────────────────────────────────────

#[tokio::test]
async fn session_roundtrip() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let principal = runtime_principal("analyst-1");
    let as_of = fixed_ts();
    let lexicon = assembly_lexicon();

    let edge = Uuid::new_v4();
    let (from, to) = (Uuid::new_v4(), Uuid::new_v4());
    let doc_id = Uuid::new_v4();

    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);
    assert!(workbook.committed.is_empty());

    // T6.2 (2026-08-12): `kyc_ubo.assert.edge.control` now carries
    // `SubjectRegistered` (matrix row 1) — this fixture predates that stud
    // and staged assert-control straight against an unregistered subject,
    // which now correctly fails frontier recognition. Register first, same
    // as `new_ubo_from_baseplate` does; the dependent-chain intent of this
    // test (assert-control -> attach-evidence -> verify) is unchanged, it
    // now has a realistic 4th predecessor move instead of 3.
    workbook
        .stage(
            "(kyc_ubo.assert.subject.register)",
            &principal,
            AuthorityRef("analyst.register".into()),
            as_of,
        )
        .expect("move 0 (register) must recognise against the empty-committed frontier");

    // GENUINE T6.2 FINDING (2026-08-12, reported per the plan's own B3
    // instruction, not silently worked around): `KycWorkbook::stage()`
    // recognises a candidate's legality by checking membership of
    // `(verb_fqn, target)` in `enumerate_placement_set`'s probed moves list
    // (`kyc_workbook.rs::stage`) — and that probe uses a generic/`Null`
    // payload (`placement.rs::probe_event`), never the caller's real
    // payload. Every precondition variant that existed before T6.2 is
    // state-only (reads `ControlState`/`ObligationState`, never
    // `event.payload`), so the probe was always representative. T6.2's
    // `NoDuplicateActiveEdge` (row 1/2) is the FIRST payload-reading
    // precondition (`from_entity_id`/`to_entity_id` off the event, not the
    // target) — against the `Null`-payload probe it fails unconditionally
    // (`PreconditionFailed: "from_entity_id/to_entity_id required..."`),
    // so `assert-control`/`assert-economic-interest` can never appear in
    // `enumerate_placement_set`'s admitted moves, and `stage()` therefore
    // NEVER recognises them again — a real, permanent T2-generator/T6.2-stud
    // incompatibility, not a fixable fixture ordering issue. Reconciling
    // `enumerate_placement_set`'s probe model to thread real per-candidate
    // payloads is a T2-generator change, out of T6.2's edge-family-lexicon
    // scope and risks the pinned `placement.rs` property tests — not
    // attempted here. Worked around the ONLY way available inside this
    // fixture without touching the generator: `manual_staged_move`, this
    // file's own escape hatch for "the only way to get a move into
    // `workbook.staged` at all" when `stage()`'s gate can't admit it. This
    // move is NOT actually illegal — `commit()`'s real `check_preconditions`
    // call (fed the real payload) below proves it — only unrecognisable by
    // the payload-blind probe.
    workbook.staged.push(manual_staged_move(
        subject,
        "kyc_ubo.assert.edge.control",
        TargetBinding::for_edge(subject, EdgeId(edge)),
        serde_json::json!({
            "edge_id": edge, "from_entity_id": from, "to_entity_id": to, "kind": "voting_rights"
        }),
        as_of,
        &lexicon,
    ));

    workbook
        .stage(
            &format!(r#"(kyc_ubo.assert.edge.evidence :edge-id "{edge}" :doc_id "{doc_id}")"#),
            &principal,
            AuthorityRef("analyst.attach-evidence".into()),
            as_of,
        )
        .expect(
            "move 2 (attach-evidence) must recognise against the frontier AFTER move 1 — \
             a committed-only frontier would wrongly refuse this",
        );

    workbook
        .stage(
            &format!(r#"(kyc_ubo.assert.edge.verification :edge-id "{edge}")"#),
            &principal,
            AuthorityRef("analyst.verify".into()),
            as_of,
        )
        .expect(
            "move 3 (verify) must recognise against the frontier AFTER moves 1+2 — \
             a committed-only frontier would wrongly refuse this too",
        );

    // Every staged move's source_text is the canonical resolved re-render
    // (T0.2 / design §6 step 4), not verbatim typed text.
    for staged in &workbook.staged {
        let entry = lexicon.get(staged.event.verb_fqn.as_str());
        assert_eq!(
            staged.source_text,
            render_intent_event_to_sexpr(&staged.event, entry),
            "source_text must be the canonical render, recomputed independently"
        );
    }

    let (pre_commit_state, _pre_commit_obligation, _pre_commit_type_registry) =
        workbook.validate().expect("staged chain must validate");
    let pre_commit_status = pre_commit_state
        .edges
        .get(&EdgeId(edge))
        .map(|e| e.status)
        .expect("edge must exist in the pre-commit preview");

    let mut scope = TestScope::begin(&pool).await;
    let outcomes = workbook
        .commit(&mut scope, &registry)
        .await
        .expect("a fully legal 4-move chain must commit");
    assert_eq!(outcomes.len(), 4);
    scope.commit().await;

    // Re-open (KIT-4 re-run-whole) must reproduce the pre-commit preview,
    // bit-identically — not just at the T3 layer, end to end through commit.
    let mut conn2 = pool.acquire().await.unwrap();
    let reopened = open_workbook(&mut conn2, subject).await.unwrap();
    assert_eq!(reopened.committed.len(), 4);
    let (post_commit_state, _post_commit_obligation, _post_commit_type_registry) = reopened.validate().unwrap();
    assert_eq!(
        post_commit_state.edges.get(&EdgeId(edge)).map(|e| e.status),
        Some(pre_commit_status),
        "re-open after commit must reproduce the last pre-commit validate() state"
    );

    cleanup(&pool, subject).await;
}

// ── new_ubo_from_baseplate ──────────────────────────────────────────────────

#[tokio::test]
async fn new_ubo_from_baseplate() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let principal = runtime_principal("analyst-1");
    let as_of = fixed_ts();

    let mut conn = pool.acquire().await.unwrap();
    let workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);
    assert!(
        workbook.committed.is_empty(),
        "never-appended subject has no committed history"
    );
    let (empty_state, _empty_obligation, _empty_type_registry) = workbook.validate().unwrap();
    assert_eq!(empty_state.edges.len(), 0);
    assert!(!empty_state.registered);

    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);
    workbook
        .stage(
            "(kyc_ubo.assert.subject.register)",
            &principal,
            AuthorityRef("analyst.register".into()),
            as_of,
        )
        .expect("register has no preconditions — legal against the empty baseplate");

    let mut scope = TestScope::begin(&pool).await;
    workbook.commit(&mut scope, &registry).await.unwrap();
    scope.commit().await;

    let mut conn2 = pool.acquire().await.unwrap();
    let reopened = open_workbook(&mut conn2, subject).await.unwrap();
    let (state, _obligation, _type_registry) = reopened.validate().unwrap();
    assert!(
        state.registered,
        "committed register must fold true on re-open"
    );

    cleanup(&pool, subject).await;
}

// ── T1 (EOP-VS-UBO-GAME-001 §3.4 R6/C2): entity-id via the real DSL surface ──

/// T1 gate (RED before this tranche's `canonical_event_shape` landed —
/// see the history below — GREEN now). Through the workbook s-expression
/// surface ONLY — no hand-built `IntentEvent`, no `manual_staged_move`
/// escape hatch — stage `(kyc_ubo.assert.subject.register :subject-id S
/// :entity-id E)` using the exact kebab-case arg names `config/verbs/kyc/
/// dsl-kyc.yaml` declares, commit it, and check whether `E` actually landed
/// in `registered_entity_ids`. R6 says one function builds every surface's
/// event; C2 says the board offers only what the append admits. If the
/// workbook and the op layer disagree about where `entity-id` goes, both
/// cannot be true at once — this is the test that proves whether they do.
///
/// Prior research (`sexpr_to_parsed_move`, `kyc_workbook.rs:209-230`):
/// `entity-id` is one of the five fixed target-binding slots, so it is
/// routed into `ParsedMove.target.entity_id`, never into the payload. The
/// fold arm for `register` (`fold/control.rs`) reads `entity_id` off the
/// **payload**, not the target. Predicted failure: the entity is silently
/// never registered. This test also surfaces a second, earlier symptom of
/// the same root cause: `enumerate_placement_set`'s candidate for
/// `register` is always target-bare (`TargetBinding::for_subject`, general
/// branch, `placement.rs`) since `register` is not edge-scoped, not
/// geometry-gated, not type-registry-scoped — so a parsed target carrying
/// `entity_id: Some(E)` cannot equality-match ANY enumerated candidate's
/// target, and `stage()`'s frontier-membership gate refuses the call
/// before the fold is ever reached. Either failure is the same underlying
/// defect (R6: two paths build the event's target/payload split
/// differently) surfacing at a different point in the pipeline — this test
/// does not assume which one fires, only that the board never ends up
/// knowing about the entity.
#[tokio::test]
async fn t1_register_with_entity_id_via_workbook_surface() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let entity = Uuid::new_v4();
    let registry = v1_registry();
    let principal = runtime_principal("analyst-1");
    let as_of = fixed_ts();

    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);

    // Step 1: stage, through the real DSL parser, using the YAML-declared
    // kebab-case arg names verbatim — no hand-built IntentEvent anywhere.
    let stage_result = workbook.stage(
        &format!(r#"(kyc_ubo.assert.subject.register :subject-id "{subject}" :entity-id "{entity}")"#,
            subject = subject.0),
        &principal,
        AuthorityRef("analyst.register".into()),
        as_of,
    );

    let staged = match stage_result {
        Ok(staged) => staged.clone(),
        Err(e) => {
            // The predicted-secondary failure mode: stage() itself refuses
            // the call because the parsed target (carrying entity_id)
            // cannot equality-match enumerate_placement_set's target-bare
            // candidate for `register`. Still RED, still proves the same
            // R6 defect (two surfaces disagree about what belongs on the
            // target vs the payload) — just caught one step earlier than
            // a silent post-commit fold miss.
            panic!(
                "RED (as predicted, via the frontier-membership gate rather than a silent \
                 fold miss): stage() refused a real DSL call carrying `:entity-id` because \
                 the workbook's own board-move enumerator never produces an entity-id-bearing \
                 candidate for `register` — got: {e:?}"
            );
        }
    };

    // Step 1 (commit).
    let mut scope = TestScope::begin(&pool).await;
    workbook
        .commit(&mut scope, &registry)
        .await
        .expect("register has no preconditions that would block this commit");
    scope.commit().await;

    // Step 2: the fold must have recorded E.
    let mut conn2 = pool.acquire().await.unwrap();
    let reopened = open_workbook(&mut conn2, subject).await.unwrap();
    let (state, _obligation, _type_registry) = reopened.validate().unwrap();
    assert!(
        state.registered_entity_ids.contains(&EntityId(entity)),
        "R6/C2 RED: the workbook staged+committed \
         `(kyc_ubo.assert.subject.register :subject-id S :entity-id E)` through the real DSL \
         surface (staged event payload: {:?}), yet E={entity} is NOT in \
         registered_entity_ids={:?} — the workbook routed :entity-id into the TARGET binding, \
         but the register fold arm reads entity_id from the PAYLOAD. Two surfaces, two \
         different ideas of what `entity-id` means — exactly what R6 forbids.",
        staged.event.payload, state.registered_entity_ids
    );

    // Step 3: the board must now offer a geometry-gated connect candidate
    // touching E. A geometry-gated candidate needs TWO registered, TYPED
    // entities (`placement.rs`'s `is_geometry_gated` branch pairs every
    // registered member against every other) — register and type a second
    // entity, through the same real DSL surface, no escape hatch.
    let second = Uuid::new_v4();
    workbook = reopened;
    workbook
        .stage(
            &format!(r#"(kyc_ubo.assert.subject.register :subject-id "{s}" :entity-id "{second}")"#, s = subject.0),
            &principal,
            AuthorityRef("analyst.register".into()),
            as_of,
        )
        .expect("second registration must recognise — register has no preconditions blocking it");
    workbook
        .stage(
            &format!(
                r#"(kyc_ubo.assert.subject.type :subject-id "{s}" :entity-id "{entity}" :entity-type "natural_person")"#,
                s = subject.0
            ),
            &principal,
            AuthorityRef("analyst.assert-type".into()),
            as_of,
        )
        .expect("typing E as natural_person must recognise against the frontier AFTER E is registered");
    workbook
        .stage(
            &format!(
                r#"(kyc_ubo.assert.subject.type :subject-id "{s}" :entity-id "{second}" :entity-type "private_limited_company")"#,
                s = subject.0
            ),
            &principal,
            AuthorityRef("analyst.assert-type".into()),
            as_of,
        )
        .expect("typing the second entity must recognise against the frontier");

    let mut scope2 = TestScope::begin(&pool).await;
    workbook
        .commit(&mut scope2, &registry)
        .await
        .expect("three legal moves (register + 2×assert-type) must commit");
    scope2.commit().await;

    let mut conn3 = pool.acquire().await.unwrap();
    let final_workbook = open_workbook(&mut conn3, subject).await.unwrap();
    let lexicon = assembly_lexicon();
    let (control, obligation, type_registry) = final_workbook.validate().unwrap();
    let placement = enumerate_placement_set(
        subject, &control, &obligation, &type_registry, &lexicon,
    );
    let has_connect_candidate = placement
        .moves
        .iter()
        .any(|m| m.proposed_edge.is_some());
    assert!(
        has_connect_candidate,
        "R6/C2 GREEN-line check: with E and a second entity both registered and typed, the \
         board must offer at least one geometry-gated connect candidate; \
         enumerate_placement_set returned {} moves, none carrying a proposed_edge",
        placement.moves.len()
    );

    cleanup(&pool, subject).await;
}

// ── both_surfaces_agree_from_declared_args ──────────────────────────────────
//
// P5 gate (EOP-VS-UBO-GAME-001 §3.4 R6): the op layer and the workbook layer
// must derive byte-identical (target, payload) from the same declared
// arguments — not "close enough", and not "folds to the same state" (a
// strictly weaker property, since identical events fold identically by
// construction). This simulates the op-layer path directly — calling
// `canonical_event_shape` with a raw args object, exactly what every
// `kyc_stream_ops.rs` op does — and the workbook-layer path — a real DSL
// parse + `KycWorkbook::stage()`, exactly what the KYC super-user REPL
// surface does — then asserts the two constructed shapes are identical.
// `IntentEvent::new` does not mutate `target`/`payload`, so comparing
// `staged.event.target`/`.payload` against `canonical_event_shape`'s direct
// return is a fair apples-to-apples check of the shape decision alone,
// independent of the identity-stamping (actor/authority/as_of/idem-key)
// each surface adds afterward.
#[tokio::test]
async fn both_surfaces_agree_from_declared_args() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let entity = Uuid::new_v4();
    let principal = runtime_principal("analyst-1");
    let as_of = fixed_ts();

    // Op-layer path: exactly what kyc_stream_ops.rs's KycSubjectRegister::execute does.
    let args = serde_json::json!({ "entity-id": entity.to_string() });
    let (op_target, op_payload, op_edge) =
        canonical_event_shape("kyc_ubo.assert.subject.register", subject, &args)
            .expect("op-layer canonical_event_shape call");
    assert!(op_edge.is_none(), "register never mints an edge");

    // Workbook-layer path: a real DSL parse + stage, same declared value.
    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);
    let staged = workbook
        .stage(
            &format!(
                r#"(kyc_ubo.assert.subject.register :subject-id "{subject}" :entity-id "{entity}")"#,
                subject = subject.0
            ),
            &principal,
            AuthorityRef("analyst.register".into()),
            as_of,
        )
        .expect("workbook-layer stage");

    assert_eq!(
        staged.event.target, op_target,
        "R6 VIOLATION: op layer and workbook layer built different TargetBinding from the \
         same declared :entity-id — a second constructor exists"
    );
    assert_eq!(
        staged.event.payload, op_payload,
        "R6 VIOLATION: op layer and workbook layer built different payload from the same \
         declared :entity-id — a second constructor exists"
    );

    cleanup(&pool, subject).await;
}

// ── invalid_workbook_blocks_commit ──────────────────────────────────────────

#[tokio::test]
async fn invalid_workbook_blocks_commit() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let lexicon = assembly_lexicon();
    let as_of = fixed_ts();

    let edge = Uuid::new_v4();
    let (from, to) = (Uuid::new_v4(), Uuid::new_v4());

    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);

    // Move 1: legal. Move 2: illegal (verify with no evidence attached — the
    // K-11 proof ratchet). Move 3: would-be-legal, never gets the chance.
    workbook.staged.push(manual_staged_move(
        subject,
        "kyc_ubo.assert.edge.control",
        TargetBinding::for_subject(subject),
        serde_json::json!({"edge_id": edge, "from_entity_id": from, "to_entity_id": to, "kind": "voting_rights"}),
        as_of,
        &lexicon,
    ));
    workbook.staged.push(manual_staged_move(
        subject,
        "kyc_ubo.assert.edge.verification",
        TargetBinding::for_edge(subject, EdgeId(edge)),
        serde_json::json!({}),
        as_of,
        &lexicon,
    ));
    workbook.staged.push(manual_staged_move(
        subject,
        "kyc_ubo.assert.edge.evidence",
        TargetBinding::for_edge(subject, EdgeId(edge)),
        serde_json::json!({"doc_id": Uuid::new_v4()}),
        as_of,
        &lexicon,
    ));

    assert!(
        workbook.validate().is_err(),
        "validate() must reject before any commit attempt"
    );

    // Directly call commit() too — simulating a caller that skipped validate().
    let mut scope = TestScope::begin(&pool).await;
    let result = workbook.commit(&mut scope, &registry).await;
    assert!(
        result.is_err(),
        "commit() must independently reject via its own fail-fast preview"
    );
    scope.rollback().await;

    assert_eq!(
        count(&pool, subject).await,
        0,
        "a rejected commit must leave zero rows — nothing partial lands"
    );

    cleanup(&pool, subject).await;
}

// ── zero_inference_assertion ────────────────────────────────────────────────

/// Import allowlist over `kyc_workbook.rs` (design §8 v0.2: allowlist, not a
/// keyword denylist — fails closed on anything new, including via a new
/// sibling helper module this file starts importing from). No model/LLM/
/// agent crate may ever appear in this module's `use` statements.
#[test]
fn zero_inference_assertion() {
    const ALLOWLIST: &[&str] = &[
        "std",
        "chrono",
        "sqlx",
        "uuid",
        "thiserror",
        "serde_json",
        "dsl_parser",
        "dsl_runtime",
        "ob_poc_kyc_seam",
        "ob_poc_kyc_store",
        "ob_poc_kyc_substrate",
        "sem_os_core",
    ];

    let source = include_str!("../src/domain_ops/kyc_workbook.rs");
    let mut violations = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("use ") else {
            continue;
        };
        let root = rest
            .split(|c: char| c == ':' || c == ';' || c == '{' || c.is_whitespace())
            .next()
            .unwrap_or("");
        if root.is_empty() || root == "crate" || root == "super" || root == "self" {
            continue;
        }
        if !ALLOWLIST.contains(&root) {
            violations.push(line.to_string());
        }
    }

    assert!(
        violations.is_empty(),
        "kyc_workbook.rs has imports outside the zero-inference allowlist {ALLOWLIST:?} \
         (extend the allowlist deliberately, in this test, if the import is genuinely \
         zero-inference — never silently): {violations:#?}"
    );
}

/// EOP-PLAN-KYCUBO-KIT-T7 v0.1 §0.3 / I-5: `zero_inference_assertion` above scans
/// external-crate imports only — it explicitly skips `crate`/`super`/`self` roots
/// (see the `continue` above), so a crate-internal `use crate::mcp::...` or
/// `use crate::agent::...` inside the workbook module family would compile today
/// without tripping the gate. This tooth closes that blind spot directly: no
/// `use` line in `kyc_workbook.rs` or `kyc_workbook_surface.rs` may reference the
/// ob-poc inference/discovery surface (`crate::mcp`, `crate::agent`) or any
/// plain-English ramp module (`crate::repl::kyc_ramp`, indicative name per T7
/// §1 I-5). Direction of dependency stays ramp → workbook, never workbook → ramp.
#[test]
fn workbook_module_remains_ramp_free() {
    const FORBIDDEN_PREFIXES: &[&str] = &["crate::mcp", "crate::agent", "crate::repl::kyc_ramp"];

    const SOURCES: &[(&str, &str)] = &[
        (
            "kyc_workbook.rs",
            include_str!("../src/domain_ops/kyc_workbook.rs"),
        ),
        (
            "kyc_workbook_surface.rs",
            include_str!("../src/repl/kyc_workbook_surface.rs"),
        ),
    ];

    let mut violations = Vec::new();
    for (file, source) in SOURCES {
        for line in source.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("use ") else {
                continue;
            };
            if FORBIDDEN_PREFIXES.iter().any(|p| rest.starts_with(p)) {
                violations.push(format!("{file}: {line}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "workbook module family must stay ramp-free — found imports from the \
         forbidden set {FORBIDDEN_PREFIXES:?} (I-5: the ramp depends on workbook \
         types, never the reverse): {violations:#?}"
    );
}

// ── stale_snapshot_recovers ──────────────────────────────────────────────────

#[tokio::test]
async fn stale_snapshot_recovers() {
    let pool = connect().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();
    let principal = runtime_principal("analyst-1");
    let lexicon = assembly_lexicon();
    let as_of = fixed_ts();
    let edge = Uuid::new_v4();

    // Pre-existing committed history: an evidenced edge (setup connection).
    {
        let mut scope = TestScope::begin(&pool).await;
        let assert_event = IntentEvent::new(
            subject,
            "kyc_ubo.assert.edge.control",
            Principal::test_analyst(),
            AuthorityRef("setup.assert-control".into()),
            TargetBinding::for_subject(subject),
            serde_json::json!({"edge_id": edge, "from_entity_id": Uuid::new_v4(), "to_entity_id": Uuid::new_v4(), "kind": "voting_rights"}),
            as_of,
        )
        .with_lexicon_hash(lexicon.hash);
        append_in_scope(
            &mut scope,
            &registry,
            &assert_event,
            "(setup-assert)",
            |_, _, _| Ok(()),
        )
        .await
        .unwrap();

        let evidence_event = IntentEvent::new(
            subject,
            "kyc_ubo.assert.edge.evidence",
            Principal::test_analyst(),
            AuthorityRef("setup.attach-evidence".into()),
            TargetBinding::for_edge(subject, EdgeId(edge)),
            serde_json::json!({"doc_id": Uuid::new_v4()}),
            as_of,
        )
        .with_lexicon_hash(lexicon.hash);
        append_in_scope(
            &mut scope,
            &registry,
            &evidence_event,
            "(setup-evidence)",
            |_, _, _| Ok(()),
        )
        .await
        .unwrap();
        scope.commit().await;
    }

    // Workbook opens: sees the edge Evidenced, stages `verify` — legal
    // against THIS frontier.
    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);
    workbook
        .stage(
            &format!(r#"(kyc_ubo.assert.edge.verification :edge-id "{edge}")"#),
            &principal,
            AuthorityRef("analyst.verify".into()),
            as_of,
        )
        .expect(
            "verify must recognise — the edge is Evidenced in this workbook's committed history",
        );

    // Concurrent connection commits `supersede` on the SAME edge mid-session
    // — a genuine second connection, a real interleave, not a mock.
    {
        let mut scope2 = TestScope::begin(&pool).await;
        let supersede_event = IntentEvent::new(
            subject,
            "kyc_ubo.assert.edge.supersession",
            Principal::test_analyst(),
            AuthorityRef("concurrent.supersede".into()),
            TargetBinding::for_edge(subject, EdgeId(edge)),
            serde_json::json!({}),
            as_of,
        )
        .with_lexicon_hash(lexicon.hash);
        // T6.2 (2026-08-12): `supersede` now carries `EdgeExists`/`EdgeActive`
        // (matrix row 4), but this setup append goes through the raw
        // `append_in_scope` with an unconditional `Ok(())` validator (like the
        // setup events above), bypassing the real lexicon checker entirely —
        // so this concurrent write still lands regardless of what supersede's
        // entry declares. The comment below described the pre-T6.2 state; the
        // bypass, not the (now-stale) "no precondition" premise, is why this
        // still succeeds.
        let outcome = append_in_scope(&mut scope2, &registry, &supersede_event, "(concurrent-supersede)", |_, _, _| Ok(()))
            .await
            .expect("concurrent supersede must itself succeed (raw append bypasses the checker, same as the setup events above)");
        scope2.commit().await;
        assert!(
            !outcome.deduped,
            "concurrent supersede must be a real new append, not a dedupe no-op"
        );
    }

    // Workbook 1 now commits its staged `verify` — the edge it validated
    // against is gone (superseded) by the time commit runs. The fail-fast
    // whole-chain preview (against a FRESH load_events on the scope's own
    // connection) must catch this and reject — never silently commit a
    // verify against a now-superseded edge.
    let mut scope1 = TestScope::begin(&pool).await;
    let result = workbook.commit(&mut scope1, &registry).await;
    assert!(
        result.is_err(),
        "commit() must re-validate against TRUE current state and reject the now-stale verify"
    );
    scope1.rollback().await;

    // The concurrent supersede is the only row that landed for the workbook's
    // own staged verify — confirm nothing from the stale commit attempt did.
    assert_eq!(
        count(&pool, subject).await,
        3,
        "2 setup rows + 1 concurrent supersede; the stale verify must NOT have appended a 4th"
    );

    cleanup(&pool, subject).await;
}
