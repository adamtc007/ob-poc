//! Navigation replay for deterministic state reconstruction.
//!
//! NavigationLog records all verbs with timestamps, enabling:
//! - Deterministic replay to reconstruct state
//! - Audit trail of user navigation
//! - Debugging and testing

use crate::state::DroneState;
use crate::verb::Verb;
use crate::Fault;
use esper_snapshot::WorldSnapshot;
use serde::{Deserialize, Serialize};

/// A verb with timestamp for replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedVerb {
    /// Frame tick when verb was executed.
    pub tick: u64,

    /// The verb that was executed.
    pub verb: Verb,
}

impl TimestampedVerb {
    /// Create a new timestamped verb.
    pub fn new(tick: u64, verb: Verb) -> Self {
        Self { tick, verb }
    }
}

/// Log of navigation commands for replay.
///
/// # Determinism
///
/// Given the same WorldSnapshot and NavigationLog, replay will always
/// produce the same final DroneState. This is useful for:
/// - Reproducing user sessions for debugging
/// - Testing navigation flows
/// - Undo/redo implementation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NavigationLog {
    /// Session identifier.
    pub session_id: u64,

    /// Cache key for the world snapshot used.
    pub snapshot_key: String,

    /// Recorded events.
    pub events: Vec<TimestampedVerb>,
}

impl NavigationLog {
    /// Create a new empty log.
    pub fn new(session_id: u64, snapshot_key: String) -> Self {
        Self {
            session_id,
            snapshot_key,
            events: Vec::new(),
        }
    }

    /// Record a verb execution.
    pub fn record(&mut self, tick: u64, verb: Verb) {
        // Skip noops
        if matches!(verb, Verb::Noop) {
            return;
        }
        self.events.push(TimestampedVerb::new(tick, verb));
    }

    /// Replay all events to reconstruct state.
    ///
    /// # Arguments
    ///
    /// * `world` - The world snapshot (must match snapshot_key)
    ///
    /// # Returns
    ///
    /// The final DroneState after replaying all events.
    pub fn replay(&self, world: &WorldSnapshot) -> Result<DroneState, Fault> {
        let mut state = DroneState::new();

        for event in &self.events {
            state.tick = event.tick;
            // Ignore recoverable faults during replay
            let _ = state.execute(event.verb, world);
        }

        Ok(state)
    }

    /// Replay up to a specific tick.
    pub fn replay_to(&self, world: &WorldSnapshot, target_tick: u64) -> Result<DroneState, Fault> {
        let mut state = DroneState::new();

        for event in &self.events {
            if event.tick > target_tick {
                break;
            }
            state.tick = event.tick;
            let _ = state.execute(event.verb, world);
        }

        Ok(state)
    }

    /// Get the number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Get events as DSL strings.
    pub fn to_dsl(&self) -> Vec<String> {
        self.events.iter().map(|e| e.verb.to_dsl()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esper_snapshot::{CameraPreset, ChamberKind, ChamberSnapshot, Rect, NONE_IDX};

    fn make_test_world() -> WorldSnapshot {
        let chamber = ChamberSnapshot {
            id: 0,
            kind: ChamberKind::Grid,
            bounds: Rect::new(0.0, 0.0, 100.0, 100.0),
            default_camera: CameraPreset::default(),
            entity_ids: vec![1, 2, 3],
            kind_ids: vec![1, 1, 1],
            x: vec![10.0, 50.0, 90.0],
            y: vec![10.0, 50.0, 90.0],
            label_ids: vec![0, 1, 2],
            detail_refs: vec![1, 2, 3],
            first_child: vec![NONE_IDX, NONE_IDX, NONE_IDX],
            next_sibling: vec![1, 2, NONE_IDX],
            prev_sibling: vec![NONE_IDX, 0, 1],
            doors: vec![],
            grid: esper_snapshot::GridSnapshot::default(),
        };

        WorldSnapshot {
            envelope: esper_snapshot::SnapshotEnvelope {
                schema_version: 1,
                source_hash: 0,
                policy_hash: 0,
                created_at: 0,
                cbu_id: 1,
            },
            string_table: vec!["A".to_string(), "B".to_string(), "C".to_string()],
            chambers: vec![chamber],
        }
    }

    #[test]
    fn log_record_and_replay() {
        let world = make_test_world();
        let mut log = NavigationLog::new(1, "test".to_string());

        // Record some navigation
        log.record(1, Verb::ZoomTo(2.0));
        log.record(2, Verb::PanTo { x: 50.0, y: 50.0 });
        log.record(3, Verb::Select(1));

        assert_eq!(log.len(), 3);

        // Replay
        let state = log.replay(&world).unwrap();

        assert_eq!(state.camera.target_zoom, 2.0);
        assert_eq!(state.camera.target.x, 50.0);
        assert_eq!(state.taxonomy.selection, Some(1));
    }

    #[test]
    fn log_replay_to_tick() {
        let world = make_test_world();
        let mut log = NavigationLog::new(1, "test".to_string());

        log.record(1, Verb::ZoomTo(2.0));
        log.record(5, Verb::ZoomTo(3.0));
        log.record(10, Verb::ZoomTo(4.0));

        // Replay to tick 5
        let state = log.replay_to(&world, 5).unwrap();
        assert_eq!(state.camera.target_zoom, 3.0);

        // Replay to tick 2
        let state = log.replay_to(&world, 2).unwrap();
        assert_eq!(state.camera.target_zoom, 2.0);
    }

    #[test]
    fn log_skip_noop() {
        let mut log = NavigationLog::new(1, "test".to_string());

        log.record(1, Verb::Noop);
        log.record(2, Verb::ZoomTo(2.0));
        log.record(3, Verb::Noop);

        assert_eq!(log.len(), 1); // Only the zoom
    }

    #[test]
    fn log_to_dsl() {
        let mut log = NavigationLog::new(1, "test".to_string());

        log.record(1, Verb::ZoomTo(2.0));
        log.record(2, Verb::Next);

        let dsl = log.to_dsl();
        assert_eq!(dsl.len(), 2);
        assert!(dsl[0].contains("zoom-to"));
        assert!(dsl[1].contains("next"));
    }

    #[test]
    fn log_determinism() {
        let world = make_test_world();
        let mut log = NavigationLog::new(1, "test".to_string());

        // Record a sequence
        log.record(1, Verb::Select(0));
        log.record(2, Verb::Next);
        log.record(3, Verb::ZoomTo(1.5));
        log.record(4, Verb::PanTo { x: 25.0, y: 25.0 });

        // Replay twice - should get identical state
        let state1 = log.replay(&world).unwrap();
        let state2 = log.replay(&world).unwrap();

        assert_eq!(state1.taxonomy.selection, state2.taxonomy.selection);
        assert_eq!(state1.camera.target, state2.camera.target);
        assert_eq!(state1.camera.target_zoom, state2.camera.target_zoom);
    }
}
