//! Persistence abstraction for the bpmn-lite journey runtime.
//!
//! [`JourneyStore`] is the single trait that all runtime components use for
//! state access. Two implementations ship here:
//!
//! - [`InMemoryJourneyStore`] — in-process, no DB required. Used for all unit
//!   and integration tests in `bpmn-test-harness`.
//! - `PostgresJourneyStore` is deferred to Tranche 7+ (needs `sqlx`).
//!
//! # Design note
//! Every method is async so that the Postgres implementation can slot in
//! without changing call sites.

use crate::retention::RetentionPolicy;
use crate::types::*;
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// A row to append to the journey audit log.
#[derive(Debug, Clone)]
pub struct JourneyLogEntry {
    pub instance_id: InstanceId,
    pub token_id: Option<TokenId>,
    pub event_kind: String,
    pub from_node: Option<String>,
    pub to_node: Option<String>,
    pub data_delta: Option<serde_json::Value>,
}

/// Minimal view of a pending-wait row returned by correlation lookup.
#[derive(Debug, Clone)]
pub struct PendingWaitInfo {
    pub id: Uuid,
    pub instance_id: InstanceId,
    pub token_id: TokenId,
    pub node_name: String,
}

// ---------------------------------------------------------------------------
// JourneyStore trait
// ---------------------------------------------------------------------------

/// All persistence operations consumed by the journey runtime.
#[async_trait::async_trait]
pub trait JourneyStore: Send + Sync {
    // --- Instance operations ---

    async fn create_instance(
        &self,
        journey_name: &str,
        initial_data: serde_json::Value,
    ) -> Result<WorkflowInstance>;

    async fn get_instance(&self, id: InstanceId) -> Result<Option<WorkflowInstance>>;

    async fn update_instance_status(
        &self,
        id: InstanceId,
        status: InstanceStatus,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<()>;

    // --- Token operations ---

    async fn create_token(
        &self,
        instance_id: InstanceId,
        node: &str,
        fork_ref: Option<Uuid>,
        lineage: Vec<String>,
    ) -> Result<ActiveToken>;

    async fn get_tokens_for_instance(&self, instance_id: InstanceId) -> Result<Vec<ActiveToken>>;

    async fn advance_token(&self, token_id: TokenId, new_node: &str) -> Result<()>;

    async fn delete_token(&self, token_id: TokenId) -> Result<()>;

    async fn append_to_write_log(&self, token_id: TokenId, entry: WriteLogEntry) -> Result<()>;

    // --- Instance data ---

    async fn write_instance_data(
        &self,
        instance_id: InstanceId,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()>;

    async fn read_instance_data(
        &self,
        instance_id: InstanceId,
        key: &str,
    ) -> Result<Option<serde_json::Value>>;

    // --- Event queue ---

    async fn enqueue_event(
        &self,
        instance_id: InstanceId,
        kind: EventKind,
        payload: serde_json::Value,
    ) -> Result<EventId>;

    /// Dequeue up to `max` unclaimed events (oldest first).
    async fn dequeue_events(&self, max: usize) -> Result<Vec<EventEnvelope>>;

    /// Acknowledge that an event has been processed and can be discarded.
    async fn ack_event(&self, event_id: EventId) -> Result<()>;

    // --- Journey log ---

    async fn append_journey_log(&self, entry: JourneyLogEntry) -> Result<()>;

    // --- Pending waits ---

    async fn create_pending_wait(
        &self,
        instance_id: InstanceId,
        token_id: TokenId,
        wait_kind: &str,
        node_name: &str,
        correlation_key: Option<String>,
        timeout_at: Option<DateTime<Utc>>,
    ) -> Result<Uuid>;

    async fn find_pending_wait_by_correlation(
        &self,
        wait_kind: &str,
        correlation_key: &str,
    ) -> Result<Option<PendingWaitInfo>>;

    // --- Switch decisions ---

    async fn create_switch_request(
        &self,
        instance_id: InstanceId,
        token_id: TokenId,
        gateway_name: &str,
        gateway_kind: &str,
        context: serde_json::Value,
    ) -> Result<Uuid>;

    // --- Join arrivals ---

    /// Record that `token_id` has arrived at `join_name` for `instance_id`.
    /// Returns the total number of distinct tokens that have arrived so far.
    async fn record_join_arrival(
        &self,
        join_name: &str,
        instance_id: InstanceId,
        token_id: TokenId,
    ) -> Result<usize>;

    /// Return all live tokens for `instance_id` whose `current_node == join_name`.
    /// Used by the merge protocol to collect branch write-logs before firing.
    async fn get_tokens_at_join(
        &self,
        join_name: &str,
        instance_id: InstanceId,
    ) -> Result<Vec<ActiveToken>>;

    /// Store a dynamic expected-arrival count for an inclusive-gateway join.
    async fn set_expected_join_count(
        &self,
        join_name: &str,
        instance_id: InstanceId,
        count: usize,
    ) -> Result<()>;

    /// Retrieve a previously stored dynamic expected-arrival count.
    async fn get_expected_join_count(
        &self,
        join_name: &str,
        instance_id: InstanceId,
    ) -> Result<Option<usize>>;

    /// Reduce the expected-arrival count by one (token-death short-circuit).
    /// Returns the new expected count.
    async fn reduce_expected_join_count(
        &self,
        join_name: &str,
        instance_id: InstanceId,
    ) -> Result<usize>;

    // --- Retention ---

    /// Return instance IDs eligible for archival under the given policy.
    ///
    /// Default implementation is a no-op (nothing to archive).
    async fn find_archivable_instances(
        &self,
        _policy: &RetentionPolicy,
    ) -> Result<Vec<InstanceId>> {
        Ok(vec![])
    }

    /// Archive the journey log entries for a completed instance.
    ///
    /// Returns the number of log rows affected. Default implementation is a
    /// no-op (returns 0). `PostgresJourneyStore` implements this for real.
    async fn archive_instance_log(&self, _instance_id: InstanceId) -> Result<usize> {
        Ok(0)
    }
}

// ---------------------------------------------------------------------------
// InMemoryJourneyStore
// ---------------------------------------------------------------------------

#[derive(Default)]
struct InMemoryState {
    instances: HashMap<InstanceId, WorkflowInstance>,
    tokens: HashMap<TokenId, ActiveToken>,
    /// Events not yet ACKed, in insertion order.
    events: Vec<EventEnvelope>,
    instance_data: HashMap<(InstanceId, String), serde_json::Value>,
    journey_log: Vec<JourneyLogEntry>,
    /// (id, instance_id, token_id, wait_kind, node_name, correlation_key)
    pending_waits: Vec<(Uuid, InstanceId, TokenId, String, String, Option<String>)>,
    /// (join_name, instance_id) → set of arrived token IDs
    join_arrivals: HashMap<(String, InstanceId), Vec<TokenId>>,
    /// (join_name, instance_id) → dynamic expected count (inclusive gateway case)
    join_expected_counts: HashMap<(String, InstanceId), usize>,
    /// (id, instance_id, token_id, gateway_name)
    switch_requests: Vec<(Uuid, InstanceId, TokenId, String)>,
}

/// Thread-safe in-memory implementation of [`JourneyStore`].
pub struct InMemoryJourneyStore {
    state: Arc<Mutex<InMemoryState>>,
}

impl InMemoryJourneyStore {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(InMemoryState::default())),
        }
    }
}

impl Default for InMemoryJourneyStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl JourneyStore for InMemoryJourneyStore {
    async fn create_instance(
        &self,
        journey_name: &str,
        initial_data: serde_json::Value,
    ) -> Result<WorkflowInstance> {
        let inst = WorkflowInstance {
            id: Uuid::new_v4(),
            journey_name: journey_name.to_string(),
            version: 1,
            status: InstanceStatus::Active,
            started_at: Utc::now(),
            completed_at: None,
            data: initial_data,
        };
        self.state
            .lock()
            .unwrap()
            .instances
            .insert(inst.id, inst.clone());
        Ok(inst)
    }

    async fn get_instance(&self, id: InstanceId) -> Result<Option<WorkflowInstance>> {
        Ok(self.state.lock().unwrap().instances.get(&id).cloned())
    }

    async fn update_instance_status(
        &self,
        id: InstanceId,
        status: InstanceStatus,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        if let Some(inst) = s.instances.get_mut(&id) {
            inst.status = status;
            inst.completed_at = completed_at;
        }
        Ok(())
    }

    async fn create_token(
        &self,
        instance_id: InstanceId,
        node: &str,
        fork_ref: Option<Uuid>,
        lineage: Vec<String>,
    ) -> Result<ActiveToken> {
        let token = ActiveToken {
            id: Uuid::new_v4(),
            instance_id,
            current_node: node.to_string(),
            fork_ref,
            branch_lineage: lineage,
            write_log: Vec::new(),
        };
        self.state
            .lock()
            .unwrap()
            .tokens
            .insert(token.id, token.clone());
        Ok(token)
    }

    async fn get_tokens_for_instance(&self, instance_id: InstanceId) -> Result<Vec<ActiveToken>> {
        let s = self.state.lock().unwrap();
        Ok(s.tokens
            .values()
            .filter(|t| t.instance_id == instance_id)
            .cloned()
            .collect())
    }

    async fn advance_token(&self, token_id: TokenId, new_node: &str) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        if let Some(t) = s.tokens.get_mut(&token_id) {
            t.current_node = new_node.to_string();
        }
        Ok(())
    }

    async fn delete_token(&self, token_id: TokenId) -> Result<()> {
        self.state.lock().unwrap().tokens.remove(&token_id);
        Ok(())
    }

    async fn append_to_write_log(&self, token_id: TokenId, entry: WriteLogEntry) -> Result<()> {
        let mut s = self.state.lock().unwrap();
        if let Some(t) = s.tokens.get_mut(&token_id) {
            t.write_log.push(entry);
        }
        Ok(())
    }

    async fn write_instance_data(
        &self,
        instance_id: InstanceId,
        key: &str,
        value: serde_json::Value,
    ) -> Result<()> {
        self.state
            .lock()
            .unwrap()
            .instance_data
            .insert((instance_id, key.to_string()), value);
        Ok(())
    }

    async fn read_instance_data(
        &self,
        instance_id: InstanceId,
        key: &str,
    ) -> Result<Option<serde_json::Value>> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .instance_data
            .get(&(instance_id, key.to_string()))
            .cloned())
    }

    async fn enqueue_event(
        &self,
        instance_id: InstanceId,
        kind: EventKind,
        payload: serde_json::Value,
    ) -> Result<EventId> {
        let id = Uuid::new_v4();
        self.state.lock().unwrap().events.push(EventEnvelope {
            id,
            instance_id,
            event_kind: kind,
            payload,
        });
        Ok(id)
    }

    async fn dequeue_events(&self, max: usize) -> Result<Vec<EventEnvelope>> {
        let mut s = self.state.lock().unwrap();
        let n = max.min(s.events.len());
        let batch: Vec<EventEnvelope> = s.events.drain(0..n).collect();
        Ok(batch)
    }

    async fn ack_event(&self, _event_id: EventId) -> Result<()> {
        // In-memory: events are drained on dequeue, nothing to ack.
        Ok(())
    }

    async fn append_journey_log(&self, entry: JourneyLogEntry) -> Result<()> {
        self.state.lock().unwrap().journey_log.push(entry);
        Ok(())
    }

    async fn create_pending_wait(
        &self,
        instance_id: InstanceId,
        token_id: TokenId,
        wait_kind: &str,
        node_name: &str,
        correlation_key: Option<String>,
        _timeout_at: Option<DateTime<Utc>>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        self.state.lock().unwrap().pending_waits.push((
            id,
            instance_id,
            token_id,
            wait_kind.to_string(),
            node_name.to_string(),
            correlation_key,
        ));
        Ok(id)
    }

    async fn find_pending_wait_by_correlation(
        &self,
        wait_kind: &str,
        correlation_key: &str,
    ) -> Result<Option<PendingWaitInfo>> {
        let s = self.state.lock().unwrap();
        for (id, inst_id, tok_id, kind, node, corr) in &s.pending_waits {
            if kind == wait_kind && corr.as_deref() == Some(correlation_key) {
                return Ok(Some(PendingWaitInfo {
                    id: *id,
                    instance_id: *inst_id,
                    token_id: *tok_id,
                    node_name: node.clone(),
                }));
            }
        }
        Ok(None)
    }

    async fn create_switch_request(
        &self,
        instance_id: InstanceId,
        token_id: TokenId,
        gateway_name: &str,
        _gateway_kind: &str,
        _context: serde_json::Value,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        self.state.lock().unwrap().switch_requests.push((
            id,
            instance_id,
            token_id,
            gateway_name.to_string(),
        ));
        Ok(id)
    }

    async fn record_join_arrival(
        &self,
        join_name: &str,
        instance_id: InstanceId,
        token_id: TokenId,
    ) -> Result<usize> {
        let mut s = self.state.lock().unwrap();
        let arrivals = s
            .join_arrivals
            .entry((join_name.to_string(), instance_id))
            .or_default();
        if !arrivals.contains(&token_id) {
            arrivals.push(token_id);
        }
        Ok(arrivals.len())
    }

    async fn get_tokens_at_join(
        &self,
        join_name: &str,
        instance_id: InstanceId,
    ) -> Result<Vec<ActiveToken>> {
        let s = self.state.lock().unwrap();
        let tokens = s
            .tokens
            .values()
            .filter(|t| t.instance_id == instance_id && t.current_node == join_name)
            .cloned()
            .collect();
        Ok(tokens)
    }

    async fn set_expected_join_count(
        &self,
        join_name: &str,
        instance_id: InstanceId,
        count: usize,
    ) -> Result<()> {
        self.state
            .lock()
            .unwrap()
            .join_expected_counts
            .insert((join_name.to_string(), instance_id), count);
        Ok(())
    }

    async fn get_expected_join_count(
        &self,
        join_name: &str,
        instance_id: InstanceId,
    ) -> Result<Option<usize>> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .join_expected_counts
            .get(&(join_name.to_string(), instance_id))
            .copied())
    }

    async fn reduce_expected_join_count(
        &self,
        join_name: &str,
        instance_id: InstanceId,
    ) -> Result<usize> {
        let mut s = self.state.lock().unwrap();
        let entry = s
            .join_expected_counts
            .entry((join_name.to_string(), instance_id))
            .or_insert(0);
        if *entry > 0 {
            *entry -= 1;
        }
        Ok(*entry)
    }
}
