//! T2.7 (EOP-PLAN-CONTROLPLANE-001): shadow wiring for `ob-poc-control-plane`.
//!
//! Translates already-computed `SemOsContextEnvelope` state into the
//! control plane's `EvaluationContext` (never recomputes verb-surface
//! membership or SemOS pruning — see `ob_poc_control_plane::context`),
//! calls `ob_poc_control_plane::evaluate_shadow`, and persists the report
//! beside the legacy Phase 5 recheck outcome for divergence triage. This
//! module never gates dispatch; persistence is best-effort (failures are
//! logged, never propagated — same posture as `agent::telemetry::store`).

use std::collections::HashMap;

use sem_os_policy::abac::ActorContext;
use uuid::Uuid;

use crate::agent::sem_os_context_envelope::{PruneReason, SemOsContextEnvelope};
use crate::journey::pack::PackManifest;
use crate::repl::verb_config_index::VerbConfigIndex;

/// T9.1a (EOP-PLAN-CONTROLPLANE-001 Addendum B): builds G3's
/// `PackResolutionInput` from the REPL's real, live single-active-pack
/// session state — not from `sem_os_policy::domain_pack::DomainPackManifest`
/// (the SemOS Domain Pack taxonomy system). This corrects an error in the
/// T9.1-pre design pass, recorded in the ownership ledger's T9.1a entries:
/// `pack_resolution.rs`'s own module doc names its designed production
/// analogue as `src/runbook/constraint_gate.rs::check_pack_constraints`
/// against `journey::pack_manager::{PackManager, EffectiveConstraints}` —
/// and those operate on REPL *journey* packs (`config/packs/*.yaml`, bare
/// ids like `"kyc-case"`), not SemOS Domain Packs (dotted ids like
/// `"ob-poc.cbu"`). The design pass's caution about not conflating "pack"
/// with `constellation_family`/`constellation_map` was correct in
/// principle but pointed at the wrong "other" system — the SemOS Domain
/// Pack taxonomy has no live runtime instance at all (confirmed: zero
/// production `PackManager::new(` call sites, see the T9.1a ledger entry),
/// while the REPL journey pack this function actually uses is real,
/// live, and already tracked (`ReplSessionV2::active_pack_id()`,
/// `ReplOrchestratorV2::pack_router`).
///
/// `PackManager` itself also has zero production callers — resolved here
/// not by building new session-persistent activation tracking (which
/// would duplicate what the REPL already tracks), but by constructing a
/// **fresh, throwaway `PackManager`** per shadow-recheck call: register
/// the single currently-active pack, activate it, and call the exact
/// same `check_pack_constraints` the C-015/C-016 ledger rows say G3 was
/// designed to invoke. `PackManager` is pure in-memory state (`HashMap`s,
/// no I/O) — this is cheap, not a workaround.
///
/// `constraint_denies_intent` is always `false` here, not a placeholder:
/// `EffectiveConstraints::is_empty_intersection()` can only be `true`
/// when the *intersection* of multiple simultaneously-active packs'
/// `allowed_verbs` is empty (`journey/pack_manager.rs::effective_constraints`'s
/// intersection logic) — with exactly one active pack (this REPL's real
/// model; `active_pack_id()` returns a single `Option<String>`, never a
/// set), there is nothing to intersect against, so the condition is
/// unreachable by construction. Whether the verb itself is permitted is
/// carried by `candidate_pack_ids` instead (`vec![pack_id]` when
/// `check_pack_constraints` returns `Ok`, `vec![]` — read as `MissingPack`
/// by `decide()` — when it returns `Err`, i.e. the active pack forbids or
/// doesn't declare this verb).
///
/// **Known limitation, not swept under the rug:** `pack_resolution.rs`'s
/// own doc says "No active pack means no execution" — taken literally,
/// `active_pack_id() == None` always yields `MissingPack`. Some verbs
/// (navigation, `session.*`) legitimately execute outside any pack's
/// InPack tollgate. This may over-report G3 failures for those verbs.
/// Safe because this is shadow-only (never gates real dispatch) — a
/// gating (non-shadow) use of this function would need this resolved
/// first, not inherited as-is.
pub(crate) fn build_pack_resolution_input(
    active_pack: Option<(&str, &PackManifest)>,
    verb_fqn: &str,
    semreg_allowed_set_available: bool,
) -> ob_poc_control_plane::pack_resolution::PackResolutionInput {
    let candidate_pack_ids = match active_pack {
        None => Vec::new(),
        Some((pack_id, manifest)) => {
            let mut manager = crate::journey::pack_manager::PackManager::new();
            manager.register_pack(manifest.clone());
            match manager.activate_pack(pack_id) {
                Ok(()) => {
                    let constraints = manager.effective_constraints();
                    match crate::runbook::constraint_gate::check_pack_constraints(
                        &[verb_fqn.to_string()],
                        &constraints,
                    ) {
                        Ok(()) => vec![pack_id.to_string()],
                        Err(_) => Vec::new(),
                    }
                }
                Err(e) => {
                    // Dormant -> Active should never fail for a freshly
                    // registered pack; if it somehow does, fail honestly
                    // (no candidate) rather than guess.
                    tracing::warn!(
                        error = %e,
                        pack_id,
                        "T9.1a: freshly-registered pack failed to activate — treating as MissingPack"
                    );
                    Vec::new()
                }
            }
        }
    };

    ob_poc_control_plane::pack_resolution::PackResolutionInput {
        candidate_pack_ids,
        semreg_allowed_set_available,
        constraint_denies_intent: false,
    }
}

/// T9.1b (EOP-PLAN-CONTROLPLANE-001 Addendum B): builds G4's
/// `DagProofInput` by reusing the real v1.3 gate's own resolution
/// mechanism — `resolve_transition_probe`, extracted from
/// `pre_dispatch_gate_check`'s original inline body (see that function's
/// doc for the extraction's equivalence proof) — rather than re-deriving
/// verb→transition resolution from scratch. This is the same
/// `entity_id_arg`/`target_state_arg`/`target_workspace`/`target_slot`
/// (`transition_args`, 87 verbs declaring it) and the same
/// `GateChecker::check_transition` the real dispatch-path gate uses.
///
/// `gate_pipeline` is `ReplOrchestratorV2`'s own `GatePipeline` (built at
/// `ob-poc-web::main` startup) — already reachable at this call site
/// since `phase5_runtime_recheck` is a method on the same struct; no new
/// plumbing needed.
///
/// Returns `None` when: no `GatePipeline` is wired (shadow simply has
/// nothing to observe yet — not an error); the verb has no
/// `transition_args` declared (most verbs are not state transitions at
/// all); the DAG has no matching transition for this verb; or resolution
/// itself failed (missing/invalid entity_id arg, unresolvable
/// workspace — logged, not silently promoted to a wrong-but-passing
/// fact, same posture as `build_entity_binding_input`'s entity_facts
/// lookup failure).
///
/// `lifecycle_fail_open_class` stays `None` and
/// `lifecycle_gate_mode_fail_closed` stays `false` — T0.2's
/// `enforce_requires_states_precondition` needs a live `&mut dyn
/// TransactionScope` (designed for real dispatch, not read-only shadow
/// observation); unifying it here is real follow-on work, not silently
/// folded into this tranche (see the ownership ledger).
pub(crate) async fn build_dag_proof_input(
    gate_pipeline: Option<&crate::runbook::step_executor_bridge::GatePipeline>,
    verb_fqn: &str,
    entry_args: &HashMap<String, String>,
) -> Option<ob_poc_control_plane::dag_proof::DagProofInput> {
    let pipe = gate_pipeline?;
    let probe = crate::runbook::step_executor_bridge::resolve_transition_probe(
        pipe,
        verb_fqn,
        |arg| entry_args.get(arg).map(|s| s.as_str()),
    )
    .await;

    match probe {
        Ok(Some(probe)) => Some(ob_poc_control_plane::dag_proof::DagProofInput {
            entity_id: probe.entity_id,
            from_state: probe.from_state,
            to_state: probe.to_state,
            blocking_violations: probe.blocking_violations,
            lifecycle_fail_open_class: None,
            lifecycle_gate_mode_fail_closed: false,
        }),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(
                verb_fqn,
                error = %e,
                "T9.1b: DAG transition probe resolution failed — G4 shadow-evaluates as not-attempted"
            );
            None
        }
    }
}

/// G2 item 2 (`EOP-PLAN-CONTROLPLANE-GRADUATION-001` §3): a real,
/// non-empty, provably-correct `allowed_columns` derivation for a narrow,
/// exactly-verified subset of CRUD verbs — the piece the prior session's
/// STOP-condition (`write_set_attestation.rs`'s
/// `empty_allowed_columns_breaches_every_write_with_any_column_even_on_exact_table_and_entity_match`
/// test) found missing. `domain_metadata.yaml`'s `writes: [...]` list is
/// table-only (`sem_os_obpoc_adapter::metadata::VerbFootprint` has no
/// column field at all — confirmed by direct read, not assumed); the only
/// place a verb's actual write *columns* are declared anywhere in this
/// codebase is the verb YAML's own `crud.args[].maps_to` mapping — the
/// exact same source `dsl_runtime::crud_executor`'s real dispatch
/// (`execute_insert`/`execute_update`/`execute_upsert`) reads to build its
/// SQL and to self-report `record_write`'s captured columns.
///
/// **Scope, deliberately narrow — verified against `crud_executor.rs`'s
/// exact column-selection logic per operation, not guessed:**
/// - `CrudOperation::Insert` / `CrudOperation::Upsert`: `raw_columns`
///   ALWAYS starts with the primary-key column
///   (`execute_insert`/`execute_upsert`, `let mut raw_columns = vec![pk_col...]`)
///   plus every `arg_defs[].maps_to` present in the call's `args`. This
///   function returns the *superset* (every declared `maps_to` ∪ pk_col),
///   safe because the real per-call `raw_columns` is always a subset of
///   it (arg presence is the only thing that varies per call, not the
///   declared mapping).
/// - `CrudOperation::Update`: `raw_columns` is `arg_defs[].maps_to` minus
///   the key column (the key becomes a `WHERE` bind, never a `SET`
///   column) — this function's superset (all declared `maps_to`,
///   including the key) is still safe, it just never gets exercised for
///   the key column specifically.
/// - **Insert/Upsert additionally require `crud.returning` to be
///   EXPLICITLY set in the verb YAML.** When it is absent, the real
///   dispatch falls back to `infer_pk_column` — a best-effort
///   table-name-based heuristic with its own fallback arm this function
///   does not re-verify exhaustively; guessing wrong here would
///   UNDER-declare the allowed set (the unsafe direction, since a real
///   pk write would then look like a breach), so those verbs return
///   `None` rather than risk it.
/// - **Every other `CrudOperation` variant (`Delete`, `Link`, `Unlink`,
///   `RoleLink`, `RoleUnlink`, `ListByFk`, `ListParties`,
///   `SelectWithJoin`, `Select`, `EntityCreate`, `EntityUpsert`) returns
///   `None`, deliberately not attempted this session.** `Delete` alone
///   is a proven case why: `crud_executor.rs::execute_delete`'s
///   soft-delete branch writes a literal `"deleted_at"` column that
///   never appears in any `arg_defs[].maps_to` mapping at all — a
///   generic `maps_to`-only derivation would silently under-declare it.
///   `Link`/`Unlink` write junction-table `from_col`/`to_col` pairs via a
///   structurally different code path this function does not mirror.
///
fn derive_crud_allowed_columns(
    rv: &crate::dsl_v2::runtime_registry::RuntimeVerb,
) -> Option<Vec<String>> {
    use crate::dsl_v2::runtime_registry::RuntimeBehavior;

    let RuntimeBehavior::Crud(crud) = &rv.behavior else {
        return None;
    };
    let maps_to_cols: Vec<String> = rv.args.iter().filter_map(|a| a.maps_to.clone()).collect();
    derive_allowed_columns_for_operation(crud.operation, crud.returning.clone(), maps_to_cols)
}

/// The pure per-operation half of [`derive_crud_allowed_columns`], split
/// out so its Insert/Update/Upsert/"everything else" branch logic is unit
/// testable without constructing a full `RuntimeVerb` fixture (that type
/// has no `Default` impl and a dozen-plus fields unrelated to this
/// decision).
fn derive_allowed_columns_for_operation(
    operation: dsl_core::CrudOperation,
    returning: Option<String>,
    maps_to_cols: Vec<String>,
) -> Option<Vec<String>> {
    use dsl_core::CrudOperation;

    let mut columns = maps_to_cols;

    match operation {
        CrudOperation::Insert | CrudOperation::Upsert => {
            let pk_col = returning?; // explicit only — see doc comment
            columns.push(pk_col);
        }
        CrudOperation::Update => {
            if columns.is_empty() {
                return None;
            }
        }
        _ => return None,
    }

    columns.sort();
    columns.dedup();
    Some(columns)
}

/// G2 item 4 (`EOP-SESSION-CONTROLPLANE-G14-TABLE-FORMAT-FIX-001`):
/// schema-qualifies a `domain_metadata.yaml` `writes:`/`reads:` table entry
/// to the exact `"{schema}.{table}"` string `crud_executor.rs`'s
/// `record_write` self-reports in production (all 4 call sites —
/// `execute_insert`/`execute_update`/`execute_delete`/`execute_upsert` —
/// verified to use the identical `format!("{schema}.{table}")`, sourced
/// from the same `dispatch()`-computed `schema`/`table` pair).
///
/// `domain_metadata.yaml`'s own file header documents its convention
/// verbatim: *"Tables without a schema prefix default to `ob-poc`. Tables
/// in other schemas use `schema.table` notation."* — this function
/// implements exactly that stated convention: a bare name is qualified as
/// `ob-poc.{table}`; a name that already contains a `.` is trusted
/// verbatim (the file's own explicit schema declaration).
///
/// **Known, pre-existing, NOT fixed by this function:** cross-checking the
/// real file against `migrations/master-schema.sql`'s `CREATE SCHEMA`
/// list (`"ob-poc"`, `sem_reg`, `sem_reg_authoring`, `sem_reg_pub` — no
/// `kyc` schema exists) found the stated convention is itself violated in
/// two independent, narrow ways already present in the data, neither
/// introduced or corrected by this fix:
/// - `kyc.cases` / `kyc.ownership_snapshots` use a `kyc.` prefix that
///   is **not a real Postgres schema** — both tables actually live in
///   `"ob-poc"` (`CREATE TABLE "ob-poc".cases`, `CREATE TABLE
///   "ob-poc".ownership_snapshots`). Passed through verbatim by this
///   function (it trusts any dotted name), so `WriteSetInput.tables` for
///   `kyc.set-case`/`ownership.compute`/`ownership.snapshot.list` will
///   read `"kyc.cases"`/`"kyc.ownership_snapshots"` — a string that will
///   never match any real `CapturedWrite.table` (which would report
///   `"ob-poc.cases"`/`"ob-poc.ownership_snapshots"`).
/// - `team.create`/`team.add-member`/`team.remove-member` declare bare
///   names (`teams`, `memberships`) that this function therefore qualifies
///   as `ob-poc.teams`/`ob-poc.memberships` — but `config/verbs/team.yaml`
///   declares those verbs' real `crud_mapping.schema` as `teams`, not
///   `ob-poc` (`record_write` will actually report `"teams.teams"`/
///   `"teams.memberships"`).
///
/// Both are pre-existing data-quality defects in `domain_metadata.yaml`
/// itself (wrong schema attribution, not a format mismatch this function
/// is scoped to fix) — flagged, not silently absorbed; see this session's
/// doc for the recommendation to correct the 5 affected entries in a
/// follow-up. Every other domain in the file (confirmed by full-file grep:
/// the only dotted prefixes appearing anywhere in `reads:`/`writes:` are
/// `kyc.`, `sem_reg.`, `sem_reg_authoring.`, `sem_reg_pub.`) either matches
/// its real schema already (`sem_reg*`) or has no footprint entry at all
/// for any verb using a non-`ob-poc` `crud_mapping.schema` (confirmed:
/// zero `access-review.*`/`application-instance.*` entries in
/// `domain_metadata.yaml`, the only other verb YAMLs declaring a
/// non-`ob-poc` schema besides `team.yaml`).
fn qualify_footprint_table(table: &str) -> String {
    if table.contains('.') {
        table.to_string()
    } else {
        format!("ob-poc.{table}")
    }
}

/// T9.1e (EOP-PLAN-CONTROLPLANE-001 Addendum B): builds G7's
/// `WriteSetInput` from the verb's declared write footprint
/// (`config/sem_os_seeds/domain_metadata.yaml`'s per-verb `writes: [...]`,
/// loaded once at startup into `domain_metadata`). `WriteSetGate::decide`
/// (`write_set.rs`) only checks `contract_derived` and non-empty `tables`
/// — `state_slots` may legitimately stay empty (no production source
/// exists yet, and the gate doesn't require it). `allowed_columns` is now
/// real (G2 item 2, `derive_crud_allowed_columns` above) for the narrow
/// Insert/Update/Upsert-with-explicit-`returning` subset; every other
/// verb still gets an empty list (honest "cannot derive," not a fabricated
/// guess) — matching this function's own pre-existing `None`-on-cannot-
/// derive posture for the table-level footprint.
///
/// `tables` is schema-qualified via [`qualify_footprint_table`] (G2 item
/// 4) so it can actually be compared against `CapturedWrite.table`'s
/// `"{schema}.{table}"` production format — see that function's doc for
/// the verified convention and its two known, pre-existing exceptions.
///
/// Returns `None` (not a fabricated `CannotDerive`) when: no
/// `DomainMetadata` is wired; the verb has no footprint entry at all; or
/// the footprint's `writes` list is empty — a read-only verb legitimately
/// writes nothing, and grading it against a write-bounding gate would be
/// the same false-negative-by-construction class of bug flagged elsewhere
/// in this module (compare G4's "no matching transition" `None` case).
///
/// `idempotency_key` is a deterministic `entry_id:verb_fqn` pair — shadow
/// only, not the real T5.1 write-set attestation mechanism (which owns
/// its own idempotency key derivation for actual dispatch correlation).
pub(crate) fn build_write_set_input(
    domain_metadata: Option<&sem_os_obpoc_adapter::metadata::DomainMetadata>,
    verb_fqn: &str,
    entry_id: Uuid,
    entity_ids: Vec<Uuid>,
) -> Option<ob_poc_control_plane::write_set::WriteSetInput> {
    let metadata = domain_metadata?;
    let footprint = metadata.find_verb_footprint(verb_fqn)?;
    if footprint.writes.is_empty() {
        return None;
    }

    let allowed_columns = verb_fqn
        .split_once('.')
        .and_then(|(domain, verb)| crate::dsl_v2::runtime_registry::runtime_registry().get(domain, verb))
        .and_then(derive_crud_allowed_columns)
        .unwrap_or_default();

    let tables = footprint.writes.iter().map(|t| qualify_footprint_table(t)).collect();

    Some(ob_poc_control_plane::write_set::WriteSetInput {
        entity_ids,
        state_slots: Vec::new(),
        tables,
        allowed_columns,
        idempotency_key: format!("{entry_id}:{verb_fqn}"),
        contract_derived: true,
    })
}

/// T9.5 (EOP-PLAN-CONTROLPLANE-001 Addendum B): builds G8's
/// `StpClassifierInput` from the same `RuntimeBehavior` lookup
/// `sem_os_runtime::verb_executor_adapter` already uses to route
/// CRUD-vs-Plugin-vs-Durable dispatch (`RuntimeBehavior::Durable` is the
/// "external workflow engine, e.g. BPMN-Lite" variant). `is_durable_verb`
/// is `false`, honestly, whenever the FQN doesn't parse as `domain.verb`
/// or the registry has no entry for it — an unregistered verb cannot be a
/// durable one by definition (there's no `RuntimeDurableConfig` for it to
/// carry).
///
/// `durable_execution_explicitly_allowed` is always `false` here, not a
/// placeholder: this function is only ever called from
/// `phase5_runtime_recheck`, which is Path A's own REPL/runbook dispatch
/// — never a BPMN direct-worker context (the one place durable execution
/// is actually permitted to run outside its owning engine). A verb that
/// is both durable and reached this call site has, by construction,
/// nothing granting the exception `StpClassifierInput` exists to check
/// for.
///
/// `has_unpinned_entities` is threaded in by the caller rather than
/// computed here — see [`has_unpinned_entities`] (G6b, EOP-PLAN-
/// CONTROLPLANE-GRADUATION-001, RR-5 row 2 "Entity tables intended for
/// TOCTOU"), which now derives this bool from the same `entity_facts_map`
/// G2/G13 already fetched for this call, replacing the blanket "any bound
/// entity is unpinned" placeholder this doc used to describe (T4.3's
/// populator: real per-entity `row_version` pins now flow end-to-end from
/// `PgEntityFactsSource` through `build_decision_snapshot_input`/
/// `build_pins` into a sealed envelope's `SnapshotPins`, so an entity that
/// was actually found and pinned is no longer forced through
/// `RequiresHumanGate`).
pub(crate) fn build_stp_classifier_input(
    verb_fqn: &str,
    has_unpinned_entities: bool,
) -> ob_poc_control_plane::stp_classifier::StpClassifierInput {
    use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeBehavior};

    let is_durable_verb = verb_fqn
        .split_once('.')
        .and_then(|(domain, verb)| runtime_registry().get(domain, verb))
        .map(|rv| matches!(rv.behavior, RuntimeBehavior::Durable(_)))
        .unwrap_or(false);

    ob_poc_control_plane::stp_classifier::StpClassifierInput {
        is_durable_verb,
        durable_execution_explicitly_allowed: false,
        has_unpinned_entities,
    }
}

/// G6b (EOP-PLAN-CONTROLPLANE-GRADUATION-001, RR-5 row 2 — "Entity tables
/// intended for TOCTOU"): the real "is this bound entity pinned" fact,
/// derived from the exact same `entity_facts_map` batched read G2/G13
/// already fetch (`entity_facts.rs::EntityFactsSource::entity_facts`,
/// covering `cbu`/`entity`/`case`/`deal`/`client_group` — the five kinds
/// the row_version migration (`20260422_row_version_entity_tables.sql`,
/// confirmed applied 2026-07-14: `\d+ "ob-poc".cbus` shows the column and
/// trigger live) actually covers).
///
/// An entity counts as pinned when it is present in `facts_map` — per
/// `build_entity_binding_input`'s own contract, `EntityFactsSource`
/// returns an entry *only* for a row it actually read (and captured
/// `row_version` from in the same query); an entity missing from the map
/// was not found (or its kind isn't one of the five covered ones), so
/// there is no `row_version` to pin — honestly unpinned, matching G2's
/// own `exists: false` grading for the identical condition.
///
/// `facts_map: None` (the batched fetch itself errored — see
/// `entity_facts_map`'s construction in `phase5_runtime_recheck`/
/// `reseal_for_human_gate_resume`) is unpinned for every requested entity,
/// fail-closed: an I/O failure is not evidence of a pin.
///
/// Zero requested entities is vacuously **not** unpinned (nothing to
/// pin-check) — same posture `build_entity_binding_input`/
/// `build_write_set_input` already use for the empty case.
pub(crate) fn has_unpinned_entities(
    requests: &[(Uuid, String)],
    facts_map: Option<&HashMap<Uuid, ob_poc_boundary::entity_facts::EntityFactsRow>>,
) -> bool {
    if requests.is_empty() {
        return false;
    }
    let Some(facts_map) = facts_map else {
        return true;
    };
    requests.iter().any(|(id, _)| !facts_map.contains_key(id))
}

/// T9.6 (EOP-PLAN-CONTROLPLANE-001 Addendum B): builds G13's `SnapshotInput`
/// from the exact same batched entity-facts rows G2 already fetches —
/// `EntityFactsRow.row_version` was added specifically for this convergence
/// (see `entity_facts.rs`'s own module doc: "T9.2's `SnapshotPins` need
/// `row_version` from the same rows"), so this reads no new data at all.
///
/// `sem_reg_snapshot_id` / `session_snapshot_id` / `kyc_manifest_hash` /
/// `versions` (the `PinnedVersionSet`) all stay at their `Option::None` /
/// `Default` values — not a placeholder, but this call site's honest
/// answer: `snapshot.rs`'s own module doc says plainly "No production
/// analogue exists today" for those four pins specifically (SemReg
/// snapshot-set id, session snapshot id, KYC manifest hash, and the T4.4
/// compiler/model/prompt version bundle). Only the per-entity
/// `row_version` pin has a real, live source at this call site.
///
/// `DecisionSnapshotGate::decide` (see `snapshot.rs`) only checks whether
/// `Some(_)` was supplied at all — an empty-but-present `SnapshotInput` is
/// `Success` by the gate's own design ("this gate pins whatever was read,
/// it doesn't judge it"), so `None` is reserved for the one case that
/// means "we didn't even attempt a read": the batched facts fetch itself
/// erroring. A verb with zero entity-typed args legitimately gets
/// `Some(SnapshotInput { entity_row_versions: vec![], .. })` — vacuously
/// nothing to pin, not a failed attempt — matching G2's own
/// zero-entity-args posture (`build_evaluation_context`'s doc).
pub(crate) fn build_decision_snapshot_input(
    facts: Option<&HashMap<Uuid, ob_poc_boundary::entity_facts::EntityFactsRow>>,
) -> Option<ob_poc_control_plane::snapshot::SnapshotInput> {
    let facts = facts?;
    let entity_row_versions = facts
        .values()
        .map(|row| {
            (
                row.facts.entity_id,
                row.facts.expected_kind.clone(),
                row.row_version,
            )
        })
        .collect();
    Some(ob_poc_control_plane::snapshot::SnapshotInput {
        entity_row_versions,
        ..Default::default()
    })
}

/// T9.7 (widened T10.1, EOP-PLAN-CONTROLPLANE-001 Addendum B/C): builds
/// G9's `RunbookProofInput` from the one real fact this call site has —
/// the runbook entry's own `CompiledRunbookId`, when present.
/// `try_compile_entry()` populates this before the execution loop reaches
/// `phase5_runtime_recheck` for entries created through the current
/// pipeline (INV-3: raw DSL execution without a `CompiledRunbookId` is
/// never permitted) — the fallback on-the-fly compile path only exists
/// for legacy entries, so `None` here is a rare, legitimate case, not a
/// systematic false negative. Widened from a bare `bool` (T9.7) to the
/// real `Uuid` because T10.1's sealing path needs an actual
/// `CompiledRunbookRef` to construct, not merely a presence signal.
///
/// Always `Some(_)` (never `None`) — unlike G2/G7/G8/G13, there is no
/// fallible I/O step here to fail; the fact is read directly off the
/// entry already in hand.
pub(crate) fn build_runbook_proof_input(
    compiled_runbook_id: Option<Uuid>,
) -> ob_poc_control_plane::proof::RunbookProofInput {
    ob_poc_control_plane::proof::RunbookProofInput {
        compiled_runbook_id,
    }
}

/// T9.7 (EOP-PLAN-CONTROLPLANE-001 Addendum B): builds G12's
/// `VersionPinningInput`. Only `compiler_version` has a real source at
/// this call site (`env!("CARGO_PKG_VERSION")`, this crate's own build
/// version — the closest existing proxy for "DSL/compiler crate version",
/// per `PinnedVersionSet`'s own field doc); `bus_catalogue_version`/
/// `model_version`/`prompt_version` stay `None` — no production source
/// for any of the three exists at this call site yet. Always `Some(_)`,
/// same posture as G9: reading `env!` cannot fail.
pub(crate) fn build_version_pinning_input() -> ob_poc_control_plane::versioning::VersionPinningInput
{
    ob_poc_control_plane::versioning::VersionPinningInput {
        versions: ob_poc_control_plane::snapshot::PinnedVersionSet {
            compiler_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..Default::default()
        },
    }
}

/// T9.1-pre (EOP-PLAN-CONTROLPLANE-001 Addendum B): identifies which of a
/// verb's resolved args are entity references, by contract — not by
/// regexing values for UUID shape (`write_set.rs::derive_write_set_heuristic`
/// was explicitly ruled out as a G2 input source during the T9.1-pre design
/// pass: a UUID-shaped string isn't necessarily a bound entity, and a
/// missed one is a silently ungraded binding). Uses
/// `VerbConfigIndex::entries[fqn].args[].lookup_entity_type` (the same
/// contract metadata `write_set.rs`'s A4 contract-driven path already
/// consumes) to find entity-typed args, then resolves each matched arg
/// name against `entry_args` for its UUID value. Args present in the
/// contract but absent from `entry_args`, or whose value doesn't parse as
/// a UUID, are silently skipped — not every entity-typed arg is required
/// to be bound for every verb (optional args), and a non-UUID value here
/// indicates the arg wasn't resolved to an entity reference at all (e.g.
/// still a symbol placeholder), not a binding failure this function should
/// report.
pub(crate) fn entity_binding_requests(
    verb_config_index: &VerbConfigIndex,
    verb_fqn: &str,
    entry_args: &HashMap<String, String>,
) -> Vec<(Uuid, String)> {
    let Some(entry) = verb_config_index.get(verb_fqn) else {
        return Vec::new();
    };
    entry
        .args
        .iter()
        .filter_map(|arg| {
            let entity_type = arg.lookup_entity_type.as_ref()?;
            let raw = entry_args.get(&arg.name)?;
            let id = Uuid::parse_str(raw.trim()).ok()?;
            Some((id, entity_type.clone()))
        })
        .collect()
}

/// Converts a batched [`ob_poc_boundary::entity_facts::EntityFactsRow`]
/// lookup result into G2's `EntityBindingInput`. Every `(entity_id, kind)`
/// in `requests` gets an entry — entities missing from `facts` (the
/// `EntityFactsSource` contract: absent means not found) become an
/// honest `exists: false` fact rather than being silently dropped, so a
/// dangling reference is graded `NotFound`, not skipped.
pub(crate) fn build_entity_binding_input(
    requests: &[(Uuid, String)],
    facts: &HashMap<Uuid, ob_poc_boundary::entity_facts::EntityFactsRow>,
) -> ob_poc_control_plane::entity_binding::EntityBindingInput {
    let entities = requests
        .iter()
        .map(|(id, kind)| match facts.get(id) {
            Some(row) => row.facts.clone(),
            None => ob_poc_control_plane::entity_binding::EntityFacts {
                entity_id: *id,
                exists: false,
                expected_kind: kind.clone(),
                actual_kind: String::new(),
                lifecycle_state_readable: false,
                availability_blocked: false,
                availability_reason: None,
                in_active_pack: false,
            },
        })
        .collect();
    ob_poc_control_plane::entity_binding::EntityBindingInput { entities }
}

/// Builds the T2/T9.1-wired portion of `EvaluationContext`.
///
/// **Wired with real data (not fabricated) as of T9.1c/T9.1d
/// (EOP-PLAN-CONTROLPLANE-001 Addendum B):**
/// - G1 (intent admission, T2.1): `envelope.allowed_verbs`/`pruned_verbs`.
/// - G2 (entity binding, T9.1-pre): `entity_binding` is `Some(input)`
///   whenever the caller *attempted* binding at all — including a verb
///   with zero entity-typed args, which correctly yields
///   `Some(EntityBindingInput { entities: vec![] })`. Per
///   `entity_binding.rs::decide`, an empty `entities` list is vacuous
///   `Success` (nothing to check, so nothing failed) — passing `None`
///   instead for the no-entity-args case would incorrectly turn every
///   entity-less verb (e.g. `session.info`) into a hard
///   `GateResult::Failure("no EntityBindingInput supplied")`, exactly the
///   "guaranteed-wrong signal" class of bug the T9.1c/d empirical probe
///   exists to catch. Reserve `None` for when the caller genuinely
///   couldn't attempt the check at all (no DB access) — an honest "we
///   don't know", appropriately graded `Failure`, not "there was nothing
///   to check". The caller does the I/O
///   (`ob_poc_boundary::entity_facts::EntityFactsSource`, §9.1's
///   decision-assembler law); this function only assembles what it's
///   given via [`build_entity_binding_input`].
/// - G5 (authority, T9.1c): `access_decision` is `Deny` iff `envelope.pruned_verbs`
///   carries an `AbacDenied` entry for this verb, else `Allow` — the only
///   authority-specific signal this call site has. `actor_id`/`role` come
///   from the SAME `ActorContext` this call site already resolves for the
///   G1 check (`sequencer.rs`'s `phase5_runtime_recheck`), not a new or
///   divergent actor-resolution mechanism.
/// - G6 (evidence, T9.1d): `evidence_gaps` maps directly from
///   `envelope.evidence_gaps` (SemOS's own real governance/evidence
///   computation, already run to build the envelope) — no new source. The
///   KYC-specific fields (`kyc_precondition_failures`,
///   `*_obligation_ids`) stay empty: no KYC-substrate adapter is wired at
///   this call site, and most verbs dispatched through Path A are not
///   KYC-domain verbs at all. This makes the resulting `EvidenceOutcome`
///   `Sufficient` for the common case and `MissingRequiredEvidence`
///   whenever SemOS itself detected a gap — never a fabricated split
///   finer than what's actually observed.
///
/// - G3 (pack resolution, T9.1a): built by [`build_pack_resolution_input`]
///   from the REPL's live single-active-pack session state
///   (`ReplSessionV2::active_pack_id()` + `ReplOrchestratorV2::pack_router`)
///   — see that function's doc for the full design, including the
///   correction of the T9.1-pre design pass's original (wrong) assumption
///   that this needed the SemOS Domain Pack taxonomy.
///
/// - G4 (DAG transition proof, T9.1b): built by [`build_dag_proof_input`]
///   by reusing the real v1.3 gate's own resolution mechanism
///   (`step_executor_bridge::resolve_transition_probe`, extracted from
///   `pre_dispatch_gate_check` — see that function's doc) against the
///   `GatePipeline` already carried on `ReplOrchestratorV2`. `None` when
///   no `GatePipeline` is wired, the verb has no `transition_args`
///   declared, or the DAG has no matching transition — all legitimate
///   (most verbs are not state transitions at all). `lifecycle_fail_open_class`
///   stays `None` / `lifecycle_gate_mode_fail_closed` stays `false`: T0.2's
///   `enforce_requires_states_precondition` needs a live `&mut dyn
///   TransactionScope` (it's designed for real dispatch, not a read-only
///   shadow observation) — unifying it here is real follow-on work, not
///   silently folded into this tranche.
///
/// - G7 (write-set, T9.1e): built by [`build_write_set_input`] from the
///   verb's declared write footprint (`domain_metadata.yaml`, loaded once
///   at startup). `None` when no `DomainMetadata` is wired, the verb has
///   no footprint entry, or the footprint declares no writes (read-only
///   verbs legitimately write nothing) — all legitimate, not fabricated.
///   `state_slots`/`allowed_columns` stay empty (no production source for
///   column-level footprint exists yet); `WriteSetGate::decide` doesn't
///   require them.
///
/// - G8 (STP classifier, T9.5): built by [`build_stp_classifier_input`]
///   from the same `RuntimeBehavior` lookup the real dispatch router uses
///   to distinguish CRUD/Plugin/GraphQuery/Durable verbs.
///   `durable_execution_explicitly_allowed` is always `false` (this call
///   site is Path A's own REPL dispatch, never a BPMN direct-worker
///   context). `has_unpinned_entities` is `!entity_ids.is_empty()` — no
///   production `SnapshotPins` populator exists yet (T4.3), so every
///   bound entity is honestly unpinned.
///
/// - G13 (decision snapshot, T9.6): built by
///   [`build_decision_snapshot_input`] from the same batched entity-facts
///   rows G2 already fetches (`EntityFactsRow.row_version`, no second
///   query). `sem_reg_snapshot_id`/`session_snapshot_id`/`kyc_manifest_hash`/
///   `versions` all stay at their defaults — no production source exists
///   for those yet (`snapshot.rs`'s own module doc). `None` only when the
///   batched facts fetch itself errored (same posture as G2/G8).
///
/// - G9 (runbook proof, T9.7): built by [`build_runbook_proof_input`] from
///   `entry.compiled_runbook_id.is_some()` — real, INV-3-enforced. Declares
///   real predecessors (`gate::GATE_DEPENDENCIES`) matching
///   `ControlPlaneProof`'s own field list, so a `Success` here means every
///   proof that artefact would embed genuinely succeeded, not just that
///   the runbook reference happens to exist.
///
/// - G12 (version pinning, T9.7): built by [`build_version_pinning_input`]
///   — only `compiler_version` (`env!("CARGO_PKG_VERSION")`) has a real
///   source here; the other three `PinnedVersionSet` fields stay `None`.
///
/// `is_ai_originated`/`interpretation_attested` are conservatively `false`
/// (no attestation requirement applied) because this call site has no
/// Sage-pre-classification / intent-telemetry signal threaded through yet
/// (V&S §6.13.1's attestation source is net-new per T2.1's module doc) —
/// marking every intent as AI-originated without a real attestation signal
/// would make G1 fail unconditionally in shadow, which is not an honest
/// reflection of anything this call site actually observed.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_evaluation_context(
    envelope: &SemOsContextEnvelope,
    verb_fqn: &str,
    intent_id: Uuid,
    actor: &ActorContext,
    entity_binding: Option<ob_poc_control_plane::entity_binding::EntityBindingInput>,
    pack_resolution: Option<ob_poc_control_plane::pack_resolution::PackResolutionInput>,
    dag_proof: Option<ob_poc_control_plane::dag_proof::DagProofInput>,
    write_set: Option<ob_poc_control_plane::write_set::WriteSetInput>,
    stp_classifier: Option<ob_poc_control_plane::stp_classifier::StpClassifierInput>,
    snapshot: Option<ob_poc_control_plane::snapshot::SnapshotInput>,
    runbook_proof: Option<ob_poc_control_plane::proof::RunbookProofInput>,
    version_pinning: Option<ob_poc_control_plane::versioning::VersionPinningInput>,
) -> ob_poc_control_plane::context::EvaluationContext {
    let is_admitted = envelope.allowed_verbs.contains(verb_fqn);
    let exclusion_reasons = envelope
        .pruned_verbs
        .iter()
        .filter(|pruned| pruned.fqn == verb_fqn)
        .map(|pruned| format!("{:?}", pruned.reason))
        .collect();

    let abac_denied = envelope.pruned_verbs.iter().find(|pruned| {
        pruned.fqn == verb_fqn && matches!(pruned.reason, PruneReason::AbacDenied { .. })
    });
    let access_decision = if abac_denied.is_some() {
        ob_poc_control_plane::authority_gate::AccessDecisionKind::Deny
    } else {
        ob_poc_control_plane::authority_gate::AccessDecisionKind::Allow
    };
    let deny_reason = abac_denied.map(|pruned| format!("{:?}", pruned.reason));

    ob_poc_control_plane::context::EvaluationContext {
        entity_binding,
        pack_resolution,
        dag_proof,
        write_set,
        stp_classifier,
        snapshot,
        runbook_proof,
        version_pinning,
        intent_admission: Some(ob_poc_control_plane::intent_admission::IntentAdmissionInput {
            intent_id,
            verb_fqn: verb_fqn.to_string(),
            is_admitted,
            exclusion_reasons,
            is_ai_originated: false,
            interpretation_attested: false,
        }),
        authority: Some(ob_poc_control_plane::authority_gate::AuthorityInput {
            actor_id: actor.actor_id.clone(),
            role: actor.roles.join(","),
            access_decision,
            deny_reason,
            // Not runtime-observable at this call site (T4.3's verify_pins
            // has zero production call sites — see the ownership ledger);
            // `false` honestly means "no TOCTOU check occurred here", not
            // "no drift exists". Same posture for the three flags below —
            // no signal source exists yet, so they stay at their safe
            // (non-blocking) default rather than a guessed value.
            toctou_drifted: false,
            requires_human_approval: false,
            requires_second_line_review: false,
            segregation_of_duties_violated: false,
        }),
        evidence: Some(ob_poc_control_plane::evidence_gate::EvidenceInput {
            evidence_gaps: envelope.evidence_gaps.clone(),
            // No KYC-substrate adapter wired at this call site (T9.1d
            // scope) — most Path A dispatches are not KYC-domain verbs at
            // all. Leaving these empty is honest: it means "not observed
            // here", not "confirmed absent".
            kyc_precondition_failures: Vec::new(),
            satisfied_obligation_ids: Vec::new(),
            open_obligation_ids: Vec::new(),
        }),
        ..Default::default()
    }
}

/// One row for `"ob-poc".control_plane_shadow_decisions`.
///
/// `pub` (not `pub(crate)`) since G5 (`EOP-PLAN-CONTROLPLANE-GRADUATION-001`
/// §3 item 4): `ob-poc-web`'s `bus_runtime.rs` (Path D's adapter, a
/// different crate) needs to build and persist a shadow-decision row for
/// bus-federated dispatch, the same shape Path A already persists. This is
/// a deliberate, disclosed pub-surface widening (flagged in the G5 session
/// doc's blind-review summary, not silently absorbed) — the alternative
/// (duplicating the row shape / INSERT statement inside `ob-poc-web`)
/// would be exactly the "parallel, redundant tracking mechanism" the
/// mission brief said not to build.
#[derive(Debug, Clone)]
pub struct ShadowDecisionRow {
    pub session_id: Uuid,
    pub entry_id: Uuid,
    pub verb_fqn: String,
    pub gate_results: serde_json::Value,
    pub legacy_outcome_blocked: bool,
    pub shadow_intent_admission_blocked: bool,
    pub diverged: bool,
    /// G5 (migration `20260713_control_plane_shadow_decisions_execution_path.sql`):
    /// which `ExecutionPath` this decision was evaluated under. Every
    /// caller must now pass this explicitly (no more implicit
    /// Path-A-only assumption) — Path A's own call sites pass
    /// `ExecutionPath::RunbookSequencer` like every other caller, not a
    /// default.
    pub execution_path: ob_poc_types::ExecutionPath,
}

/// Serialises an `EvaluationReport` into the `gate_results` JSONB column:
/// `{"IntentAdmission": "Success", "PackResolution": "NotEvaluated { blocked_by: [...] }", ...}`.
///
/// `pub(crate)` (not `pub`): the sole non-`#[cfg(test)]` external caller is
/// `control_plane_audit`'s `rederivation_matches_evaluate_with_report_on_a_fully_admitted_context`
/// test, which needs the exact same JSONB shape `insert_shadow_decision`
/// persists in order to cross-check DD-4(ii)'s re-derivation against a
/// real `evaluate_with_report` output, not a hand-copied fixture.
pub(crate) fn report_to_json(report: &ob_poc_control_plane::gate::EvaluationReport) -> serde_json::Value {
    let map: serde_json::Map<String, serde_json::Value> = ob_poc_control_plane::gate::GateId::ALL
        .iter()
        .map(|id| {
            let rendered = report
                .get(*id)
                .map(|result| format!("{result:?}"))
                .unwrap_or_else(|| "missing".to_string());
            (format!("{id:?}"), serde_json::Value::String(rendered))
        })
        .collect();
    serde_json::Value::Object(map)
}

/// Builds the persistable row: compares the shadow G1 outcome against the
/// legacy Phase 5 recheck's block/allow decision for this entry.
pub fn build_shadow_decision_row(
    session_id: Uuid,
    entry_id: Uuid,
    verb_fqn: &str,
    report: &ob_poc_control_plane::gate::EvaluationReport,
    legacy_outcome_blocked: bool,
    execution_path: ob_poc_types::ExecutionPath,
) -> ShadowDecisionRow {
    let shadow_intent_admission_blocked = !matches!(
        report.get(ob_poc_control_plane::gate::GateId::IntentAdmission),
        Some(&ob_poc_control_plane::gate::GateResult::Success)
    );

    ShadowDecisionRow {
        session_id,
        entry_id,
        verb_fqn: verb_fqn.to_string(),
        gate_results: report_to_json(report),
        legacy_outcome_blocked,
        shadow_intent_admission_blocked,
        diverged: shadow_intent_admission_blocked != legacy_outcome_blocked,
        execution_path,
    }
}

/// Best-effort insert. Never returns `Err` — a shadow-decision persistence
/// failure must not affect the request it was observing.
#[cfg(feature = "database")]
pub async fn insert_shadow_decision(pool: &sqlx::PgPool, row: &ShadowDecisionRow) -> bool {
    let result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".control_plane_shadow_decisions (
            session_id, entry_id, verb_fqn, gate_results,
            legacy_outcome_blocked, shadow_intent_admission_blocked, diverged,
            execution_path
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(row.session_id)
    .bind(row.entry_id)
    .bind(&row.verb_fqn)
    .bind(&row.gate_results)
    .bind(row.legacy_outcome_blocked)
    .bind(row.shadow_intent_admission_blocked)
    .bind(row.diverged)
    .bind(row.execution_path.as_letter())
    .execute(pool)
    .await;

    match result {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!(
                error = %err,
                entry_id = %row.entry_id,
                "control_plane_shadow_decisions insert failed (best-effort, non-blocking)"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::sem_os_context_envelope::PrunedVerb;

    fn test_actor() -> ActorContext {
        ActorContext {
            actor_id: "actor-1".to_string(),
            roles: vec!["compliance_officer".to_string()],
            department: None,
            clearance: None,
            jurisdictions: Vec::new(),
        }
    }

    #[test]
    fn admitted_verb_builds_true_is_admitted() {
        let envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None, None, None, None, None, None);
        let input = ctx.intent_admission.expect("intent_admission set");
        assert!(input.is_admitted);
        assert!(input.exclusion_reasons.is_empty());
    }

    #[test]
    fn pruned_verb_carries_stringified_reason() {
        let envelope = SemOsContextEnvelope::test_with_verbs_and_pruned(
            &[],
            vec![PrunedVerb {
                fqn: "cbu.confirm".to_string(),
                reason: PruneReason::AgentModeBlocked {
                    mode: "read_only".to_string(),
                },
            }],
        );
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None, None, None, None, None, None);
        let input = ctx.intent_admission.expect("intent_admission set");
        assert!(!input.is_admitted);
        assert_eq!(input.exclusion_reasons.len(), 1);
        assert!(input.exclusion_reasons[0].contains("AgentModeBlocked"));
    }

    #[test]
    fn abac_denied_prune_reason_maps_to_authority_deny() {
        let envelope = SemOsContextEnvelope::test_with_verbs_and_pruned(
            &[],
            vec![PrunedVerb {
                fqn: "cbu.confirm".to_string(),
                reason: PruneReason::AbacDenied {
                    actor_role: "viewer".to_string(),
                    required: "compliance_officer".to_string(),
                },
            }],
        );
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None, None, None, None, None, None);
        let input = ctx.authority.expect("authority set");
        assert_eq!(
            input.access_decision,
            ob_poc_control_plane::authority_gate::AccessDecisionKind::Deny
        );
        assert!(input.deny_reason.is_some());
    }

    #[test]
    fn no_abac_denial_maps_to_authority_allow() {
        let envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None, None, None, None, None, None);
        let input = ctx.authority.expect("authority set");
        assert_eq!(
            input.access_decision,
            ob_poc_control_plane::authority_gate::AccessDecisionKind::Allow
        );
        assert!(input.deny_reason.is_none());
    }

    #[test]
    fn evidence_gaps_thread_through_from_envelope() {
        let mut envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        envelope.evidence_gaps = vec!["missing_source_of_wealth".to_string()];
        let ctx = build_evaluation_context(&envelope, "cbu.confirm", Uuid::nil(), &test_actor(), None, None, None, None, None, None, None, None);
        let input = ctx.evidence.expect("evidence set");
        assert_eq!(input.evidence_gaps, vec!["missing_source_of_wealth".to_string()]);
    }

    #[test]
    fn divergence_flagged_when_shadow_and_legacy_disagree() {
        let ctx = ob_poc_control_plane::context::EvaluationContext {
            intent_admission: Some(ob_poc_control_plane::intent_admission::IntentAdmissionInput {
                intent_id: Uuid::nil(),
                verb_fqn: "cbu.confirm".to_string(),
                is_admitted: true,
                exclusion_reasons: vec![],
                is_ai_originated: false,
                interpretation_attested: false,
            }),
            ..Default::default()
        };
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        // Shadow says admitted (not blocked); legacy says blocked -> diverged.
        let row = build_shadow_decision_row(Uuid::nil(), Uuid::nil(), "cbu.confirm", &report, true, ob_poc_types::ExecutionPath::RunbookSequencer);
        assert!(!row.shadow_intent_admission_blocked);
        assert!(row.diverged);

        // Legacy agrees (not blocked) -> no divergence.
        let row = build_shadow_decision_row(Uuid::nil(), Uuid::nil(), "cbu.confirm", &report, false, ob_poc_types::ExecutionPath::RunbookSequencer);
        assert!(!row.diverged);
    }

    // ── T9.1-pre (Addendum B): entity_binding_requests + G2 reachability ──

    fn verb_config_with_entity_arg(
        verb_fqn: &str,
        arg_name: &str,
        entity_type: &str,
    ) -> VerbConfigIndex {
        use crate::repl::verb_config_index::{ArgSummary, VerbIndexEntry};
        let mut index = VerbConfigIndex::empty();
        index.insert_test_entry(VerbIndexEntry {
            fqn: verb_fqn.to_string(),
            description: String::new(),
            invocation_phrases: Vec::new(),
            sentence_templates: Vec::new(),
            sentences: None,
            args: vec![ArgSummary {
                name: arg_name.to_string(),
                arg_type: "uuid".to_string(),
                required: true,
                description: None,
                maps_to: None,
                lookup_entity_type: Some(entity_type.to_string()),
            }],
            crud_key: None,
            confirm_policy: crate::repl::runbook::ConfirmPolicy::Always,
            precondition_checks: Vec::new(),
        });
        index
    }

    #[test]
    fn entity_binding_requests_finds_contract_typed_arg() {
        let index = verb_config_with_entity_arg("cbu.confirm", "cbu-id", "cbu");
        let id = Uuid::new_v4();
        let mut args = HashMap::new();
        args.insert("cbu-id".to_string(), id.to_string());

        let requests = entity_binding_requests(&index, "cbu.confirm", &args);
        assert_eq!(requests, vec![(id, "cbu".to_string())]);
    }

    #[test]
    fn entity_binding_requests_skips_unresolved_and_non_uuid_values() {
        let index = verb_config_with_entity_arg("cbu.confirm", "cbu-id", "cbu");

        // Arg not present in entry_args at all.
        let requests = entity_binding_requests(&index, "cbu.confirm", &HashMap::new());
        assert!(requests.is_empty());

        // Arg present but not a UUID (unresolved symbol placeholder).
        let mut args = HashMap::new();
        args.insert("cbu-id".to_string(), "@some-symbol".to_string());
        let requests = entity_binding_requests(&index, "cbu.confirm", &args);
        assert!(requests.is_empty());
    }

    #[test]
    fn entity_binding_requests_empty_for_verb_with_no_entity_args() {
        let index = VerbConfigIndex::empty();
        let requests = entity_binding_requests(&index, "session.info", &HashMap::new());
        assert!(requests.is_empty());
    }

    #[test]
    fn build_entity_binding_input_marks_missing_facts_as_not_found() {
        let id = Uuid::new_v4();
        let requests = vec![(id, "cbu".to_string())];
        let facts = HashMap::new(); // batched lookup found nothing for `id`
        let input = build_entity_binding_input(&requests, &facts);
        assert_eq!(input.entities.len(), 1);
        assert!(!input.entities[0].exists);
        assert_eq!(input.entities[0].expected_kind, "cbu");
    }

    #[test]
    fn empty_entity_binding_input_is_vacuous_success_not_failure() {
        // The doc-corrected contract this test locks in: a verb with zero
        // entity-typed args must pass G2 (Some(entities: vec![]) ->
        // vacuous Success), not fail it via a spurious None.
        let envelope = SemOsContextEnvelope::test_with_verbs(&["session.info"]);
        let entity_binding = Some(ob_poc_control_plane::entity_binding::EntityBindingInput {
            entities: Vec::new(),
        });
        let ctx = build_evaluation_context(
            &envelope,
            "session.info",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::EntityBinding),
            Some(&ob_poc_control_plane::gate::GateResult::Success)
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn g2_reaches_success_end_to_end_against_a_real_cbu_row() {
        // Empirical reachability proof (this session's established
        // discipline — verified via evaluate_shadow, not assumed from
        // reading GATE_DEPENDENCIES): contract-typed arg detection ->
        // real batched DB fetch -> EntityBindingInput -> evaluate_shadow
        // actually reports G2 Success, for a verb whose only prerequisite
        // (per GATE_DEPENDENCIES, EntityBinding has none) is satisfied.
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");

        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("at least one cbu row exists in the dev database");

        let index = verb_config_with_entity_arg("test.verb-with-cbu-arg", "cbu-id", "cbu");
        let mut args = HashMap::new();
        args.insert("cbu-id".to_string(), cbu_id.to_string());

        let requests = entity_binding_requests(&index, "test.verb-with-cbu-arg", &args);
        assert_eq!(requests, vec![(cbu_id, "cbu".to_string())]);

        let source = ob_poc_boundary::entity_facts::PgEntityFactsSource { pool: &pool };
        let facts = ob_poc_boundary::entity_facts::EntityFactsSource::entity_facts(&source, &requests)
            .await
            .expect("batched fetch succeeds");

        let entity_binding = Some(build_entity_binding_input(&requests, &facts));
        let envelope = SemOsContextEnvelope::test_with_verbs(&["test.verb-with-cbu-arg"]);
        let ctx = build_evaluation_context(
            &envelope,
            "test.verb-with-cbu-arg",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::EntityBinding),
            Some(&ob_poc_control_plane::gate::GateResult::Success),
            "G2 must report a real, non-not_evaluated Success against a real cbu row"
        );
    }

    /// Historical snapshot (T9.1-pre, before T9.1a landed): with G2 real
    /// but `pack_resolution: None` explicitly passed, PackResolution
    /// reports its own genuine `Failure("no PackResolutionInput
    /// supplied")` rather than staying blocked by EntityBinding, and
    /// Authority/Evidence are blocked *solely* by PackResolution — this
    /// is what motivated T9.1a. Kept as a regression check on the
    /// `pack_resolution: None` code path specifically (a caller that
    /// can't or doesn't supply pack data), not a claim that G3 is
    /// globally unwired — see `g3_reaches_success_and_unblocks_authority_evidence`
    /// below for the now-real end-to-end path.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn g3_none_leaves_authority_and_evidence_blocked() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");
        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("cbu row exists");

        let index = verb_config_with_entity_arg("cbu.confirm", "cbu-id", "cbu");
        let mut args = std::collections::HashMap::new();
        args.insert("cbu-id".to_string(), cbu_id.to_string());
        let requests = entity_binding_requests(&index, "cbu.confirm", &args);
        let source = ob_poc_boundary::entity_facts::PgEntityFactsSource { pool: &pool };
        let facts = ob_poc_boundary::entity_facts::EntityFactsSource::entity_facts(&source, &requests)
            .await
            .unwrap();
        let entity_binding = Some(build_entity_binding_input(&requests, &facts));

        let envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        let ctx = build_evaluation_context(
            &envelope,
            "cbu.confirm",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::EntityBinding),
            Some(&ob_poc_control_plane::gate::GateResult::Success)
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::Authority),
            Some(&ob_poc_control_plane::gate::GateResult::NotEvaluated {
                blocked_by: vec![ob_poc_control_plane::gate::GateId::PackResolution],
            })
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::Evidence),
            Some(&ob_poc_control_plane::gate::GateResult::NotEvaluated {
                blocked_by: vec![ob_poc_control_plane::gate::GateId::PackResolution],
            })
        );
    }

    // ── T9.1a (Addendum B): build_pack_resolution_input ────────────────────

    fn test_pack_manifest(pack_id: &str, allowed_verbs: Vec<&str>) -> PackManifest {
        PackManifest {
            id: pack_id.to_string(),
            name: pack_id.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            invocation_phrases: Vec::new(),
            required_context: Vec::new(),
            optional_context: Vec::new(),
            workspaces: Vec::new(),
            allowed_verbs: allowed_verbs.into_iter().map(String::from).collect(),
            forbidden_verbs: Vec::new(),
            risk_policy: Default::default(),
            required_questions: Vec::new(),
            optional_questions: Vec::new(),
            stop_rules: Vec::new(),
            templates: Vec::new(),
            pack_summary_template: None,
            section_layout: Vec::new(),
            definition_of_done: Vec::new(),
            progress_signals: Vec::new(),
            handoff_target: None,
        }
    }

    #[test]
    fn no_active_pack_yields_missing_pack_candidates() {
        let input = build_pack_resolution_input(None, "cbu.confirm", true);
        assert!(input.candidate_pack_ids.is_empty());
        assert!(!input.constraint_denies_intent);
        assert!(input.semreg_allowed_set_available);
    }

    #[test]
    fn active_pack_allowing_the_verb_resolves_it_as_candidate() {
        let manifest = test_pack_manifest("cbu-maintenance", vec!["cbu.confirm"]);
        let input = build_pack_resolution_input(Some(("cbu-maintenance", &manifest)), "cbu.confirm", true);
        assert_eq!(input.candidate_pack_ids, vec!["cbu-maintenance".to_string()]);
        assert!(!input.constraint_denies_intent);
    }

    #[test]
    fn active_pack_not_declaring_the_verb_yields_no_candidates() {
        let manifest = test_pack_manifest("cbu-maintenance", vec!["cbu.confirm"]);
        let input = build_pack_resolution_input(Some(("cbu-maintenance", &manifest)), "kyc-case.approve", true);
        assert!(input.candidate_pack_ids.is_empty());
        assert!(!input.constraint_denies_intent);
    }

    #[test]
    fn active_pack_forbidding_the_verb_yields_no_candidates() {
        let mut manifest = test_pack_manifest("cbu-maintenance", vec![]); // unconstrained allowed set
        manifest.forbidden_verbs = vec!["cbu.confirm".to_string()];
        let input = build_pack_resolution_input(Some(("cbu-maintenance", &manifest)), "cbu.confirm", true);
        assert!(input.candidate_pack_ids.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn g3_reaches_success_and_unblocks_authority_evidence() {
        // Empirical reachability proof (this session's established
        // discipline): with G2 real (T9.1-pre) and G3 now real (T9.1a),
        // verify via evaluate_shadow — not assumed from GATE_DEPENDENCIES
        // — that Authority/Evidence stop being NotEvaluated once both
        // their prerequisites genuinely succeed.
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");
        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("cbu row exists");

        let index = verb_config_with_entity_arg("cbu.confirm", "cbu-id", "cbu");
        let mut args = std::collections::HashMap::new();
        args.insert("cbu-id".to_string(), cbu_id.to_string());
        let requests = entity_binding_requests(&index, "cbu.confirm", &args);
        let source = ob_poc_boundary::entity_facts::PgEntityFactsSource { pool: &pool };
        let facts = ob_poc_boundary::entity_facts::EntityFactsSource::entity_facts(&source, &requests)
            .await
            .unwrap();
        let entity_binding = Some(build_entity_binding_input(&requests, &facts));

        let manifest = test_pack_manifest("cbu-maintenance", vec!["cbu.confirm"]);
        let pack_resolution = Some(build_pack_resolution_input(
            Some(("cbu-maintenance", &manifest)),
            "cbu.confirm",
            true,
        ));

        let envelope = SemOsContextEnvelope::test_with_verbs(&["cbu.confirm"]);
        let ctx = build_evaluation_context(
            &envelope,
            "cbu.confirm",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            pack_resolution,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::EntityBinding),
            Some(&ob_poc_control_plane::gate::GateResult::Success)
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::PackResolution),
            Some(&ob_poc_control_plane::gate::GateResult::Success)
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::Authority),
            Some(&ob_poc_control_plane::gate::GateResult::Success),
            "Authority must reach a real outcome now that its declared dependencies (IntentAdmission, PackResolution) both succeed"
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::Evidence),
            Some(&ob_poc_control_plane::gate::GateResult::Success),
            "Evidence must reach a real outcome now that its declared dependencies (EntityBinding, PackResolution) both succeed"
        );
    }

    // ── T9.1b (Addendum B): build_dag_proof_input ──────────────────────

    #[tokio::test]
    async fn build_dag_proof_input_none_when_no_gate_pipeline() {
        let args = HashMap::new();
        let dag_proof = build_dag_proof_input(None, "cbu.confirm", &args).await;
        assert!(dag_proof.is_none(), "no GatePipeline wired -> nothing to observe, not an error");
    }

    /// Minimal self-contained GatePipeline fixture — no live DB, no
    /// `harness` feature, same in-memory pattern
    /// `step_executor_bridge`'s equivalence tests use.
    struct FixedSlotState(std::collections::HashMap<(String, String, Uuid), Option<String>>);

    #[async_trait::async_trait]
    impl dsl_runtime::cross_workspace::SlotStateProvider for FixedSlotState {
        async fn read_slot_state(
            &self,
            workspace: &str,
            slot: &str,
            entity_id: Uuid,
            _pool: &sqlx::PgPool,
        ) -> anyhow::Result<Option<String>> {
            Ok(self
                .0
                .get(&(workspace.to_string(), slot.to_string(), entity_id))
                .cloned()
                .unwrap_or(None))
        }
    }

    struct FixedLookup(Option<dsl_core::TransitionArgs>);

    impl crate::runbook::step_executor_bridge::VerbTransitionLookup for FixedLookup {
        fn lookup(&self, _verb_fqn: &str) -> Option<dsl_core::TransitionArgs> {
            self.0.clone()
        }
    }

    const TEST_DAG_YAML: &str = r#"
workspace: testws
dag_id: test_dag
slots:
  - id: testslot
    stateless: false
    state_machine:
      id: sm
      states: [{ id: FROM, entry: true }, { id: TO }]
      transitions:
        - from: FROM
          to: TO
          via: test.transition-verb
cross_workspace_constraints: []
"#;

    fn test_gate_pipeline() -> crate::runbook::step_executor_bridge::GatePipeline {
        let dir = std::env::temp_dir().join(format!("t91b_shadow_test_dag_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("test.yaml"), TEST_DAG_YAML).unwrap();
        let registry =
            std::sync::Arc::new(dsl_runtime::cross_workspace::DagRegistry::from_dir(&dir).unwrap());
        std::fs::remove_dir_all(&dir).ok();

        let gate_checker = std::sync::Arc::new(dsl_runtime::GateChecker::new(
            registry.clone(),
            std::sync::Arc::new(FixedSlotState(Default::default())),
            std::sync::Arc::new(dsl_runtime::cross_workspace::SameEntityResolver),
        ));
        let verb_metadata: std::sync::Arc<dyn crate::runbook::step_executor_bridge::VerbTransitionLookup> =
            std::sync::Arc::new(FixedLookup(Some(dsl_core::TransitionArgs {
                entity_id_arg: "entity-id".into(),
                target_state_arg: None,
                target_workspace: Some("testws".into()),
                target_slot: Some("testslot".into()),
            })));
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://harness-mock-never-connects")
            .expect("connect_lazy with a valid-shaped URL never fails");

        crate::runbook::step_executor_bridge::GatePipeline {
            registry,
            gate_checker,
            verb_metadata,
            pool: std::sync::Arc::new(pool),
            cascade_planner: None,
        }
    }

    #[tokio::test]
    async fn g4_reaches_success_end_to_end_against_a_fixture_dag() {
        // Empirical reachability proof (this session's established
        // discipline, matching g2_reaches_success/g3_reaches_success
        // above): build_dag_proof_input -> build_evaluation_context ->
        // evaluate_shadow actually reports G4 Success for a verb whose
        // declared transition_args resolve cleanly against a legal
        // transition with no blocking violations.
        let pipe = test_gate_pipeline();
        let entity_id = Uuid::new_v4();
        let mut args = HashMap::new();
        args.insert("entity-id".to_string(), entity_id.to_string());

        let dag_proof = build_dag_proof_input(Some(&pipe), "test.transition-verb", &args)
            .await
            .expect("verb has transition_args and a matching DAG transition");
        assert_eq!(dag_proof.entity_id, entity_id);
        assert!(dag_proof.blocking_violations.is_empty());

        // G4 depends on EntityBinding + PackResolution (GATE_DEPENDENCIES)
        // — both must genuinely succeed too, or G4 stays NotEvaluated
        // regardless of dag_proof's own content.
        let entity_binding = Some(ob_poc_control_plane::entity_binding::EntityBindingInput {
            entities: Vec::new(),
        });
        let pack_resolution = Some(ob_poc_control_plane::pack_resolution::PackResolutionInput {
            candidate_pack_ids: vec!["fixture-pack".to_string()],
            semreg_allowed_set_available: true,
            constraint_denies_intent: false,
        });

        let envelope = SemOsContextEnvelope::test_with_verbs(&["test.transition-verb"]);
        let ctx = build_evaluation_context(
            &envelope,
            "test.transition-verb",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            pack_resolution,
            Some(dag_proof),
            None,
            None,
            None,
            None,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::DagProof),
            Some(&ob_poc_control_plane::gate::GateResult::Success),
            "G4 must report a real, non-not_evaluated Success against a legal DAG transition"
        );
    }

    #[tokio::test]
    async fn build_dag_proof_input_none_when_dag_has_no_matching_transition() {
        let pipe = test_gate_pipeline();
        let mut args = HashMap::new();
        args.insert("entity-id".to_string(), Uuid::new_v4().to_string());
        // FixedLookup returns Some(transition_args) for every verb_fqn
        // (it doesn't discriminate), but the fixture DAG only declares a
        // transition `via: test.transition-verb` — "unrelated.verb" has
        // no matching TransitionRef, so candidates come back empty ->
        // None, exactly like a real verb with transition_args declared
        // but no matching DAG transition.
        let dag_proof = build_dag_proof_input(Some(&pipe), "unrelated.verb", &args).await;
        assert!(dag_proof.is_none());
    }

    // ── T9.1e (Addendum B): build_write_set_input ──────────────────────

    fn test_domain_metadata(verb_fqn: &str, writes: Vec<&str>) -> sem_os_obpoc_adapter::metadata::DomainMetadata {
        let yaml = format!(
            r#"
domains:
  test:
    description: test domain
    verb_data_footprint:
      {verb_fqn}:
        writes: [{writes}]
"#,
            writes = writes.join(", ")
        );
        sem_os_obpoc_adapter::metadata::DomainMetadata::from_yaml(&yaml).expect("valid fixture YAML")
    }

    #[test]
    fn real_domain_metadata_yaml_loads_and_has_at_least_one_write_footprint() {
        // Regression guard, not just a fixture check: the real
        // config/sem_os_seeds/domain_metadata.yaml this loader reads at
        // ob-poc-web startup must actually parse and carry at least one
        // non-empty writes: [...] entry, or T9.1e's wiring is silently
        // dead in production (build_write_set_input always None).
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("config/sem_os_seeds/domain_metadata.yaml");
        let metadata = sem_os_obpoc_adapter::metadata::DomainMetadata::from_file(&path)
            .expect("real domain_metadata.yaml must parse");
        let has_a_write_footprint = metadata.domains.values().any(|domain| {
            domain
                .verb_data_footprint
                .values()
                .any(|footprint| !footprint.writes.is_empty())
        });
        assert!(
            has_a_write_footprint,
            "real domain_metadata.yaml has zero non-empty writes: [...] entries — T9.1e's wiring would be silently dead"
        );
    }

    // ── G2 item 2: derive_crud_allowed_columns ──

    #[test]
    fn derive_crud_allowed_columns_none_for_unregistered_verb() {
        let ws = build_write_set_input(
            Some(&test_domain_metadata("nonexistent-domain.nonexistent-verb", vec!["\"t\""])),
            "nonexistent-domain.nonexistent-verb",
            Uuid::new_v4(),
            vec![],
        )
        .expect("footprint declares a write");
        // No entry in runtime_registry() for this FQN -> derivation falls
        // back to the honest empty default, same as before this session.
        assert!(ws.allowed_columns.is_empty());
    }

    #[test]
    fn derive_crud_allowed_columns_real_insert_verb_with_explicit_returning() {
        // `capability-binding.draft` (config/verbs/capability-binding.yaml):
        // operation: insert, returning: id, args mapping to
        // application_instance_id / service_id / notes. This is the exact
        // shape `crud_executor.rs::execute_insert` reads to build its SQL
        // and to self-report `record_write`'s raw_columns (which always
        // starts with the pk column, per that function's own
        // `let mut raw_columns = vec![pk_col.to_string()]`).
        let rv = crate::dsl_v2::runtime_registry::runtime_registry()
            .get("capability-binding", "draft")
            .expect("capability-binding.draft must be registered — verb YAML moved?");
        let derived = derive_crud_allowed_columns(rv)
            .expect("Insert with explicit `returning` must derive a non-empty set");
        let mut expected = vec![
            "application_instance_id".to_string(),
            "service_id".to_string(),
            "notes".to_string(),
            "id".to_string(),
        ];
        expected.sort();
        assert_eq!(derived, expected);
    }

    #[test]
    fn derive_allowed_columns_for_operation_none_for_delete() {
        // Delete must return None regardless of maps_to contents — this
        // session did not mirror execute_delete's soft-delete `deleted_at`
        // literal-column write, and a maps_to-only derivation would
        // silently under-declare it (the unsafe direction).
        assert!(derive_allowed_columns_for_operation(
            dsl_core::CrudOperation::Delete,
            None,
            vec!["thing_id".to_string()],
        )
        .is_none());
    }

    #[test]
    fn derive_allowed_columns_for_operation_none_for_link_and_unlink() {
        assert!(derive_allowed_columns_for_operation(dsl_core::CrudOperation::Link, None, vec![]).is_none());
        assert!(derive_allowed_columns_for_operation(dsl_core::CrudOperation::Unlink, None, vec![]).is_none());
    }

    #[test]
    fn derive_allowed_columns_for_operation_none_for_insert_without_explicit_returning() {
        // infer_pk_column's heuristic is not re-verified by this
        // derivation — an Insert/Upsert verb with no explicit `returning`
        // must fail closed to None, not guess.
        assert!(derive_allowed_columns_for_operation(
            dsl_core::CrudOperation::Insert,
            None,
            vec!["status".to_string()],
        )
        .is_none());
        assert!(derive_allowed_columns_for_operation(
            dsl_core::CrudOperation::Upsert,
            None,
            vec!["status".to_string()],
        )
        .is_none());
    }

    #[test]
    fn derive_allowed_columns_for_operation_insert_includes_pk_and_maps_to() {
        let derived = derive_allowed_columns_for_operation(
            dsl_core::CrudOperation::Insert,
            Some("id".to_string()),
            vec!["status".to_string(), "legal_name".to_string()],
        )
        .expect("explicit returning + non-empty maps_to must derive");
        let mut expected = vec!["id".to_string(), "status".to_string(), "legal_name".to_string()];
        expected.sort();
        assert_eq!(derived, expected);
    }

    #[test]
    fn derive_allowed_columns_for_operation_update_none_when_no_mapped_columns() {
        // A verb with zero maps_to columns can never legitimately SET
        // anything — None (cannot derive), not a fabricated empty Some.
        assert!(derive_allowed_columns_for_operation(dsl_core::CrudOperation::Update, None, vec![]).is_none());
    }

    #[test]
    fn derive_allowed_columns_for_operation_update_excludes_pk_requirement() {
        // Update never needs `returning` — the key column is a WHERE
        // bind, not a SET column, so no pk-guessing risk applies here.
        let derived = derive_allowed_columns_for_operation(
            dsl_core::CrudOperation::Update,
            None,
            vec!["status".to_string()],
        )
        .expect("non-empty maps_to must derive");
        assert_eq!(derived, vec!["status".to_string()]);
    }

    /// Supersedes the prior session's
    /// `derived_columns_are_correct_but_table_name_format_does_not_match_captured_writes`
    /// (`EOP-SESSION-CONTROLPLANE-G1-ITEM2-G2-ITEM2-IMPL-001`), which
    /// proved a genuinely legitimate write on a correctly-derived column
    /// still misclassified as a breach because `WriteSetInput.tables` was
    /// a bare table name (`"capability_bindings"`) while `CapturedWrite`
    /// carries `record_write`'s real `"{schema}.{table}"` format
    /// (`"ob-poc.capability_bindings"`). `build_write_set_input` now
    /// schema-qualifies via `qualify_footprint_table` (this session, G2
    /// item 4) — this test proves the same scenario now attests `Bounded`,
    /// not `Breach`.
    #[test]
    fn derived_columns_are_correct_and_table_name_now_matches_captured_writes() {
        let metadata = test_domain_metadata("capability-binding.draft", vec!["\"capability_bindings\""]);
        let ws = build_write_set_input(Some(&metadata), "capability-binding.draft", Uuid::new_v4(), vec![Uuid::nil()])
            .expect("footprint declares a write");
        assert!(
            !ws.allowed_columns.is_empty(),
            "sanity: the column derivation itself must be non-empty here"
        );
        // Schema-qualified now, matching crud_executor.rs's real
        // record_write format exactly.
        assert_eq!(ws.tables, vec!["ob-poc.capability_bindings".to_string()]);

        let proof = ob_poc_control_plane::write_set::tests_support::proof(
            vec![Uuid::nil()],
            ws.state_slots.clone(),
            ws.tables.clone(),
            ws.allowed_columns.clone(),
            &ws.idempotency_key,
        );
        // The real capture format record_write uses in production
        // (crud_executor.rs: `format!("{schema}.{table}")`).
        let captured = vec![ob_poc_control_plane::write_set_attestation::CapturedWrite {
            table: "ob-poc.capability_bindings".to_string(),
            entity_id: Uuid::nil(),
            columns: vec!["service_id".to_string()], // a genuinely-declared, legitimate column
        }];
        let outcome = ob_poc_control_plane::write_set_attestation::attest(&captured, &proof);
        assert_eq!(
            outcome,
            ob_poc_control_plane::write_set_attestation::AttestationOutcome::Bounded,
            "a fully legitimate write on a correctly-derived column and now-matching \
             schema-qualified table must attest Bounded: {outcome:?}"
        );
    }

    /// G2 item 4: `qualify_footprint_table` schema-qualifies a bare
    /// `domain_metadata.yaml` table name with the documented default
    /// schema (`ob-poc`), matching `crud_executor.rs`'s own
    /// `crud.schema.as_deref().unwrap_or("ob-poc")` default.
    #[test]
    fn qualify_footprint_table_bare_name_defaults_to_ob_poc() {
        assert_eq!(qualify_footprint_table("deals"), "ob-poc.deals");
        assert_eq!(qualify_footprint_table("capability_bindings"), "ob-poc.capability_bindings");
    }

    /// A table name that already carries a `schema.table` prefix (per
    /// `domain_metadata.yaml`'s own documented convention) is trusted
    /// verbatim, not re-qualified — this is the real, correct form for
    /// the `sem_reg.*`/`sem_reg_authoring.*`/`sem_reg_pub.*` family, which
    /// are genuine Postgres schemas (confirmed against
    /// `migrations/master-schema.sql`'s `CREATE SCHEMA` list).
    #[test]
    fn qualify_footprint_table_already_dotted_name_passes_through_verbatim() {
        assert_eq!(qualify_footprint_table("sem_reg.changesets"), "sem_reg.changesets");
        assert_eq!(
            qualify_footprint_table("sem_reg_authoring.governance_audit_log"),
            "sem_reg_authoring.governance_audit_log"
        );
    }

    /// Known, pre-existing, NOT fixed by this session (documented in
    /// `qualify_footprint_table`'s own doc comment): `domain_metadata.yaml`
    /// uses a `kyc.` prefix for `kyc.set-case`'s footprint even though no
    /// `kyc` Postgres schema exists — the real table is
    /// `"ob-poc".cases`. This function trusts any dotted name verbatim
    /// (correct for the `sem_reg*` family), so this specific entry stays
    /// wrong until `domain_metadata.yaml`'s own data is corrected — a
    /// separate, narrower follow-up flagged in the session doc, not
    /// silently swept under this fix.
    #[test]
    fn qualify_footprint_table_known_gap_kyc_prefix_is_not_a_real_schema() {
        let qualified = qualify_footprint_table("kyc.cases");
        assert_eq!(qualified, "kyc.cases", "documents the pass-through behavior as-is");
        assert_ne!(
            qualified, "ob-poc.cases",
            "the real table (per master-schema.sql: CREATE TABLE \"ob-poc\".cases) — \
             this assertion documents the known gap, not a claim this session fixed it"
        );
    }

    #[test]
    fn build_write_set_input_none_when_no_domain_metadata() {
        let entry_id = Uuid::new_v4();
        let ws = build_write_set_input(None, "cbu.confirm", entry_id, vec![]);
        assert!(ws.is_none());
    }

    #[test]
    fn build_write_set_input_none_when_verb_has_no_footprint() {
        let metadata = test_domain_metadata("deal.create", vec!["\"deals\""]);
        let entry_id = Uuid::new_v4();
        let ws = build_write_set_input(Some(&metadata), "unrelated.verb", entry_id, vec![]);
        assert!(ws.is_none());
    }

    #[test]
    fn build_write_set_input_none_when_footprint_declares_no_writes() {
        let metadata = test_domain_metadata("cbu.show", vec![]);
        let entry_id = Uuid::new_v4();
        // Empty writes list -> None, not a fabricated CannotDerive: a
        // read-only verb legitimately writes nothing.
        let ws = build_write_set_input(Some(&metadata), "cbu.show", entry_id, vec![]);
        assert!(ws.is_none());
    }

    #[test]
    fn build_write_set_input_some_with_tables_when_footprint_declares_writes() {
        let metadata = test_domain_metadata("deal.create", vec!["\"deals\""]);
        let entry_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();
        let ws = build_write_set_input(Some(&metadata), "deal.create", entry_id, vec![entity_id])
            .expect("verb has a non-empty write footprint");
        // Schema-qualified (G2 item 4) — matches crud_executor.rs's real
        // record_write format, not the bare domain_metadata.yaml name.
        assert_eq!(ws.tables, vec!["ob-poc.deals".to_string()]);
        assert!(ws.contract_derived);
        assert_eq!(ws.entity_ids, vec![entity_id]);
        assert!(ws.state_slots.is_empty());
        assert!(ws.allowed_columns.is_empty());
        assert_eq!(ws.idempotency_key, format!("{entry_id}:deal.create"));
    }

    /// A verb footprint declaring multiple write tables (real pattern in
    /// `domain_metadata.yaml` — plugin verbs writing several tables in one
    /// call, e.g. `cbu.assign-role: writes: [cbus, cbu_entity_roles]`)
    /// must have every table schema-qualified, not just the first.
    #[test]
    fn build_write_set_input_qualifies_every_table_in_a_multi_table_footprint() {
        let metadata =
            test_domain_metadata("cbu.assign-role", vec!["\"cbus\"", "\"cbu_entity_roles\""]);
        let entry_id = Uuid::new_v4();
        let ws = build_write_set_input(Some(&metadata), "cbu.assign-role", entry_id, vec![])
            .expect("verb has a non-empty write footprint");
        assert_eq!(
            ws.tables,
            vec!["ob-poc.cbus".to_string(), "ob-poc.cbu_entity_roles".to_string()]
        );
    }

    // ── T9.5 (Addendum B): build_stp_classifier_input ──

    #[test]
    fn build_stp_classifier_input_false_for_unregistered_fqn() {
        // No entry for this made-up verb in the real runtime_registry() —
        // is_durable_verb must honestly report false, not panic or guess.
        let input = build_stp_classifier_input("nonexistent-domain.nonexistent-verb", false);
        assert!(!input.is_durable_verb);
        assert!(!input.durable_execution_explicitly_allowed);
        assert!(!input.has_unpinned_entities);
    }

    #[test]
    fn build_stp_classifier_input_false_for_malformed_fqn() {
        // No '.' separator at all -> split_once yields None -> honestly false.
        let input = build_stp_classifier_input("noperiod", true);
        assert!(!input.is_durable_verb);
        assert!(input.has_unpinned_entities);
    }

    #[test]
    fn build_stp_classifier_input_threads_has_unpinned_entities_from_caller() {
        let input = build_stp_classifier_input("cbu.confirm", true);
        assert!(input.has_unpinned_entities);
        let input = build_stp_classifier_input("cbu.confirm", false);
        assert!(!input.has_unpinned_entities);
    }

    #[test]
    fn build_stp_classifier_input_never_allows_durable_execution_at_this_call_site() {
        // Always false regardless of the durability finding — this call
        // site (phase5_runtime_recheck) is Path A's own REPL dispatch,
        // never a BPMN direct-worker context.
        let input = build_stp_classifier_input("cbu.confirm", false);
        assert!(!input.durable_execution_explicitly_allowed);
    }

    // ── T9.6 (Addendum B): build_decision_snapshot_input ──

    fn fixture_entity_facts_row(entity_id: Uuid, kind: &str, row_version: i64) -> ob_poc_boundary::entity_facts::EntityFactsRow {
        ob_poc_boundary::entity_facts::EntityFactsRow {
            facts: ob_poc_control_plane::entity_binding::EntityFacts {
                entity_id,
                exists: true,
                expected_kind: kind.to_string(),
                actual_kind: kind.to_string(),
                lifecycle_state_readable: true,
                availability_blocked: false,
                availability_reason: None,
                in_active_pack: true,
            },
            row_version,
        }
    }

    #[test]
    fn build_decision_snapshot_input_none_when_facts_fetch_not_attempted() {
        // Mirrors G2's own None-on-fetch-error posture — not a fabricated
        // empty-but-successful snapshot.
        assert!(build_decision_snapshot_input(None).is_none());
    }

    #[test]
    fn build_decision_snapshot_input_some_empty_when_no_entities_requested() {
        // Vacuous: nothing to pin, not a failed attempt — Some(default),
        // matching DecisionSnapshotGate's own "empty pins still succeed" law.
        let facts = HashMap::new();
        let input = build_decision_snapshot_input(Some(&facts)).expect("attempted, even if empty");
        assert!(input.entity_row_versions.is_empty());
        assert!(input.sem_reg_snapshot_id.is_none());
        assert!(input.session_snapshot_id.is_none());
        assert!(input.kyc_manifest_hash.is_none());
    }

    #[test]
    fn build_decision_snapshot_input_carries_real_row_versions() {
        let entity_id = Uuid::new_v4();
        let mut facts = HashMap::new();
        facts.insert(entity_id, fixture_entity_facts_row(entity_id, "cbu", 7));
        let input = build_decision_snapshot_input(Some(&facts)).expect("facts supplied");
        assert_eq!(input.entity_row_versions, vec![(entity_id, "cbu".to_string(), 7)]);
    }

    // ── G6b: has_unpinned_entities ──────────────────────────────────────

    #[test]
    fn has_unpinned_entities_false_for_no_requests() {
        assert!(!has_unpinned_entities(&[], None));
        assert!(!has_unpinned_entities(&[], Some(&HashMap::new())));
    }

    #[test]
    fn has_unpinned_entities_true_when_facts_fetch_failed() {
        let entity_id = Uuid::new_v4();
        let requests = vec![(entity_id, "cbu".to_string())];
        assert!(has_unpinned_entities(&requests, None));
    }

    #[test]
    fn has_unpinned_entities_true_when_entity_missing_from_facts() {
        let entity_id = Uuid::new_v4();
        let requests = vec![(entity_id, "cbu".to_string())];
        // Fetch succeeded but returned no row for this id (not found, or
        // an unsupported kind that the batched query silently omitted).
        assert!(has_unpinned_entities(&requests, Some(&HashMap::new())));
    }

    #[test]
    fn has_unpinned_entities_false_when_every_requested_entity_has_a_real_pin() {
        let entity_id = Uuid::new_v4();
        let requests = vec![(entity_id, "cbu".to_string())];
        let mut facts = HashMap::new();
        facts.insert(entity_id, fixture_entity_facts_row(entity_id, "cbu", 3));
        assert!(!has_unpinned_entities(&requests, Some(&facts)));
    }

    #[test]
    fn has_unpinned_entities_true_when_only_some_requested_entities_are_pinned() {
        let pinned = Uuid::new_v4();
        let unpinned = Uuid::new_v4();
        let requests = vec![(pinned, "cbu".to_string()), (unpinned, "entity".to_string())];
        let mut facts = HashMap::new();
        facts.insert(pinned, fixture_entity_facts_row(pinned, "cbu", 3));
        assert!(has_unpinned_entities(&requests, Some(&facts)));
    }

    // ── T9.7 (Addendum B): build_runbook_proof_input / build_version_pinning_input ──

    #[test]
    fn build_runbook_proof_input_threads_the_id_through() {
        let id = Uuid::new_v4();
        assert_eq!(build_runbook_proof_input(Some(id)).compiled_runbook_id, Some(id));
        assert_eq!(build_runbook_proof_input(None).compiled_runbook_id, None);
    }

    #[test]
    fn build_version_pinning_input_carries_a_real_compiler_version() {
        let input = build_version_pinning_input();
        assert_eq!(
            input.versions.compiler_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        assert!(input.versions.bus_catalogue_version.is_none());
        assert!(input.versions.model_version.is_none());
        assert!(input.versions.prompt_version.is_none());
    }

    #[tokio::test]
    async fn g7_reaches_success_end_to_end_given_a_legal_dag_transition() {
        // Empirical reachability proof, matching the g2/g3/g4 pattern:
        // build_write_set_input -> build_evaluation_context ->
        // evaluate_shadow actually reports G7 Success once its declared
        // dependency (G4/DagProof — GATE_DEPENDENCIES) genuinely succeeds
        // too, not just because write_set itself is populated.
        let pipe = test_gate_pipeline();
        let entity_id = Uuid::new_v4();
        let mut args = HashMap::new();
        args.insert("entity-id".to_string(), entity_id.to_string());

        let dag_proof = build_dag_proof_input(Some(&pipe), "test.transition-verb", &args)
            .await
            .expect("verb has transition_args and a matching DAG transition");

        let metadata = test_domain_metadata("test.transition-verb", vec!["\"testtable\""]);
        let entry_id = Uuid::new_v4();
        let write_set = build_write_set_input(Some(&metadata), "test.transition-verb", entry_id, vec![entity_id])
            .expect("verb has a non-empty write footprint");

        let entity_binding = Some(ob_poc_control_plane::entity_binding::EntityBindingInput {
            entities: Vec::new(),
        });
        let pack_resolution = Some(ob_poc_control_plane::pack_resolution::PackResolutionInput {
            candidate_pack_ids: vec!["fixture-pack".to_string()],
            semreg_allowed_set_available: true,
            constraint_denies_intent: false,
        });

        let envelope = SemOsContextEnvelope::test_with_verbs(&["test.transition-verb"]);
        let ctx = build_evaluation_context(
            &envelope,
            "test.transition-verb",
            Uuid::nil(),
            &test_actor(),
            entity_binding,
            pack_resolution,
            Some(dag_proof),
            Some(write_set),
            None,
            None,
            None,
            None,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::DagProof),
            Some(&ob_poc_control_plane::gate::GateResult::Success)
        );
        assert_eq!(
            report.get(ob_poc_control_plane::gate::GateId::WriteSet),
            Some(&ob_poc_control_plane::gate::GateResult::Success),
            "G7 must report a real, non-not_evaluated Success once its DagProof dependency succeeds"
        );
    }

    // ── T9.1/T9.5 closure sweep: all eight implemented gates, one dispatch ──

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn t9_1_closure_all_seven_gates_reach_a_real_outcome_on_one_dispatch() {
        // The actual amended-T9.1 exit criterion (ownership ledger,
        // "T9.1 is amended" entry): "all implemented gates non-
        // not_evaluated on every live verb family" — not seven pairwise
        // proofs, one combined dispatch. Every prior T9.1 sub-tranche
        // proved its own gate reachable given its declared dependency;
        // this is the first test that builds all seven inputs together
        // and checks the whole chain (G1 IntentAdmission through G7
        // WriteSet) in a single evaluate_shadow call, against a real cbu
        // row for G2's entity facts. T9.5 extended it in place (not a
        // new test) to also cover G8/StpClassifier, since G8 depends on
        // all seven of the others (GATE_DEPENDENCIES) — the fixture this
        // test already assembles is exactly G8's own precondition set.
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");
        let cbu_id: Uuid = sqlx::query_scalar(r#"SELECT cbu_id FROM "ob-poc".cbus LIMIT 1"#)
            .fetch_one(&pool)
            .await
            .expect("at least one cbu row exists in the dev database");

        let verb_fqn = "test.transition-verb";

        // G1: envelope admits the verb.
        let envelope = SemOsContextEnvelope::test_with_verbs(&[verb_fqn]);

        // G2: real per-entity facts for the live cbu row.
        let requests = vec![(cbu_id, "cbu".to_string())];
        let source = ob_poc_boundary::entity_facts::PgEntityFactsSource { pool: &pool };
        let facts = ob_poc_boundary::entity_facts::EntityFactsSource::entity_facts(&source, &requests)
            .await
            .expect("batched fetch succeeds");
        let entity_binding = Some(build_entity_binding_input(&requests, &facts));

        // G3: one active pack declaring the verb.
        let manifest = test_pack_manifest("fixture-pack", vec![verb_fqn]);
        let pack_resolution = Some(build_pack_resolution_input(
            Some(("fixture-pack", &manifest)),
            verb_fqn,
            true,
        ));

        // G4: fixture DAG/GateChecker, legal transition.
        let pipe = test_gate_pipeline();
        let mut args = HashMap::new();
        args.insert("entity-id".to_string(), cbu_id.to_string());
        let dag_proof = build_dag_proof_input(Some(&pipe), verb_fqn, &args)
            .await
            .expect("verb has transition_args and a matching DAG transition");

        // G7: fixture domain-metadata write footprint.
        let metadata = test_domain_metadata(verb_fqn, vec!["\"testtable\""]);
        let entry_id = Uuid::new_v4();
        let write_set = build_write_set_input(Some(&metadata), verb_fqn, entry_id, vec![cbu_id])
            .expect("verb has a non-empty write footprint");

        // G5 (Authority) and G6 (Evidence) need no separate build_* input
        // here — they're derived inside build_evaluation_context from the
        // same envelope/actor already assembled above (no AbacDenied
        // prune, no evidence_gaps -> Allow / Sufficient).
        //
        // G8 (STP classifier, T9.5/G6b): depends on all seven gates above
        // (GATE_DEPENDENCIES). This fixture's `RuntimeVerbRegistry` has no
        // entry for "test.transition-verb", so is_durable_verb is
        // honestly false. `has_unpinned_entities` is now the REAL G6b
        // populator fact, `has_unpinned_entities(&requests, Some(&facts))`
        // — not a hardcoded `false`. It resolves to `false` here because
        // `cbu_id` is a genuinely live row and `PgEntityFactsSource`
        // actually captured its `row_version` above (`facts` is non-empty
        // for it) — this is the "populator is real" proof, not a fixture
        // fabricated to make the gate pass.
        let stp_classifier = Some(build_stp_classifier_input(verb_fqn, has_unpinned_entities(&requests, Some(&facts))));

        // G13 (decision snapshot, T9.6): no declared dependency
        // (GATE_DEPENDENCIES), built from the exact same `facts` map G2
        // already fetched above — one live cbu row, one real row_version.
        let snapshot = build_decision_snapshot_input(Some(&facts));

        // G9 (runbook proof, T9.7): declares real predecessors (G1, G2,
        // G3, G4, G5, G6, G7, G13 — all already legal above), so this
        // reaches Success only because the whole chain does.
        let runbook_proof = Some(build_runbook_proof_input(Some(Uuid::new_v4())));

        // G12 (version pinning, T9.7): no declared dependency, real
        // compiler_version from this crate's own build.
        let version_pinning = Some(build_version_pinning_input());

        let ctx = build_evaluation_context(
            &envelope,
            verb_fqn,
            entry_id,
            &test_actor(),
            entity_binding,
            pack_resolution,
            Some(dag_proof),
            Some(write_set),
            stp_classifier,
            snapshot,
            runbook_proof,
            version_pinning,
        );
        let report = ob_poc_control_plane::evaluate_shadow(&ctx);

        use ob_poc_control_plane::gate::{GateId, GateResult};
        for gate_id in [
            GateId::IntentAdmission,
            GateId::EntityBinding,
            GateId::PackResolution,
            GateId::DagProof,
            GateId::Authority,
            GateId::Evidence,
            GateId::WriteSet,
            GateId::StpClassifier,
            GateId::DecisionSnapshot,
            GateId::RunbookProof,
            GateId::VersionPinning,
        ] {
            let result = report.get(gate_id);
            assert!(
                !matches!(result, Some(GateResult::NotEvaluated { .. }) | None),
                "T9.1 closure: {gate_id:?} must reach a real (non-not_evaluated) outcome, got {result:?}"
            );
            assert_eq!(
                result,
                Some(&GateResult::Success),
                "T9.1 closure: {gate_id:?} expected Success given every input was built to be legal, got {result:?}"
            );
        }
    }

    /// G6b (RR-5 row 2, E4 slug `toctou_entity_tables`) named human-gate
    /// test: an entity request for a UUID that genuinely does not exist in
    /// the dev database (real `PgEntityFactsSource` fetch, not a stub)
    /// produces `has_unpinned_entities == true` end to end, which caps G8
    /// at `HumanGated` — never `StpExecutable` — via the real
    /// `classify()` policy. This is the fail-closed twin of the closure
    /// test above: the populator being real does not mean every entity is
    /// now treated as pinned, only entities `PgEntityFactsSource`
    /// genuinely found and captured a `row_version` for.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn toctou_unpinned_entity_requires_human_gate() {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        let pool = sqlx::PgPool::connect(&url).await.expect("connect");

        // A UUID with no corresponding row in `cbus` — astronomically
        // unlikely to collide with a real fixture row.
        let missing_cbu_id = Uuid::new_v4();
        let requests = vec![(missing_cbu_id, "cbu".to_string())];
        let source = ob_poc_boundary::entity_facts::PgEntityFactsSource { pool: &pool };
        let facts = ob_poc_boundary::entity_facts::EntityFactsSource::entity_facts(&source, &requests)
            .await
            .expect("batched fetch succeeds (kind is supported; the row just isn't found)");

        // The real populator: absent from `facts` (nothing to read a
        // row_version from) is unpinned.
        assert!(
            has_unpinned_entities(&requests, Some(&facts)),
            "a not-found entity must be graded unpinned, not silently treated as pinned"
        );

        let input = build_stp_classifier_input("test.transition-verb", has_unpinned_entities(&requests, Some(&facts)));
        assert_eq!(
            ob_poc_control_plane::stp_classifier::classify(&input),
            ob_poc_control_plane::stp_classifier::StpEligibilityDecision::HumanGated,
            "an unpinned entity must cap STP eligibility at HumanGated (plan A5), never StpExecutable"
        );
    }
}
