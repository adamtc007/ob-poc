use crate::events::RuntimeEvent;
use crate::types::*;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::BTreeMap;
use uuid::Uuid;

/// Persistence trait for all BPMN-Lite state.
///
/// 28 async methods organized by concern. The VM and engine operate exclusively
/// through this trait, enabling pluggable backends (MemoryStore for POC,
/// Postgres for production).
#[async_trait]
pub trait ProcessStore: Send + Sync {
    // ── Instance ──

    async fn save_instance(&self, instance: &ProcessInstance) -> Result<()>;
    async fn load_instance(&self, id: Uuid) -> Result<Option<ProcessInstance>>;
    async fn update_instance_state(&self, id: Uuid, state: ProcessState) -> Result<()>;
    async fn update_instance_flags(&self, id: Uuid, flags: &BTreeMap<FlagKey, Value>)
        -> Result<()>;
    async fn update_instance_payload(&self, id: Uuid, payload: &str, hash: &[u8; 32])
        -> Result<()>;

    // ── Fibers ──

    async fn save_fiber(&self, instance_id: Uuid, fiber: &Fiber) -> Result<()>;
    async fn load_fiber(&self, instance_id: Uuid, fiber_id: Uuid) -> Result<Option<Fiber>>;
    async fn load_fibers(&self, instance_id: Uuid) -> Result<Vec<Fiber>>;
    async fn delete_fiber(&self, instance_id: Uuid, fiber_id: Uuid) -> Result<()>;
    async fn delete_all_fibers(&self, instance_id: Uuid) -> Result<()>;

    // ── Join barriers ──

    /// Increment the arrive count for a join barrier. Returns the new count.
    async fn join_arrive(&self, instance_id: Uuid, join_id: JoinId) -> Result<u16>;
    async fn join_reset(&self, instance_id: Uuid, join_id: JoinId) -> Result<()>;
    async fn join_delete_all(&self, instance_id: Uuid) -> Result<()>;

    // ── Dedupe cache (engine-side) ──

    async fn dedupe_get(&self, key: &str) -> Result<Option<JobCompletion>>;
    async fn dedupe_put(&self, key: &str, completion: &JobCompletion) -> Result<()>;

    // ── Job queue ──

    async fn enqueue_job(&self, activation: &JobActivation) -> Result<()>;
    async fn dequeue_jobs(&self, task_types: &[String], max: usize) -> Result<Vec<JobActivation>>;
    async fn ack_job(&self, job_key: &str) -> Result<()>;

    /// Cancel all pending and inflight jobs for an instance.
    /// Returns the list of cancelled job_keys.
    async fn cancel_jobs_for_instance(&self, instance_id: Uuid) -> Result<Vec<String>>;

    // ── Program store (versioned bytecode) ──

    async fn store_program(&self, version: [u8; 32], program: &CompiledProgram) -> Result<()>;
    async fn load_program(&self, version: [u8; 32]) -> Result<Option<CompiledProgram>>;

    // ── Dead-letter queue ──

    async fn dead_letter_put(
        &self,
        name: u32,
        corr_key: &Value,
        payload: &[u8],
        ttl_ms: u64,
    ) -> Result<()>;
    async fn dead_letter_take(&self, name: u32, corr_key: &Value) -> Result<Option<Vec<u8>>>;

    // ── Event log (append-only) ──

    /// Append an event and return its sequence number.
    async fn append_event(&self, instance_id: Uuid, event: &RuntimeEvent) -> Result<u64>;
    async fn read_events(
        &self,
        instance_id: Uuid,
        from_seq: u64,
    ) -> Result<Vec<(u64, RuntimeEvent)>>;

    // ── Payload history (for PITR) ──

    async fn save_payload_version(
        &self,
        instance_id: Uuid,
        hash: &[u8; 32],
        payload: &str,
    ) -> Result<()>;
    async fn load_payload_version(
        &self,
        instance_id: Uuid,
        hash: &[u8; 32],
    ) -> Result<Option<String>>;

    // ── Incidents ──

    async fn save_incident(&self, incident: &Incident) -> Result<()>;
    async fn load_incidents(&self, instance_id: Uuid) -> Result<Vec<Incident>>;
}
