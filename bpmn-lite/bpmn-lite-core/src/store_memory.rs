use crate::events::RuntimeEvent;
use crate::store::ProcessStore;
use crate::types::*;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap, VecDeque};
use tokio::sync::RwLock;
use uuid::Uuid;

struct Inner {
    instances: HashMap<Uuid, ProcessInstance>,
    fibers: HashMap<(Uuid, Uuid), Fiber>,
    join_counters: HashMap<(Uuid, JoinId), u16>,
    dedupe: HashMap<String, JobCompletion>,
    job_queue: VecDeque<JobActivation>,
    /// Jobs that have been dequeued but not yet acked.
    inflight_jobs: HashMap<String, JobActivation>,
    programs: HashMap<[u8; 32], CompiledProgram>,
    dead_letter: HashMap<(u32, String), (Vec<u8>, u64)>,
    events: HashMap<Uuid, Vec<(u64, RuntimeEvent)>>,
    event_seq: HashMap<Uuid, u64>,
    payload_history: HashMap<(Uuid, [u8; 32]), String>,
    incidents: HashMap<Uuid, Vec<Incident>>,
}

/// In-memory implementation of `ProcessStore` for POC/testing.
pub struct MemoryStore {
    inner: RwLock<Inner>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Inner {
                instances: HashMap::new(),
                fibers: HashMap::new(),
                join_counters: HashMap::new(),
                dedupe: HashMap::new(),
                job_queue: VecDeque::new(),
                inflight_jobs: HashMap::new(),
                programs: HashMap::new(),
                dead_letter: HashMap::new(),
                events: HashMap::new(),
                event_seq: HashMap::new(),
                payload_history: HashMap::new(),
                incidents: HashMap::new(),
            }),
        }
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Serialize a `Value` into a deterministic string key for dead-letter lookup.
fn value_key(v: &Value) -> String {
    match v {
        Value::Bool(b) => format!("b:{b}"),
        Value::I64(n) => format!("i:{n}"),
        Value::Str(s) => format!("s:{s}"),
        Value::Ref(r) => format!("r:{r}"),
    }
}

#[async_trait]
impl ProcessStore for MemoryStore {
    // ── Instance ──

    async fn save_instance(&self, instance: &ProcessInstance) -> Result<()> {
        let mut w = self.inner.write().await;
        w.instances.insert(instance.instance_id, instance.clone());
        Ok(())
    }

    async fn load_instance(&self, id: Uuid) -> Result<Option<ProcessInstance>> {
        let r = self.inner.read().await;
        Ok(r.instances.get(&id).cloned())
    }

    async fn update_instance_state(&self, id: Uuid, state: ProcessState) -> Result<()> {
        let mut w = self.inner.write().await;
        let inst = w
            .instances
            .get_mut(&id)
            .ok_or_else(|| anyhow!("instance not found: {id}"))?;
        inst.state = state;
        Ok(())
    }

    async fn update_instance_flags(
        &self,
        id: Uuid,
        flags: &BTreeMap<FlagKey, Value>,
    ) -> Result<()> {
        let mut w = self.inner.write().await;
        let inst = w
            .instances
            .get_mut(&id)
            .ok_or_else(|| anyhow!("instance not found: {id}"))?;
        inst.flags = flags.clone();
        Ok(())
    }

    async fn update_instance_payload(
        &self,
        id: Uuid,
        payload: &str,
        hash: &[u8; 32],
    ) -> Result<()> {
        let mut w = self.inner.write().await;
        let inst = w
            .instances
            .get_mut(&id)
            .ok_or_else(|| anyhow!("instance not found: {id}"))?;
        inst.domain_payload = payload.to_string();
        inst.domain_payload_hash = *hash;
        Ok(())
    }

    // ── Fibers ──

    async fn save_fiber(&self, instance_id: Uuid, fiber: &Fiber) -> Result<()> {
        let mut w = self.inner.write().await;
        w.fibers
            .insert((instance_id, fiber.fiber_id), fiber.clone());
        Ok(())
    }

    async fn load_fiber(&self, instance_id: Uuid, fiber_id: Uuid) -> Result<Option<Fiber>> {
        let r = self.inner.read().await;
        Ok(r.fibers.get(&(instance_id, fiber_id)).cloned())
    }

    async fn load_fibers(&self, instance_id: Uuid) -> Result<Vec<Fiber>> {
        let r = self.inner.read().await;
        Ok(r.fibers
            .iter()
            .filter(|((iid, _), _)| *iid == instance_id)
            .map(|(_, f)| f.clone())
            .collect())
    }

    async fn delete_fiber(&self, instance_id: Uuid, fiber_id: Uuid) -> Result<()> {
        let mut w = self.inner.write().await;
        w.fibers.remove(&(instance_id, fiber_id));
        Ok(())
    }

    async fn delete_all_fibers(&self, instance_id: Uuid) -> Result<()> {
        let mut w = self.inner.write().await;
        w.fibers.retain(|(iid, _), _| *iid != instance_id);
        Ok(())
    }

    // ── Join barriers ──

    async fn join_arrive(&self, instance_id: Uuid, join_id: JoinId) -> Result<u16> {
        let mut w = self.inner.write().await;
        let count = w.join_counters.entry((instance_id, join_id)).or_insert(0);
        *count += 1;
        Ok(*count)
    }

    async fn join_reset(&self, instance_id: Uuid, join_id: JoinId) -> Result<()> {
        let mut w = self.inner.write().await;
        w.join_counters.insert((instance_id, join_id), 0);
        Ok(())
    }

    async fn join_delete_all(&self, instance_id: Uuid) -> Result<()> {
        let mut w = self.inner.write().await;
        w.join_counters.retain(|(iid, _), _| *iid != instance_id);
        Ok(())
    }

    // ── Dedupe cache ──

    async fn dedupe_get(&self, key: &str) -> Result<Option<JobCompletion>> {
        let r = self.inner.read().await;
        Ok(r.dedupe.get(key).cloned())
    }

    async fn dedupe_put(&self, key: &str, completion: &JobCompletion) -> Result<()> {
        let mut w = self.inner.write().await;
        w.dedupe.insert(key.to_string(), completion.clone());
        Ok(())
    }

    // ── Job queue ──

    async fn enqueue_job(&self, activation: &JobActivation) -> Result<()> {
        let mut w = self.inner.write().await;
        w.job_queue.push_back(activation.clone());
        Ok(())
    }

    async fn dequeue_jobs(&self, task_types: &[String], max: usize) -> Result<Vec<JobActivation>> {
        let mut w = self.inner.write().await;
        let mut result = Vec::new();
        let mut remaining = VecDeque::new();

        while let Some(job) = w.job_queue.pop_front() {
            if result.len() < max && task_types.contains(&job.task_type) {
                w.inflight_jobs.insert(job.job_key.clone(), job.clone());
                result.push(job);
            } else {
                remaining.push_back(job);
            }
        }
        w.job_queue = remaining;
        Ok(result)
    }

    async fn ack_job(&self, job_key: &str) -> Result<()> {
        let mut w = self.inner.write().await;
        w.inflight_jobs.remove(job_key);
        Ok(())
    }

    async fn cancel_jobs_for_instance(&self, instance_id: Uuid) -> Result<Vec<String>> {
        let mut w = self.inner.write().await;
        let mut cancelled = Vec::new();

        // Remove from job_queue
        let mut remaining = VecDeque::new();
        while let Some(job) = w.job_queue.pop_front() {
            if job.process_instance_id == instance_id {
                cancelled.push(job.job_key.clone());
            } else {
                remaining.push_back(job);
            }
        }
        w.job_queue = remaining;

        // Remove from inflight_jobs
        let inflight_keys: Vec<String> = w
            .inflight_jobs
            .iter()
            .filter(|(_, job)| job.process_instance_id == instance_id)
            .map(|(key, _)| key.clone())
            .collect();
        for key in inflight_keys {
            w.inflight_jobs.remove(&key);
            cancelled.push(key);
        }

        Ok(cancelled)
    }

    // ── Program store ──

    async fn store_program(&self, version: [u8; 32], program: &CompiledProgram) -> Result<()> {
        let mut w = self.inner.write().await;
        w.programs.insert(version, program.clone());
        Ok(())
    }

    async fn load_program(&self, version: [u8; 32]) -> Result<Option<CompiledProgram>> {
        let r = self.inner.read().await;
        Ok(r.programs.get(&version).cloned())
    }

    // ── Dead-letter queue ──

    async fn dead_letter_put(
        &self,
        name: u32,
        corr_key: &Value,
        payload: &[u8],
        ttl_ms: u64,
    ) -> Result<()> {
        let mut w = self.inner.write().await;
        let key = (name, value_key(corr_key));
        w.dead_letter.insert(key, (payload.to_vec(), ttl_ms));
        Ok(())
    }

    async fn dead_letter_take(&self, name: u32, corr_key: &Value) -> Result<Option<Vec<u8>>> {
        let mut w = self.inner.write().await;
        let key = (name, value_key(corr_key));
        Ok(w.dead_letter.remove(&key).map(|(data, _)| data))
    }

    // ── Event log ──

    async fn append_event(&self, instance_id: Uuid, event: &RuntimeEvent) -> Result<u64> {
        let mut w = self.inner.write().await;
        let seq = w.event_seq.entry(instance_id).or_insert(0);
        *seq += 1;
        let current_seq = *seq;
        w.events
            .entry(instance_id)
            .or_default()
            .push((current_seq, event.clone()));
        Ok(current_seq)
    }

    async fn read_events(
        &self,
        instance_id: Uuid,
        from_seq: u64,
    ) -> Result<Vec<(u64, RuntimeEvent)>> {
        let r = self.inner.read().await;
        Ok(r.events
            .get(&instance_id)
            .map(|evts| {
                evts.iter()
                    .filter(|(seq, _)| *seq >= from_seq)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    // ── Payload history ──

    async fn save_payload_version(
        &self,
        instance_id: Uuid,
        hash: &[u8; 32],
        payload: &str,
    ) -> Result<()> {
        let mut w = self.inner.write().await;
        w.payload_history
            .insert((instance_id, *hash), payload.to_string());
        Ok(())
    }

    async fn load_payload_version(
        &self,
        instance_id: Uuid,
        hash: &[u8; 32],
    ) -> Result<Option<String>> {
        let r = self.inner.read().await;
        Ok(r.payload_history.get(&(instance_id, *hash)).cloned())
    }

    // ── Incidents ──

    async fn save_incident(&self, incident: &Incident) -> Result<()> {
        let mut w = self.inner.write().await;
        w.incidents
            .entry(incident.process_instance_id)
            .or_default()
            .push(incident.clone());
        Ok(())
    }

    async fn load_incidents(&self, instance_id: Uuid) -> Result<Vec<Incident>> {
        let r = self.inner.read().await;
        Ok(r.incidents.get(&instance_id).cloned().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn make_instance(id: Uuid) -> ProcessInstance {
        let payload = r#"{"case_id":"abc"}"#;
        let hash = sha2_hash(payload);
        ProcessInstance {
            instance_id: id,
            process_key: "test-process".to_string(),
            bytecode_version: [0u8; 32],
            domain_payload: payload.to_string(),
            domain_payload_hash: hash,
            flags: BTreeMap::from([(0, Value::Bool(true)), (1, Value::I64(42))]),
            counters: BTreeMap::new(),
            join_expected: BTreeMap::new(),
            state: ProcessState::Running,
            correlation_id: "runbook-entry-1".to_string(),
            created_at: 1000,
        }
    }

    fn sha2_hash(data: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hasher.finalize().into()
    }

    /// A2.T1: Save/load instance round-trip
    #[tokio::test]
    async fn test_instance_round_trip() {
        let store = MemoryStore::new();
        let id = Uuid::now_v7();
        let inst = make_instance(id);

        store.save_instance(&inst).await.unwrap();
        let loaded = store.load_instance(id).await.unwrap().unwrap();

        assert_eq!(loaded.instance_id, id);
        assert_eq!(loaded.domain_payload, inst.domain_payload);
        assert_eq!(loaded.domain_payload_hash, inst.domain_payload_hash);
        assert_eq!(loaded.flags.len(), 2);
        assert_eq!(loaded.flags[&0], Value::Bool(true));
        assert_eq!(loaded.flags[&1], Value::I64(42));
        assert_eq!(loaded.state, ProcessState::Running);
    }

    /// A2.T2: Save/load/delete fiber round-trip (including WaitState::Job)
    #[tokio::test]
    async fn test_fiber_round_trip() {
        let store = MemoryStore::new();
        let iid = Uuid::now_v7();
        let fid = Uuid::now_v7();

        let mut fiber = Fiber::new(fid, 0);
        fiber.wait = WaitState::Job {
            job_key: "job-123".to_string(),
        };
        fiber.stack.push(Value::I64(99));

        store.save_fiber(iid, &fiber).await.unwrap();
        let loaded = store.load_fiber(iid, fid).await.unwrap().unwrap();
        assert_eq!(loaded.fiber_id, fid);
        assert_eq!(
            loaded.wait,
            WaitState::Job {
                job_key: "job-123".to_string()
            }
        );
        assert_eq!(loaded.stack, vec![Value::I64(99)]);

        // Delete
        store.delete_fiber(iid, fid).await.unwrap();
        assert!(store.load_fiber(iid, fid).await.unwrap().is_none());
    }

    /// A2.T3: Join barrier: arrive 3 times, reset
    #[tokio::test]
    async fn test_join_barrier() {
        let store = MemoryStore::new();
        let iid = Uuid::now_v7();
        let join_id: JoinId = 0;

        assert_eq!(store.join_arrive(iid, join_id).await.unwrap(), 1);
        assert_eq!(store.join_arrive(iid, join_id).await.unwrap(), 2);
        assert_eq!(store.join_arrive(iid, join_id).await.unwrap(), 3);

        store.join_reset(iid, join_id).await.unwrap();
        assert_eq!(store.join_arrive(iid, join_id).await.unwrap(), 1);
    }

    /// A2.T4: Dedupe: put JobCompletion + get returns cached
    #[tokio::test]
    async fn test_dedupe() {
        let store = MemoryStore::new();
        let completion = JobCompletion {
            job_key: "job-abc".to_string(),
            domain_payload: r#"{"done":true}"#.to_string(),
            domain_payload_hash: sha2_hash(r#"{"done":true}"#),
            orch_flags: BTreeMap::new(),
        };

        assert!(store.dedupe_get("job-abc").await.unwrap().is_none());
        store.dedupe_put("job-abc", &completion).await.unwrap();

        let cached = store.dedupe_get("job-abc").await.unwrap().unwrap();
        assert_eq!(cached.job_key, "job-abc");
        assert_eq!(cached.domain_payload, r#"{"done":true}"#);
    }

    /// A2.T5: Job queue: enqueue 3, dequeue 2, ack 1, dequeue 1 remaining
    #[tokio::test]
    async fn test_job_queue() {
        let store = MemoryStore::new();
        let task_type = "create_case".to_string();

        for i in 0..3 {
            store
                .enqueue_job(&JobActivation {
                    job_key: format!("job-{i}"),
                    process_instance_id: Uuid::now_v7(),
                    task_type: task_type.clone(),
                    service_task_id: format!("task-{i}"),
                    domain_payload: "{}".to_string(),
                    domain_payload_hash: [0u8; 32],
                    orch_flags: BTreeMap::new(),
                    retries_remaining: 3,
                })
                .await
                .unwrap();
        }

        // Dequeue 2
        let batch1 = store.dequeue_jobs(&[task_type.clone()], 2).await.unwrap();
        assert_eq!(batch1.len(), 2);
        assert_eq!(batch1[0].job_key, "job-0");
        assert_eq!(batch1[1].job_key, "job-1");

        // Ack one
        store.ack_job("job-0").await.unwrap();

        // Dequeue remaining
        let batch2 = store.dequeue_jobs(&[task_type.clone()], 10).await.unwrap();
        assert_eq!(batch2.len(), 1);
        assert_eq!(batch2[0].job_key, "job-2");
    }

    /// A2.T6: Event log: append 5 events, read from seq 3 returns 3 events
    #[tokio::test]
    async fn test_event_log() {
        let store = MemoryStore::new();
        let iid = Uuid::now_v7();

        for i in 0..5 {
            let event = RuntimeEvent::FlagSet {
                key: i,
                value: Value::I64(i as i64),
            };
            let seq = store.append_event(iid, &event).await.unwrap();
            assert_eq!(seq, (i + 1) as u64);
        }

        let events = store.read_events(iid, 3).await.unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].0, 3);
        assert_eq!(events[1].0, 4);
        assert_eq!(events[2].0, 5);
    }

    /// A2.T7: Payload history: save 2 versions, load by hash
    #[tokio::test]
    async fn test_payload_history() {
        let store = MemoryStore::new();
        let iid = Uuid::now_v7();

        let payload_v1 = r#"{"version":1}"#;
        let hash_v1 = sha2_hash(payload_v1);
        store
            .save_payload_version(iid, &hash_v1, payload_v1)
            .await
            .unwrap();

        let payload_v2 = r#"{"version":2}"#;
        let hash_v2 = sha2_hash(payload_v2);
        store
            .save_payload_version(iid, &hash_v2, payload_v2)
            .await
            .unwrap();

        let loaded_v1 = store
            .load_payload_version(iid, &hash_v1)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded_v1, payload_v1);

        let loaded_v2 = store
            .load_payload_version(iid, &hash_v2)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(loaded_v2, payload_v2);

        // Non-existent hash returns None
        let bad_hash = [0xFFu8; 32];
        assert!(store
            .load_payload_version(iid, &bad_hash)
            .await
            .unwrap()
            .is_none());
    }
}
