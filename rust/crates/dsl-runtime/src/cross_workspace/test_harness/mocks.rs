//! In-memory mock implementations of the cross-workspace runtime traits.
//!
//! These mocks ignore the `&PgPool` parameter — the harness constructs a
//! lazy pool (`PgPool::connect_lazy`) that never actually opens a
//! connection because no SQL is ever executed.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use crate::cross_workspace::gate_checker::PredicateResolver;
use crate::cross_workspace::hierarchy_cascade::ChildEntityResolver;
use crate::cross_workspace::slot_state::SlotStateProvider;

// ---------------------------------------------------------------------------
// MockSlotStateProvider — in-memory `(workspace, slot, entity_id) → Option<state>`.
// ---------------------------------------------------------------------------

/// `(workspace, slot, entity_id)` → state. `None` means the row exists with
/// NULL state. Absence from the map means the entity is unknown
/// (`read_slot_state` returns `Ok(None)` per the trait contract for the
/// "stateless slot / unknown entity" case).
#[derive(Default)]
pub struct MockSlotStateProvider {
    states: Mutex<HashMap<(String, String, Uuid), Option<String>>>,
}

impl MockSlotStateProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set or replace the state for (workspace, slot, entity_id).
    pub fn set(&self, workspace: &str, slot: &str, entity_id: Uuid, state: Option<&str>) {
        self.states.lock().unwrap().insert(
            (workspace.to_string(), slot.to_string(), entity_id),
            state.map(String::from),
        );
    }
}

#[async_trait]
impl SlotStateProvider for MockSlotStateProvider {
    async fn read_slot_state(
        &self,
        workspace: &str,
        slot: &str,
        entity_id: Uuid,
        _pool: &PgPool,
    ) -> Result<Option<String>> {
        let map = self.states.lock().unwrap();
        Ok(map
            .get(&(workspace.to_string(), slot.to_string(), entity_id))
            .cloned()
            .unwrap_or(None))
    }
}

// ---------------------------------------------------------------------------
// MockPredicateResolver — exact-string predicate truth table.
// ---------------------------------------------------------------------------

/// Predicate string → `target_id → source_id`. The harness loads scenario
/// `predicates:` directly into this map; the GateChecker calls
/// `resolve_source_entity(predicate, target_id, ...)` and the mock looks
/// up `target_id` in the inner map for that predicate string.
///
/// If a predicate isn't in the table, returns `Ok(None)` — same as a
/// real predicate that didn't resolve to a row (which GateChecker treats
/// as a violation).
#[derive(Default)]
pub struct MockPredicateResolver {
    table: Mutex<HashMap<String, HashMap<Uuid, Uuid>>>,
}

impl MockPredicateResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a (target → source) mapping for the given predicate string.
    pub fn set(&self, predicate: &str, target: Uuid, source: Uuid) {
        self.table
            .lock()
            .unwrap()
            .entry(predicate.to_string())
            .or_default()
            .insert(target, source);
    }
}

#[async_trait]
impl PredicateResolver for MockPredicateResolver {
    async fn resolve_source_entity(
        &self,
        predicate: &str,
        target_entity_id: Uuid,
        _target_workspace: &str,
        _target_slot: &str,
        _pool: &PgPool,
    ) -> Result<Option<Uuid>> {
        let table = self.table.lock().unwrap();
        Ok(table
            .get(predicate)
            .and_then(|m| m.get(&target_entity_id).copied()))
    }
}

// ---------------------------------------------------------------------------
// MockChildEntityResolver — parent → list of (child_workspace, child_slot, child_id).
// ---------------------------------------------------------------------------

/// `(parent_ws, parent_slot, parent_id)` → list of children. The
/// `list_children` impl filters by the requested `(child_workspace,
/// child_slot)` pair so a parent can have children of multiple slot types.
#[derive(Default)]
pub struct MockChildEntityResolver {
    table: Mutex<HashMap<(String, String, Uuid), Vec<(String, String, Uuid)>>>,
}

impl MockChildEntityResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_child(
        &self,
        parent_workspace: &str,
        parent_slot: &str,
        parent_entity_id: Uuid,
        child_workspace: &str,
        child_slot: &str,
        child_entity_id: Uuid,
    ) {
        self.table
            .lock()
            .unwrap()
            .entry((
                parent_workspace.to_string(),
                parent_slot.to_string(),
                parent_entity_id,
            ))
            .or_default()
            .push((
                child_workspace.to_string(),
                child_slot.to_string(),
                child_entity_id,
            ));
    }
}

#[async_trait]
impl ChildEntityResolver for MockChildEntityResolver {
    async fn list_children(
        &self,
        parent_workspace: &str,
        parent_slot: &str,
        parent_entity_id: Uuid,
        child_workspace: &str,
        child_slot: &str,
        _pool: &PgPool,
    ) -> Result<Vec<Uuid>> {
        let table = self.table.lock().unwrap();
        Ok(table
            .get(&(
                parent_workspace.to_string(),
                parent_slot.to_string(),
                parent_entity_id,
            ))
            .map(|children| {
                children
                    .iter()
                    .filter(|(cw, cs, _)| cw == child_workspace && cs == child_slot)
                    .map(|(_, _, id)| *id)
                    .collect()
            })
            .unwrap_or_default())
    }
}
