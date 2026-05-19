//! ExecutionFrame — the live execution object (v0.5 §7.2).
//!
//! An `ExecutionFrame` is one live execution of one `ExecutablePlan`.
//! It is the unit of:
//! - Observability (each frame has its own structured trace)
//! - Failure isolation (frame failure does not affect other frames)
//! - Authority (frame carries its own context)
//! - Audit (frame accumulates audit records committed with the plan — T14)
//!
//! Phase 5: `ExecutionFrame` is introduced as a named type alongside the
//! existing `ExecutionContext`. The executor creates a frame at plan entry
//! and carries it through. Full frame-driven execution (replacing
//! `ExecutionContext` as the ambient state carrier) is Phase 6.
//!
//! ## Stub fields (Phase 6)
//!
//! - `cancellation_token`: not yet wired; Phase 6 cancellation scopes
//! - `deadline`: wired as `Instant::now() + plan_timeout` (E1 fix in T15)
//! - Full frame-based binding slot enforcement: Phase 6 (T10 produces the
//!   `BindingFrameSchema`; enforcement requires cooperative executor changes)

use std::time::Instant;
use uuid::Uuid;

// =============================================================================
// Identity types (v0.5 §10)
// =============================================================================

/// Identity of one submitted runtime execution (v0.5 §10.1).
///
/// Assigned at plan submission time. Immutable across all retry attempts.
/// Distinct from `PlanId` (the compiled plan's identity) and `AttemptId`
/// (the specific retry within this execution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionId(pub Uuid);

impl ExecutionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for ExecutionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "exec:{}", self.0)
    }
}

/// Identity of a specific retry attempt within one execution (v0.5 §10.1).
///
/// Starts at 1. Incremented on each retry. Two executions of the same plan
/// with different `AttemptId` values represent retries; only one attempt
/// actually commits (the one that returns `Committed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttemptId(pub u32);

impl AttemptId {
    pub fn first() -> Self {
        Self(1)
    }

    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

impl Default for AttemptId {
    fn default() -> Self {
        Self::first()
    }
}

impl std::fmt::Display for AttemptId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "attempt:{}", self.0)
    }
}

// =============================================================================
// BindingFrame — typed slot map (v0.5 §7.2)
// =============================================================================

/// Live binding slot map for one execution frame.
///
/// Maps binding slot names (`@name`) to their concrete UUID values, populated
/// as steps execute and produce their declared outputs.
///
/// Phase 5: populated alongside `ExecutionContext.symbols` (which continues
/// to be the executor's primary symbol table). Phase 6: replaces `symbols`
/// as the authoritative binding source.
#[derive(Debug, Clone, Default)]
pub struct BindingFrame {
    slots: std::collections::HashMap<String, uuid::Uuid>,
}

impl BindingFrame {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a produced UUID into a named slot.
    pub fn put(&mut self, name: impl Into<String>, uuid: uuid::Uuid) {
        self.slots.insert(name.into(), uuid);
    }

    /// Look up a slot by name. Returns `None` if the slot is not yet populated.
    pub fn get(&self, name: &str) -> Option<uuid::Uuid> {
        self.slots.get(name).copied()
    }

    /// Validate that all declared slots in a schema are populated.
    ///
    /// Returns the names of any unpopulated slots. Empty vec = all slots ready.
    pub fn missing_slots<'a>(&self, declared: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        declared
            .into_iter()
            .filter(|name| !self.slots.contains_key(*name))
            .map(|s| s.to_string())
            .collect()
    }
}

// =============================================================================
// AuditBuffer — per-frame audit accumulator (v0.5 §7.2, §13.5)
// =============================================================================

/// A structured audit record for one node execution in the frame.
///
/// Phase 5 stub: collected per frame, written to the `dsl_execution_audit`
/// table in T14 (audit-as-commit-boundary for DurableStep paths).
#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub execution_id: ExecutionId,
    pub attempt_id: AttemptId,
    /// The step index within the plan (maps to `node_id` in the typed DAG).
    pub node_id: usize,
    pub verb_fqn: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Outcome string — "committed", "rolled_back", "failed", etc.
    pub outcome: String,
}

/// Per-frame audit record accumulator.
///
/// In Phase 5, records are accumulated in memory but not yet written to the
/// DB inside the transaction boundary (that wiring is T14). The buffer is
/// available for inspection and out-of-band async emission.
#[derive(Debug, Default)]
pub struct AuditBuffer {
    pub records: Vec<AuditRecord>,
}

impl AuditBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, record: AuditRecord) {
        self.records.push(record);
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

// =============================================================================
// ExecutionFrame (v0.5 §7.2)
// =============================================================================

/// The live execution object for one `ExecutablePlan` execution.
///
/// Carries the identity, binding state, audit accumulator, and authority
/// context for one execution attempt. Created at plan submission; dropped
/// at outcome (committed, failed, or timed out).
///
/// Phase 5: created alongside the existing `ExecutionContext`; the executor
/// reads from both in parallel. Phase 6: replaces `ExecutionContext` as the
/// primary execution state carrier.
#[derive(Debug)]
pub struct ExecutionFrame {
    /// Identity of this execution (v0.5 §10.1).
    pub execution_id: ExecutionId,
    /// Identity of this specific attempt (v0.5 §10.1).
    pub attempt_id: AttemptId,
    /// Typed binding slot map — populated as steps produce bindings.
    pub binding_slots: BindingFrame,
    /// Audit record accumulator — written to DB inside tx boundary in T14.
    pub audit_buffer: AuditBuffer,
    /// Hard execution deadline — guards against indefinite hangs (E1 fix).
    /// Set to `Instant::now() + configured_plan_timeout` at frame creation.
    pub deadline: Instant,
    // cancellation_token: Phase 6 (sub-DAG cancellation scopes)
}

impl ExecutionFrame {
    /// Create a new frame for a first-attempt execution.
    ///
    /// `plan_timeout_secs`: the per-plan deadline in seconds from now.
    /// Defaults to 30s if 0 is passed (prevents infinite hang).
    pub fn new(plan_timeout_secs: u64) -> Self {
        let timeout = if plan_timeout_secs == 0 {
            30
        } else {
            plan_timeout_secs
        };
        Self {
            execution_id: ExecutionId::new(),
            attempt_id: AttemptId::first(),
            binding_slots: BindingFrame::new(),
            audit_buffer: AuditBuffer::new(),
            deadline: Instant::now() + std::time::Duration::from_secs(timeout),
        }
    }

    /// Create a retry attempt frame, reusing the execution_id but
    /// incrementing the attempt_id.
    pub fn retry(
        execution_id: ExecutionId,
        prior_attempt: AttemptId,
        plan_timeout_secs: u64,
    ) -> Self {
        let timeout = if plan_timeout_secs == 0 {
            30
        } else {
            plan_timeout_secs
        };
        Self {
            execution_id,
            attempt_id: prior_attempt.next(),
            binding_slots: BindingFrame::new(),
            audit_buffer: AuditBuffer::new(),
            deadline: Instant::now() + std::time::Duration::from_secs(timeout),
        }
    }

    /// True if the deadline has already passed.
    pub fn is_expired(&self) -> bool {
        Instant::now() > self.deadline
    }

    /// Record a step outcome into the audit buffer.
    pub fn record_outcome(
        &mut self,
        node_id: usize,
        verb_fqn: impl Into<String>,
        started_at: chrono::DateTime<chrono::Utc>,
        outcome: impl Into<String>,
    ) {
        self.audit_buffer.push(AuditRecord {
            execution_id: self.execution_id,
            attempt_id: self.attempt_id,
            node_id,
            verb_fqn: verb_fqn.into(),
            started_at,
            completed_at: Some(chrono::Utc::now()),
            outcome: outcome.into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_id_is_unique() {
        let a = ExecutionId::new();
        let b = ExecutionId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn attempt_id_increments() {
        let first = AttemptId::first();
        let second = first.next();
        assert_eq!(first.0 + 1, second.0);
    }

    #[test]
    fn binding_frame_put_and_get() {
        let mut frame = BindingFrame::new();
        let uuid = uuid::Uuid::new_v4();
        frame.put("cbu", uuid);
        assert_eq!(frame.get("cbu"), Some(uuid));
        assert_eq!(frame.get("missing"), None);
    }

    #[test]
    fn binding_frame_missing_slots() {
        let mut frame = BindingFrame::new();
        let uuid = uuid::Uuid::new_v4();
        frame.put("cbu", uuid);
        let missing = frame.missing_slots(["cbu", "deal", "case"]);
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&"deal".to_string()));
        assert!(missing.contains(&"case".to_string()));
    }

    #[test]
    fn execution_frame_records_outcome() {
        let mut frame = ExecutionFrame::new(30);
        assert!(!frame.is_expired());
        frame.record_outcome(0, "cbu.ensure", chrono::Utc::now(), "committed");
        assert_eq!(frame.audit_buffer.records.len(), 1);
        assert_eq!(frame.audit_buffer.records[0].outcome, "committed");
    }

    #[test]
    fn frame_retry_increments_attempt() {
        let frame = ExecutionFrame::new(30);
        let eid = frame.execution_id;
        let aid = frame.attempt_id;
        let retry = ExecutionFrame::retry(eid, aid, 30);
        assert_eq!(retry.execution_id, eid);
        assert_eq!(retry.attempt_id.0, aid.0 + 1);
    }
}
