//! Synthetic [`PendingStateAdvance`] producer for §9a invariant tests.

use ob_poc_types::{DagNodeId, PendingStateAdvance, StateTransition};
use uuid::{uuid, Uuid};

/// Stable entity id used by the fixture so harness assertions can
/// pin against a known UUID.
pub const FIXTURE_ENTITY_ID: Uuid = uuid!("11111111-2222-3333-4444-555555555555");

/// Stable DAG node id the synthetic state transition advances to.
pub const FIXTURE_DAG_NODE_ID: Uuid = uuid!("66666666-7777-8888-9999-aaaaaaaaaaaa");

/// Build a non-empty [`PendingStateAdvance`] for fixture use.
///
/// Carries one [`StateTransition`] (no `from_node`, advancing the
/// fixture entity to [`FIXTURE_DAG_NODE_ID`]) plus a non-zero
/// `writes_since_push_delta`. The shape is the minimum the §9a
/// rollback gate needs to assert against: at least one transition
/// to apply, plus a counter delta the Sequencer can compare.
///
/// # Determinism
///
/// Pure function — same inputs (the constants above) → same output.
/// Determinism harness fixtures pin against the JSON serialisation.
pub fn fixture_pending_state_advance() -> PendingStateAdvance {
    PendingStateAdvance {
        state_transitions: vec![StateTransition {
            entity_id: FIXTURE_ENTITY_ID,
            from_node: None,
            to_node: DagNodeId(FIXTURE_DAG_NODE_ID),
            reason: Some("phase-5c-migrate fixture transition".to_string()),
        }],
        constellation_marks: Vec::new(),
        writes_since_push_delta: 1,
        catalogue_effects: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_is_non_empty() {
        let p = fixture_pending_state_advance();
        assert_eq!(p.state_transitions.len(), 1);
        assert_eq!(p.state_transitions[0].entity_id, FIXTURE_ENTITY_ID);
        assert_eq!(p.state_transitions[0].to_node.0, FIXTURE_DAG_NODE_ID);
        assert_eq!(p.writes_since_push_delta, 1);
    }

    #[test]
    fn fixture_is_pure_and_byte_stable() {
        let a = fixture_pending_state_advance();
        let b = fixture_pending_state_advance();
        assert_eq!(a, b, "fixture must be byte-identical across calls");

        let json = serde_json::to_string(&a).unwrap();
        // Pin the JSON shape so a future drift in PendingStateAdvance's
        // serde representation breaks this test before it breaks the
        // §9a invariant harness.
        assert!(json.contains("\"entity_id\":\"11111111-2222-3333-4444-555555555555\""));
        assert!(json.contains("\"writes_since_push_delta\":1"));
    }
}
