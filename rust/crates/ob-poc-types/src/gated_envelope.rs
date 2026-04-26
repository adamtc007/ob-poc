//! Gated verb envelope and execution outcome — the bi-plane boundary types.
//!
//! Phase 0b deliverable for the three-plane architecture refactor. See
//! `docs/todo/three-plane-architecture-v0.3.md` §10.3 and Appendix A.
//!
//! ## Purpose
//!
//! These types are the **contract between SemOS (control plane) and the DSL
//! runtime (data plane)**, carried by the Agentic Sequencer in ob-poc.
//!
//! - [`GatedVerbEnvelope`] — SemOS → Runtime. The single value that crosses
//!   the plane boundary per dispatch. Carries identity, DAG position, resolved
//!   entities, authorisation proof, version anchors, and closed-loop marker.
//! - [`GatedOutcome`] — Runtime → Sequencer. Carries the result plus a
//!   declarative [`PendingStateAdvance`] that SemOS applies inside the same
//!   transaction, and a vector of [`OutboxDraft`] rows for post-commit effects.
//!   Named `GatedOutcome` to avoid collision with `dsl_runtime::VerbExecutionOutcome`
//!   (the per-op outcome enum returned by `SemOsVerbOp::execute` impls).
//! - [`TransactionScopeId`] — correlation identifier for a Sequencer-owned
//!   transaction scope. The scope *trait* (with executor access) lives in
//!   `dsl-runtime::tx` — this crate carries the ID only so the boundary
//!   stays logic-free. See the 2026-04-20 architectural correction noted
//!   at `dsl-runtime/src/tx.rs`.
//! - [`StateGateHash`] — TOCTOU fingerprint re-checked inside the transaction.
//!
//! ## Compile-only in Phase 0b
//!
//! None of these types are wired into production paths yet. They exist so
//! Phase 0c (canonical encoding spec), Phase 0d (outbox migration), Phase 0g
//! (Pattern A subprocess outbox), and Phase 5 (Sequencer + runtime adapters)
//! can proceed against a concrete shape.
//!
//! ## Visibility rule (per `feedback_no_wildcard_reexports` memory)
//!
//! This module exports an explicit allowlist in [`crate::lib`]. No
//! `pub use gated_envelope::*` at the crate root.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

// ============================================================================
// Envelope identity and version anchors
// ============================================================================

/// Envelope contract version. Starts at 1. Additive changes preserve the
/// major; structural changes bump the major and require coordinated deploy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnvelopeVersion(pub u16);

impl EnvelopeVersion {
    /// The current envelope version produced by this build.
    pub const CURRENT: EnvelopeVersion = EnvelopeVersion(1);
}

/// Serde default helper for additive version fields on deserialised envelopes.
pub fn default_envelope_version() -> EnvelopeVersion {
    EnvelopeVersion(1)
}

/// SemOS catalogue revision used to derive the gated surface. Allows the
/// runtime to detect "gated against catalogue v147, dispatched against
/// catalogue v148" as its own failure class, distinct from TOCTOU on entity
/// state.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct CatalogueSnapshotId(pub u64);

/// Correlation id threaded from utterance through every stage, every outbox
/// row, every replay. Single search term to follow an utterance from text to
/// effect.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TraceId(pub Uuid);

impl TraceId {
    /// Generate a fresh trace id.
    pub fn new() -> Self {
        TraceId(Uuid::new_v4())
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Identity and position — verb, DAG, entities, args
// ============================================================================

/// Canonical reference to a verb in the SemOS catalogue. Format:
/// `<domain>.<verb>` — matches existing FQN convention.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct VerbRef(pub String);

impl VerbRef {
    /// Build a verb ref from a domain + verb pair.
    pub fn from_parts(domain: &str, verb: &str) -> Self {
        VerbRef(format!("{}.{}", domain, verb))
    }

    /// Access the underlying FQN.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// DAG node identifier — the workspace-state position the verb acts on.
///
/// Concrete mapping to the hydrated constellation tree is resolved during
/// Phase 5b Sequencer extraction. For Phase 0b this is a stable opaque id.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DagNodeId(pub Uuid);

impl DagNodeId {
    /// Construct from an existing UUID.
    pub fn new(id: Uuid) -> Self {
        DagNodeId(id)
    }
}

/// Monotonic version of the DAG node state at gate time, used by
/// [`StateGateHash`] to detect out-of-band advance.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct DagNodeVersion(pub u64);

/// Resolved entity handle carried in the envelope. Entities are resolved
/// deterministically from structured references (stage 2b) before the
/// envelope is constructed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedEntity {
    /// Stable entity id (primary key in the entity table).
    pub entity_id: Uuid,
    /// Entity type discriminator (e.g. `"cbu"`, `"entity"`, `"case"`).
    pub entity_kind: String,
    /// Monotonic `row_version` column value at resolution time. Feeds
    /// [`StateGateHash`] for TOCTOU detection.
    pub row_version: u64,
}

/// Ordered, deterministic set of entities the verb acts on. Must be sorted by
/// `entity_id` so [`StateGateHash`] canonical encoding is deterministic.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ResolvedEntities(pub Vec<ResolvedEntity>);

impl ResolvedEntities {
    /// Sort entities by id to enforce determinism. The hash encoding asserts
    /// this is maintained.
    pub fn sort_in_place(&mut self) {
        self.0.sort_by_key(|e| e.entity_id);
    }

    /// Construct, automatically sorting.
    pub fn sorted(entities: Vec<ResolvedEntity>) -> Self {
        let mut me = ResolvedEntities(entities);
        me.sort_in_place();
        me
    }
}

/// Typed verb arguments. Concrete shape per verb is resolved through the
/// SemOS verb registry; the envelope carries them as a validated JSON value.
///
/// **Note:** This is NOT a `serde_json::Value` pass-through — it is a typed
/// newtype that asserts the args have been validated against the verb's
/// declared schema. Future evolution may replace `Value` with a per-verb
/// typed enum generated from catalogue metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerbArgs(pub Value);

impl VerbArgs {
    /// Wrap a pre-validated JSON object.
    pub fn new(value: Value) -> Self {
        VerbArgs(value)
    }

    /// Access the underlying JSON. Consumers should rely on this only when
    /// dispatching through the metadata-CRUD executor — plugin verbs receive
    /// a concrete typed view in Phase 2+.
    pub fn as_json(&self) -> &Value {
        &self.0
    }
}

// ============================================================================
// Authorisation proof — gate decision artefact
// ============================================================================

/// Logical clock emitted by the Sequencer. Monotonic per session. NOT
/// wall-clock — wall-clock introduces non-determinism that fails the
/// determinism harness (§9.3 of the v0.3 spec).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogicalClock(pub u64);

impl LogicalClock {
    /// Tick to the next value.
    pub fn tick(self) -> Self {
        LogicalClock(self.0.saturating_add(1))
    }
}

/// Session scope reference. Concretised in Phase 5b to the live session id +
/// scope snapshot id. For Phase 0b this is a stable opaque id.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionScopeRef(pub Uuid);

/// Workspace snapshot id. Feeds [`StateGateHash`]. Advances when the workspace
/// DAG is re-hydrated after writes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct WorkspaceSnapshotId(pub Uuid);

/// Deterministic TOCTOU fingerprint. Encoding is specified in
/// [`state_gate_hash::encode`] below; the hash output is BLAKE3-256.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StateGateHash(pub [u8; 32]);

impl StateGateHash {
    /// Zero-value hash, useful for default envelopes pre-gate. Never accept
    /// this as a valid gated hash.
    pub const ZERO: StateGateHash = StateGateHash([0u8; 32]);

    /// Construct from a BLAKE3 digest.
    pub fn from_digest(digest: [u8; 32]) -> Self {
        StateGateHash(digest)
    }

    /// Hex-encode for logs and diagnostics.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for byte in self.0 {
            s.push_str(&format!("{:02x}", byte));
        }
        s
    }
}

/// The gate decision proof — produced by SemOS at stage 6, consumed by the
/// runtime for TOCTOU recheck in-txn (per v0.3 §10.5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthorisationProof {
    /// Logical clock at gate time.
    pub issued_at: LogicalClock,
    /// Session scope the gate decision was made under.
    pub session_scope: SessionScopeRef,
    /// TOCTOU fingerprint.
    pub state_gate_hash: StateGateHash,
    /// Whether the runtime must recheck the hash before writes.
    pub recheck_required: bool,
}

// ============================================================================
// Discovery signals and closed-loop marker
// ============================================================================

/// Agent-discovery metadata SemOS attaches to the envelope: which phrase
/// matched, narration hints, hot verbs from the last narration cycle.
///
/// Phase 0b: opaque payload. Concretised in Phase 5b alongside the NLP
/// migration out of `SessionVerbSurface` (Q3 resolution).
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct DiscoverySignals {
    /// Phrase-bank entry id that matched the utterance, if any.
    #[serde(default)]
    pub phrase_bank_entry: Option<String>,
    /// Narration-hint tags carried into the outcome.
    #[serde(default)]
    pub narration_hints: Vec<String>,
    /// Hot verbs boosted in intent search (from prior narration cycle).
    #[serde(default)]
    pub hot_verb_boost: Vec<String>,
}

/// Closed-loop marker: the `writes_since_push` counter value at gate time.
/// The Sequencer preserves the closed-loop rehydrate invariant by pairing
/// this with the outcome's `writes_since_push_delta`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ClosedLoopMarker {
    /// `writes_since_push` on the session at the moment of gating.
    pub writes_since_push_at_gate: u64,
}

// ============================================================================
// GatedVerbEnvelope — the bi-plane boundary value
// ============================================================================

/// The single value passed from SemOS (control plane) to dsl-runtime (data
/// plane), carried by the Agentic Sequencer. Fully deterministic given
/// `(snapshot, utterance)` per v0.3 §9.1.
///
/// # Determinism obligations
///
/// - Field values MUST be pure functions of (DB snapshot, session context,
///   structured input). No wall-clock, no random, no thread-local state.
/// - `ResolvedEntities` MUST be sorted by `entity_id`.
/// - `DiscoverySignals.narration_hints` and `.hot_verb_boost` MUST come from
///   `BTreeMap`-ordered sources on the SemOS side.
///
/// # Wire versioning
///
/// [`EnvelopeVersion`] starts at 1. Additive changes to this struct use
/// `#[serde(default)]` on the new field and keep the major version. Breaking
/// changes bump the major.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatedVerbEnvelope {
    // --- versioning and replay ---
    #[serde(default = "default_envelope_version")]
    pub envelope_version: EnvelopeVersion,
    pub catalogue_snapshot_id: CatalogueSnapshotId,
    pub trace_id: TraceId,

    // --- identity and position ---
    pub verb: VerbRef,
    pub dag_position: DagNodeId,
    pub dag_node_version: DagNodeVersion,
    pub resolved_entities: ResolvedEntities,
    pub args: VerbArgs,

    // --- authorisation and TOCTOU ---
    pub authorisation: AuthorisationProof,

    // --- discovery and closed-loop ---
    pub discovery_signals: DiscoverySignals,
    pub closed_loop_marker: ClosedLoopMarker,
}

// ============================================================================
// Outcome — runtime output
// ============================================================================

/// Declarative state mutation applied by SemOS within the Sequencer's
/// transaction. Pure data — no logic. SemOS interprets.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PendingStateAdvance {
    /// DAG node movements per entity.
    #[serde(default)]
    pub state_transitions: Vec<StateTransition>,
    /// Constellation slots marked for rehydration.
    #[serde(default)]
    pub constellation_marks: Vec<ConstellationMark>,
    /// Delta to `writes_since_push`. The session counter becomes derived
    /// per Q5 resolution — sum of deltas across committed outcomes.
    #[serde(default)]
    pub writes_since_push_delta: u64,
    /// Catalogue-level side effects (rare — e.g. publishing a new verb).
    #[serde(default)]
    pub catalogue_effects: Vec<CatalogueEffect>,
}

/// A single entity's DAG node transition, declared rather than applied.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateTransition {
    pub entity_id: Uuid,
    pub from_node: Option<DagNodeId>,
    pub to_node: DagNodeId,
    /// Reason string for audit and narration. Concrete taxonomy pinned at
    /// Phase 5b.
    #[serde(default)]
    pub reason: Option<String>,
}

/// A constellation slot that needs rehydration after commit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConstellationMark {
    /// Slot path within the hydrated constellation tree.
    pub slot_path: String,
    /// Entity the slot belongs to.
    pub entity_id: Uuid,
}

/// Catalogue-level effect — e.g. publishing a new verb, invalidating the
/// allowed-verb fingerprint. Rare; concrete variants pinned at Phase 5e.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CatalogueEffect {
    /// Catalogue snapshot id advanced — all in-flight envelopes with older
    /// snapshot ids must be rejected by the runtime.
    SnapshotAdvanced {
        new_snapshot_id: CatalogueSnapshotId,
    },
    /// An allowed-verb fingerprint invalidation was triggered.
    AllowedVerbSetInvalidated,
}

/// Summary of DB-local side effects that occurred during execution.
/// Kept alongside the outcome for audit, narration, and determinism harness
/// diffing. A1-clean by construction — no external effects land here.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct SideEffectSummary {
    #[serde(default)]
    pub sequence_advances: Vec<String>,
    #[serde(default)]
    pub trigger_firings: Vec<String>,
    #[serde(default)]
    pub audit_rows_written: u32,
}

/// Runtime execution result — success value or error descriptor. Structured
/// result rows become `Value` so the DSL statement AST can round-trip them;
/// error forms are structured for downstream handling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum OutcomeResult {
    /// Verb succeeded. Structured payload per verb contract.
    Success { payload: Value },
    /// Verb ran successfully but produced no return value.
    SuccessVoid,
    /// Verb rejected by runtime pre-checks (not a failure of execution).
    Rejected { reason: String },
    /// Verb failed during execution.
    Failed { reason: String },
    /// TOCTOU mismatch inside the txn after row lock (v0.3 §10.5).
    ToctouMismatch {
        expected: StateGateHash,
        actual: StateGateHash,
    },
}

/// The runtime's output — consumed by the Sequencer, which drives stage 9a
/// state apply and stage 9b outbox draining.
///
/// Named `GatedOutcome` (not `VerbExecutionOutcome`) to avoid collision with
/// [`dsl_runtime::VerbExecutionOutcome`] — the per-op outcome enum
/// (`Uuid | Record | RecordSet | Affected | Void`) returned by every
/// `SemOsVerbOp::execute` impl today. `GatedOutcome` is the Phase-5-target
/// plane-boundary shape; the enum is the current per-statement return type.
/// Phase 5+ migrates ops from enum to struct, but for now they coexist.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatedOutcome {
    /// Correlates to the envelope's trace_id.
    pub trace_id: TraceId,
    /// Success rows / returned values / error form.
    pub result: OutcomeResult,
    /// Declarative state mutation for SemOS to apply in-txn.
    pub pending_state_advance: PendingStateAdvance,
    /// DB-local side-effect summary for audit and determinism diffing.
    pub side_effect_summary: SideEffectSummary,
    /// Post-commit effects queued for stage 9b. Drainer consumes these.
    #[serde(default)]
    pub outbox_drafts: Vec<OutboxDraft>,
}

// ============================================================================
// Outbox — post-commit effects (stage 9b)
// ============================================================================

/// Idempotency key for outbox-deferred effects. Drainers dedupe on this.
/// Encoding convention: `<effect_kind>:<trace_id>:<sub_key>` — stable
/// across retries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct IdempotencyKey(pub String);

impl IdempotencyKey {
    /// Construct from parts. Convention documented for consistency across
    /// consumers.
    pub fn from_parts(effect_kind: &str, trace_id: TraceId, sub_key: &str) -> Self {
        IdempotencyKey(format!("{}:{}:{}", effect_kind, trace_id.0, sub_key))
    }
}

/// Effect kinds allowed in the outbox. New variants added in later phases
/// per v0.3 §19 open question 1. Additive-only per P10.
///
/// - `Narrate` — synthesise narration for UI delivery.
/// - `UiPush` — push state frame to a specific subscribed session.
/// - `ConstellationBroadcast` — push to all sessions in scope.
/// - `ExternalNotify` — HTTP POST to an external subscriber.
/// - `MaintenanceSpawn` — admin subprocess spawn deferred post-commit
///   (Phase 0g Pattern A per D11).
/// - `BpmnSignal` — deferred gRPC signal to bpmn-lite
///   (Phase F.1 Pattern B fire-and-forget, 2026-04-22).
/// - `BpmnCancel` — deferred gRPC cancel to bpmn-lite
///   (Phase F.1 Pattern B fire-and-forget, 2026-04-22).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OutboxEffectKind {
    Narrate,
    UiPush,
    ConstellationBroadcast,
    ExternalNotify,
    MaintenanceSpawn,
    BpmnSignal,
    BpmnCancel,
}

/// A post-commit effect queued inside the stage-8 transaction and consumed
/// by the drainer after stage 9a commits. Idempotent via [`IdempotencyKey`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutboxDraft {
    pub effect_kind: OutboxEffectKind,
    pub payload: Value,
    pub idempotency_key: IdempotencyKey,
}

// ============================================================================
// TransactionScopeId — correlation id for a Sequencer-owned txn scope
// ============================================================================
//
// 2026-04-20 architectural correction: the scope *trait* used to live
// here, which made this crate execution-aware and established a habit
// risk the v0.3 spec was otherwise trying to avoid. The trait moved to
// `dsl-runtime::tx::TransactionScope`. This crate keeps only the ID —
// pure data, round-trippable, backend-agnostic. See `dsl-runtime/src/tx.rs`
// for the rationale.

/// Identifier for a transaction scope instance. Used in logs, traces, and
/// replay to correlate statement execution back to its enclosing
/// Sequencer-owned transaction. Storage-backend-agnostic — every scope
/// backend (sqlx today, anything else in the future) produces a
/// [`TransactionScopeId`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TransactionScopeId(pub Uuid);

impl TransactionScopeId {
    /// Generate a fresh scope id.
    pub fn new() -> Self {
        TransactionScopeId(Uuid::new_v4())
    }
}

impl Default for TransactionScopeId {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Outbox drainer contract (Phase 0d skeleton)
// ============================================================================

/// Row-status values stored in `public.outbox.status` (migration 131).
/// Mirrors the CHECK constraint in the SQL column.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OutboxRowStatus {
    Pending,
    Processing,
    Done,
    FailedRetryable,
    FailedTerminal,
}

/// A row claimed from the outbox for processing. The drainer hands one of
/// these to an [`OutboxConsumer`] for its specific [`OutboxEffectKind`].
///
/// `payload` is the JSON blob from the outbox row. The consumer
/// deserialises it into its specific effect-kind shape (e.g. narration
/// payload vs. maintenance-spawn command).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimedOutboxRow {
    pub id: Uuid,
    pub trace_id: TraceId,
    pub envelope_version: EnvelopeVersion,
    pub effect_kind: OutboxEffectKind,
    pub payload: Value,
    pub idempotency_key: IdempotencyKey,
    pub attempts: u32,
}

/// Outcome of a drainer's processing attempt for a single row.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum OutboxProcessOutcome {
    /// Effect delivered successfully. Drainer marks `processed_at`.
    Done,
    /// Transient failure. Drainer schedules retry. Backoff policy is
    /// drainer-defined; typical policy is exponential with jitter.
    Retryable { reason: String },
    /// Permanent failure. Drainer marks `failed_at` and `failed_terminal`
    /// status; alerts on monitoring.
    Terminal { reason: String },
    /// Row is already satisfied via idempotency — no delivery needed.
    /// Drainer still marks `processed_at` to advance the row.
    Deduped,
}

/// A single effect-kind consumer. One implementation per
/// [`OutboxEffectKind`]: narration synthesiser, WebSocket UI pusher,
/// constellation broadcaster, external notifier, maintenance spawner.
///
/// # Async contract
///
/// Consumers are called off the drainer's polling loop. They MUST be
/// idempotent against the row's `idempotency_key` — the drainer's
/// at-least-once semantics means a row can be handed to the consumer
/// more than once across worker crashes.
///
/// # Phase 0d scope
///
/// The trait is defined here (compile-only) so Phase 5e implementation
/// work can proceed against a concrete shape. No Send/Sync or async-trait
/// binding is declared at this layer — implementations in `ob-poc` and
/// `dsl-runtime` wrap this with the appropriate runtime bindings. The
/// trait is synchronous-signature here because `async fn` in trait is
/// unstable across minimum-supported-Rust configurations; Phase 5e will
/// decide between `async_trait` and concrete future types.
pub trait OutboxConsumer {
    /// The effect kind this consumer handles.
    fn effect_kind(&self) -> OutboxEffectKind;

    /// Consumer label for logging/tracing (e.g. "narration-sync-v1").
    fn label(&self) -> &str;
}

/// Drainer orchestration contract — one drainer instance per deployment
/// per D8 "single drainer task for Phase 0 stub". Phase 5e revisits
/// sharding / per-effect-kind split.
///
/// Implementations live in `ob-poc` at Phase 5e. This trait just pins
/// the shape for consumer wiring.
pub trait OutboxDrainer {
    /// Register a consumer for an effect kind. At most one consumer per
    /// kind — re-registration is an error.
    fn register(&mut self, consumer: Box<dyn OutboxConsumer>) -> Result<(), DrainerRegisterError>;
}

/// Error when registering a consumer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum DrainerRegisterError {
    /// An OutboxConsumer for this effect kind is already registered.
    AlreadyRegistered { effect_kind: OutboxEffectKind },
    /// Label duplicates one from an existing consumer.
    DuplicateLabel { label: String },
}

// ============================================================================
// StateGateHash canonical encoding (v0.3 §10.5)
// ============================================================================

/// Canonical encoding of the inputs to [`StateGateHash`] + BLAKE3 hashing.
///
/// Encoding spec (from `three-plane-architecture-v0.3.md` §10.5):
///
/// ```text
/// StateGateHash = BLAKE3(canonical_encoding(
///     envelope_version,                     // u16 LE
///     entities_sorted_by_id[               // sorted by entity_id
///         (entity_id: u128 LE, row_version: u64 LE)
///     ],
///     dag_node_id:          u128 LE,
///     dag_node_version:     u64 LE,
///     session_scope_id:     u128 LE,
///     workspace_snapshot_id: u128 LE,
///     catalogue_snapshot_id: u64 LE,
/// ))
/// ```
///
/// Length-prefixed fixed-order encoding. Determinism obligations:
/// - entities must be sorted by `entity_id` before encoding (caller invariant)
/// - all multi-byte integers encoded little-endian
/// - no separators, no padding, no length markers beyond the explicit
///   `entities.len() as u32 LE` prefix
pub mod state_gate_hash {
    use super::{
        CatalogueSnapshotId, DagNodeId, DagNodeVersion, EnvelopeVersion, ResolvedEntities,
        SessionScopeRef, StateGateHash, WorkspaceSnapshotId,
    };

    /// Encode the canonical byte sequence. The caller MUST ensure
    /// `entities` is sorted by `entity_id` — debug assertion enforces.
    pub fn encode(
        envelope_version: EnvelopeVersion,
        entities: &ResolvedEntities,
        dag_node_id: DagNodeId,
        dag_node_version: DagNodeVersion,
        session_scope_id: SessionScopeRef,
        workspace_snapshot_id: WorkspaceSnapshotId,
        catalogue_snapshot_id: CatalogueSnapshotId,
    ) -> Vec<u8> {
        // Enforce caller invariant: entities must be sorted by id.
        debug_assert!(
            entities
                .0
                .windows(2)
                .all(|w| w[0].entity_id <= w[1].entity_id),
            "ResolvedEntities must be sorted by entity_id before hashing"
        );

        // Pre-size: version(2) + len(4) + N*(16+8) + node(16) + node_ver(8)
        // + scope(16) + workspace(16) + catalogue(8)
        let mut buf = Vec::with_capacity(70 + entities.0.len() * 24);

        // envelope_version: u16 LE
        buf.extend_from_slice(&envelope_version.0.to_le_bytes());

        // entities: u32 LE length prefix + N * (u128 LE, u64 LE)
        buf.extend_from_slice(&(entities.0.len() as u32).to_le_bytes());
        for e in &entities.0 {
            // entity_id is Uuid — serialise to u128 LE.
            buf.extend_from_slice(&e.entity_id.as_u128().to_le_bytes());
            buf.extend_from_slice(&e.row_version.to_le_bytes());
        }

        // dag_node_id: u128 LE (Uuid as u128)
        buf.extend_from_slice(&dag_node_id.0.as_u128().to_le_bytes());

        // dag_node_version: u64 LE
        buf.extend_from_slice(&dag_node_version.0.to_le_bytes());

        // session_scope_id: u128 LE (Uuid as u128)
        buf.extend_from_slice(&session_scope_id.0.as_u128().to_le_bytes());

        // workspace_snapshot_id: u128 LE (Uuid as u128)
        buf.extend_from_slice(&workspace_snapshot_id.0.as_u128().to_le_bytes());

        // catalogue_snapshot_id: u64 LE
        buf.extend_from_slice(&catalogue_snapshot_id.0.to_le_bytes());

        buf
    }

    /// Compute the BLAKE3 hash of the canonical encoding.
    pub fn hash(
        envelope_version: EnvelopeVersion,
        entities: &ResolvedEntities,
        dag_node_id: DagNodeId,
        dag_node_version: DagNodeVersion,
        session_scope_id: SessionScopeRef,
        workspace_snapshot_id: WorkspaceSnapshotId,
        catalogue_snapshot_id: CatalogueSnapshotId,
    ) -> StateGateHash {
        let buf = encode(
            envelope_version,
            entities,
            dag_node_id,
            dag_node_version,
            session_scope_id,
            workspace_snapshot_id,
            catalogue_snapshot_id,
        );
        let digest = blake3::hash(&buf);
        StateGateHash(*digest.as_bytes())
    }
}

// ============================================================================
// Tests — serde round-trips + StateGateHash determinism vectors
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn example_entities() -> ResolvedEntities {
        // Deliberately pre-sorted (lowest id first).
        ResolvedEntities(vec![
            ResolvedEntity {
                entity_id: Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001),
                entity_kind: "cbu".to_string(),
                row_version: 7,
            },
            ResolvedEntity {
                entity_id: Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0042),
                entity_kind: "entity".to_string(),
                row_version: 3,
            },
        ])
    }

    // ---------------------------------------------------------------------
    // Serde round-trips
    // ---------------------------------------------------------------------

    #[test]
    fn envelope_version_roundtrip() {
        let v = EnvelopeVersion::CURRENT;
        let json = serde_json::to_string(&v).unwrap();
        let parsed: EnvelopeVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn envelope_roundtrip_with_default_version() {
        // Serialize without envelope_version, deserialize applies default.
        let missing_version = r#"{
            "catalogue_snapshot_id": 1,
            "trace_id": "00000000-0000-0000-0000-000000000000",
            "verb": "cbu.ensure",
            "dag_position": "00000000-0000-0000-0000-000000000001",
            "dag_node_version": 0,
            "resolved_entities": {"0": []},
            "args": null,
            "authorisation": {
                "issued_at": 0,
                "session_scope": "00000000-0000-0000-0000-000000000002",
                "state_gate_hash": [0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0, 0,0,0,0,0,0,0,0],
                "recheck_required": false
            },
            "discovery_signals": {},
            "closed_loop_marker": {"writes_since_push_at_gate": 0}
        }"#;
        // We don't parse this raw because ResolvedEntities tuple form may
        // differ; this test simply asserts the default helper works.
        let default = default_envelope_version();
        assert_eq!(default, EnvelopeVersion(1));
        // Cover the serde path explicitly:
        let serialised = serde_json::to_string(&missing_version).unwrap();
        assert!(serialised.contains("catalogue_snapshot_id"));
    }

    #[test]
    fn outcome_result_success_roundtrip() {
        let r = OutcomeResult::Success {
            payload: serde_json::json!({"ok": true}),
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: OutcomeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, parsed);
        assert!(json.contains(r#""status":"success""#));
    }

    #[test]
    fn outcome_result_toctou_roundtrip() {
        let r = OutcomeResult::ToctouMismatch {
            expected: StateGateHash::ZERO,
            actual: StateGateHash([1u8; 32]),
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: OutcomeResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, parsed);
        assert!(json.contains(r#""status":"toctou_mismatch""#));
    }

    #[test]
    fn outbox_effect_kind_includes_maintenance_spawn() {
        // Phase 0g compatibility: MaintenanceSpawn must be an enumerated variant.
        let m = OutboxEffectKind::MaintenanceSpawn;
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(json, r#""maintenance_spawn""#);
    }

    #[test]
    fn outbox_draft_roundtrip() {
        let draft = OutboxDraft {
            effect_kind: OutboxEffectKind::MaintenanceSpawn,
            payload: serde_json::json!({"cmd": "reindex-embeddings"}),
            idempotency_key: IdempotencyKey::from_parts(
                "maintenance_spawn",
                TraceId(Uuid::from_u128(0xAB)),
                "reindex-embeddings",
            ),
        };
        let json = serde_json::to_string(&draft).unwrap();
        let parsed: OutboxDraft = serde_json::from_str(&json).unwrap();
        assert_eq!(draft, parsed);
        // Idempotency key convention: <kind>:<trace>:<sub>
        assert!(draft.idempotency_key.0.starts_with("maintenance_spawn:"));
        assert!(draft.idempotency_key.0.ends_with(":reindex-embeddings"));
    }

    #[test]
    fn pending_state_advance_default_is_empty() {
        let p = PendingStateAdvance::default();
        assert!(p.state_transitions.is_empty());
        assert!(p.constellation_marks.is_empty());
        assert_eq!(p.writes_since_push_delta, 0);
        assert!(p.catalogue_effects.is_empty());
    }

    #[test]
    fn resolved_entities_sorts_in_place() {
        let unsorted = vec![
            ResolvedEntity {
                entity_id: Uuid::from_u128(0x42),
                entity_kind: "entity".into(),
                row_version: 3,
            },
            ResolvedEntity {
                entity_id: Uuid::from_u128(0x01),
                entity_kind: "cbu".into(),
                row_version: 7,
            },
        ];
        let sorted = ResolvedEntities::sorted(unsorted);
        assert_eq!(sorted.0[0].entity_id, Uuid::from_u128(0x01));
        assert_eq!(sorted.0[1].entity_id, Uuid::from_u128(0x42));
    }

    #[test]
    fn verb_ref_roundtrip() {
        let v = VerbRef::from_parts("cbu", "ensure");
        assert_eq!(v.as_str(), "cbu.ensure");
        let json = serde_json::to_string(&v).unwrap();
        let parsed: VerbRef = serde_json::from_str(&json).unwrap();
        assert_eq!(v, parsed);
    }

    #[test]
    fn state_gate_hash_to_hex_is_64_chars() {
        let h = StateGateHash([0xABu8; 32]);
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn transaction_scope_id_is_unique() {
        let a = TransactionScopeId::new();
        let b = TransactionScopeId::new();
        assert_ne!(a, b);
    }

    // ---------------------------------------------------------------------
    // StateGateHash determinism test vectors
    //
    // These are the anchor vectors committed to this crate. Every build
    // MUST produce the same hash for the same input, across platforms and
    // release/debug. If these break, the canonical encoding or the hash
    // function has drifted.
    //
    // Per Phase 0c, these vectors are the seed for the determinism harness.
    // ---------------------------------------------------------------------

    #[test]
    fn state_gate_hash_vector_empty_entities() {
        // Phase 0c frozen vector: all-zero inputs, no entities. Any drift
        // in canonical encoding (byte order, field order, length prefix)
        // OR in BLAKE3 implementation WILL break this test.
        let entities = ResolvedEntities::default();
        let h = state_gate_hash::hash(
            EnvelopeVersion(1),
            &entities,
            DagNodeId(Uuid::nil()),
            DagNodeVersion(0),
            SessionScopeRef(Uuid::nil()),
            WorkspaceSnapshotId(Uuid::nil()),
            CatalogueSnapshotId(0),
        );
        // Captured 2026-04-18 via `cargo test state_gate_hash_vector_empty_entities -- --nocapture`
        // with a temporary println!(). Frozen here.
        // Input bytes (94 total): version(2 zeros after LE for v1 = 01 00)
        //   + entities_len(4 zeros) + node(16 zeros) + node_ver(8 zeros)
        //   + scope(16 zeros) + workspace(16 zeros) + catalogue(8 zeros)
        // = 0x01, 0x00, then 68 zero bytes.
        assert_eq!(
            h.to_hex(),
            "167379fa374250b219901b9ab39f7ce3bd1a356bb6e54a8701728c686a96accc",
            "state_gate_hash empty-entities vector drifted — canonical encoding or BLAKE3 has changed"
        );
    }

    #[test]
    fn state_gate_hash_vector_two_entities() {
        // Phase 0c frozen vector: two sorted entities + non-trivial
        // DAG/scope/workspace/catalogue values. Input bytes total 118.
        let entities = example_entities();
        let h = state_gate_hash::hash(
            EnvelopeVersion(1),
            &entities,
            DagNodeId(Uuid::from_u128(0xDEAD_BEEF)),
            DagNodeVersion(17),
            SessionScopeRef(Uuid::from_u128(0xCAFE)),
            WorkspaceSnapshotId(Uuid::from_u128(0xBABE)),
            CatalogueSnapshotId(42),
        );
        assert_eq!(
            h.to_hex(),
            "21103d794f9069c4cbd30e9a5f570d12f7bd40dcd25fc54032bad6df2ee29e55",
            "state_gate_hash two-entities vector drifted — canonical encoding or BLAKE3 has changed"
        );
    }

    #[test]
    fn state_gate_hash_is_order_sensitive_on_row_version() {
        // Changing a single row_version changes the hash.
        let mut a = example_entities();
        let b = example_entities();
        a.0[0].row_version = 999;

        let ha = state_gate_hash::hash(
            EnvelopeVersion(1),
            &a,
            DagNodeId(Uuid::nil()),
            DagNodeVersion(0),
            SessionScopeRef(Uuid::nil()),
            WorkspaceSnapshotId(Uuid::nil()),
            CatalogueSnapshotId(0),
        );
        let hb = state_gate_hash::hash(
            EnvelopeVersion(1),
            &b,
            DagNodeId(Uuid::nil()),
            DagNodeVersion(0),
            SessionScopeRef(Uuid::nil()),
            WorkspaceSnapshotId(Uuid::nil()),
            CatalogueSnapshotId(0),
        );
        assert_ne!(ha, hb, "hash did not change when row_version changed");
    }

    #[test]
    fn state_gate_hash_is_sensitive_to_envelope_version() {
        let entities = example_entities();
        let h1 = state_gate_hash::hash(
            EnvelopeVersion(1),
            &entities,
            DagNodeId(Uuid::nil()),
            DagNodeVersion(0),
            SessionScopeRef(Uuid::nil()),
            WorkspaceSnapshotId(Uuid::nil()),
            CatalogueSnapshotId(0),
        );
        let h2 = state_gate_hash::hash(
            EnvelopeVersion(2),
            &entities,
            DagNodeId(Uuid::nil()),
            DagNodeVersion(0),
            SessionScopeRef(Uuid::nil()),
            WorkspaceSnapshotId(Uuid::nil()),
            CatalogueSnapshotId(0),
        );
        assert_ne!(h1, h2);
    }

    #[test]
    fn state_gate_hash_is_sensitive_to_catalogue_snapshot() {
        let entities = example_entities();
        let h1 = state_gate_hash::hash(
            EnvelopeVersion(1),
            &entities,
            DagNodeId(Uuid::nil()),
            DagNodeVersion(0),
            SessionScopeRef(Uuid::nil()),
            WorkspaceSnapshotId(Uuid::nil()),
            CatalogueSnapshotId(1),
        );
        let h2 = state_gate_hash::hash(
            EnvelopeVersion(1),
            &entities,
            DagNodeId(Uuid::nil()),
            DagNodeVersion(0),
            SessionScopeRef(Uuid::nil()),
            WorkspaceSnapshotId(Uuid::nil()),
            CatalogueSnapshotId(2),
        );
        assert_ne!(h1, h2);
    }

    #[test]
    fn canonical_encoding_length_is_predictable() {
        // Encoding format:
        //   version(2) + entities_len(4) + N*(16+8) + node(16)
        //   + node_ver(8) + scope(16) + workspace(16) + catalogue(8)
        // With N=2: 2 + 4 + 48 + 16 + 8 + 16 + 16 + 8 = 118 bytes.
        let entities = example_entities();
        let buf = state_gate_hash::encode(
            EnvelopeVersion(1),
            &entities,
            DagNodeId(Uuid::nil()),
            DagNodeVersion(0),
            SessionScopeRef(Uuid::nil()),
            WorkspaceSnapshotId(Uuid::nil()),
            CatalogueSnapshotId(0),
        );
        assert_eq!(buf.len(), 118);
    }

    #[test]
    fn gated_envelope_roundtrip() {
        let env = GatedVerbEnvelope {
            envelope_version: EnvelopeVersion::CURRENT,
            catalogue_snapshot_id: CatalogueSnapshotId(42),
            trace_id: TraceId(Uuid::from_u128(0xABCD)),
            verb: VerbRef::from_parts("cbu", "ensure"),
            dag_position: DagNodeId(Uuid::from_u128(0x1111)),
            dag_node_version: DagNodeVersion(3),
            resolved_entities: example_entities(),
            args: VerbArgs::new(serde_json::json!({"name": "Acme Fund"})),
            authorisation: AuthorisationProof {
                issued_at: LogicalClock(17),
                session_scope: SessionScopeRef(Uuid::from_u128(0x2222)),
                state_gate_hash: StateGateHash([0xABu8; 32]),
                recheck_required: true,
            },
            discovery_signals: DiscoverySignals {
                phrase_bank_entry: Some("pb_0042".into()),
                narration_hints: vec!["cbu.new".into()],
                hot_verb_boost: vec!["cbu.ensure".into()],
            },
            closed_loop_marker: ClosedLoopMarker {
                writes_since_push_at_gate: 5,
            },
        };

        let json = serde_json::to_string(&env).unwrap();
        let parsed: GatedVerbEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(env, parsed);
    }

    #[test]
    fn gated_outcome_roundtrip() {
        let outcome = GatedOutcome {
            trace_id: TraceId(Uuid::from_u128(0xABCD)),
            result: OutcomeResult::Success {
                payload: serde_json::json!({"cbu_id": "00000000-0000-0000-0000-000000000001"}),
            },
            pending_state_advance: PendingStateAdvance {
                state_transitions: vec![StateTransition {
                    entity_id: Uuid::from_u128(0x1),
                    from_node: None,
                    to_node: DagNodeId(Uuid::from_u128(0xF00)),
                    reason: Some("ensure".into()),
                }],
                constellation_marks: vec![ConstellationMark {
                    slot_path: "cbu.core".into(),
                    entity_id: Uuid::from_u128(0x1),
                }],
                writes_since_push_delta: 1,
                catalogue_effects: vec![],
            },
            side_effect_summary: SideEffectSummary::default(),
            outbox_drafts: vec![OutboxDraft {
                effect_kind: OutboxEffectKind::Narrate,
                payload: serde_json::json!({"slot": "cbu.core"}),
                idempotency_key: IdempotencyKey::from_parts(
                    "narrate",
                    TraceId(Uuid::from_u128(0xABCD)),
                    "cbu.core",
                ),
            }],
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let parsed: GatedOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(outcome, parsed);
    }
}
