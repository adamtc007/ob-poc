use crate::events::RuntimeEvent;
use crate::store::ProcessStore;
use crate::types::*;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::collections::BTreeMap;
use uuid::Uuid;

/// Serialize a `Value` into a deterministic string key for dead-letter lookup.
/// Must match MemoryStore's `value_key()` exactly.
fn value_key(v: &Value) -> String {
    match v {
        Value::Bool(b) => format!("b:{b}"),
        Value::I64(n) => format!("i:{n}"),
        Value::Str(s) => format!("s:{s}"),
        Value::Ref(r) => format!("r:{r}"),
    }
}

/// Deserialize a JSONB `Vec<Value>` into `[Value; 8]`, padding with `Value::Bool(false)` if short.
fn regs_from_json(json: serde_json::Value) -> Result<[Value; 8]> {
    let vec: Vec<Value> =
        serde_json::from_value(json).context("failed to deserialize fiber regs")?;
    if vec.len() > 8 {
        return Err(anyhow!(
            "fiber regs has {} elements, expected <= 8",
            vec.len()
        ));
    }
    let mut regs: [Value; 8] = std::array::from_fn(|_| Value::Bool(false));
    for (i, v) in vec.into_iter().enumerate() {
        regs[i] = v;
    }
    Ok(regs)
}

/// Convert a `[u8; 32]` BYTEA column loaded as `Vec<u8>` back to `[u8; 32]`.
fn bytes_to_hash(bytes: Vec<u8>) -> Result<[u8; 32]> {
    bytes
        .try_into()
        .map_err(|v: Vec<u8>| anyhow!("expected 32 bytes, got {}", v.len()))
}

/// Convert an epoch-ms i64 to a `chrono::DateTime<chrono::Utc>` for TIMESTAMPTZ binding.
fn epoch_ms_to_datetime(epoch_ms: i64) -> chrono::DateTime<chrono::Utc> {
    use chrono::TimeZone;
    let secs = epoch_ms / 1000;
    let nanos = ((epoch_ms % 1000) * 1_000_000) as u32;
    chrono::Utc
        .timestamp_opt(secs, nanos)
        .single()
        .unwrap_or_else(chrono::Utc::now)
}

/// PostgreSQL-backed implementation of `ProcessStore`.
pub struct PostgresProcessStore {
    pool: sqlx::PgPool,
}

impl PostgresProcessStore {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Run embedded migrations.
    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("failed to run bpmn-lite migrations")?;
        Ok(())
    }
}

#[async_trait]
impl ProcessStore for PostgresProcessStore {
    // ── Instance ──

    async fn save_instance(&self, instance: &ProcessInstance) -> Result<()> {
        let flags = serde_json::to_value(&instance.flags)?;
        let counters = serde_json::to_value(&instance.counters)?;
        let join_expected = serde_json::to_value(&instance.join_expected)?;
        let state = serde_json::to_value(&instance.state)?;
        let created_at = epoch_ms_to_datetime(instance.created_at);

        sqlx::query(
            r#"
            INSERT INTO process_instances (
                instance_id, process_key, bytecode_version, domain_payload,
                domain_payload_hash, flags, counters, join_expected, state,
                correlation_id, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (instance_id) DO UPDATE SET
                process_key = EXCLUDED.process_key,
                bytecode_version = EXCLUDED.bytecode_version,
                domain_payload = EXCLUDED.domain_payload,
                domain_payload_hash = EXCLUDED.domain_payload_hash,
                flags = EXCLUDED.flags,
                counters = EXCLUDED.counters,
                join_expected = EXCLUDED.join_expected,
                state = EXCLUDED.state,
                correlation_id = EXCLUDED.correlation_id
            "#,
        )
        .bind(instance.instance_id)
        .bind(&instance.process_key)
        .bind(&instance.bytecode_version[..])
        .bind(&instance.domain_payload)
        .bind(&instance.domain_payload_hash[..])
        .bind(&flags)
        .bind(&counters)
        .bind(&join_expected)
        .bind(&state)
        .bind(&instance.correlation_id)
        .bind(created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn load_instance(&self, id: Uuid) -> Result<Option<ProcessInstance>> {
        let row = sqlx::query(
            r#"
            SELECT instance_id, process_key, bytecode_version, domain_payload,
                   domain_payload_hash, flags, counters, join_expected, state,
                   correlation_id,
                   EXTRACT(EPOCH FROM created_at) * 1000 AS created_at_ms
            FROM process_instances
            WHERE instance_id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => Ok(None),
            Some(row) => {
                use sqlx::Row;
                let bytecode_version: Vec<u8> = row.get("bytecode_version");
                let domain_payload_hash: Vec<u8> = row.get("domain_payload_hash");
                let flags_json: serde_json::Value = row.get("flags");
                let counters_json: serde_json::Value = row.get("counters");
                let join_expected_json: serde_json::Value = row.get("join_expected");
                let state_json: serde_json::Value = row.get("state");
                let created_at_ms: f64 = row.get("created_at_ms");

                Ok(Some(ProcessInstance {
                    instance_id: row.get("instance_id"),
                    process_key: row.get("process_key"),
                    bytecode_version: bytes_to_hash(bytecode_version)?,
                    domain_payload: row.get("domain_payload"),
                    domain_payload_hash: bytes_to_hash(domain_payload_hash)?,
                    flags: serde_json::from_value(flags_json)?,
                    counters: serde_json::from_value(counters_json)?,
                    join_expected: serde_json::from_value(join_expected_json)?,
                    state: serde_json::from_value(state_json)?,
                    correlation_id: row.get("correlation_id"),
                    created_at: created_at_ms as i64,
                }))
            }
        }
    }

    async fn update_instance_state(&self, id: Uuid, state: ProcessState) -> Result<()> {
        let state_json = serde_json::to_value(&state)?;
        let result = sqlx::query("UPDATE process_instances SET state = $1 WHERE instance_id = $2")
            .bind(&state_json)
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("instance not found: {id}"));
        }
        Ok(())
    }

    async fn update_instance_flags(
        &self,
        id: Uuid,
        flags: &BTreeMap<FlagKey, Value>,
    ) -> Result<()> {
        let flags_json = serde_json::to_value(flags)?;
        let result = sqlx::query("UPDATE process_instances SET flags = $1 WHERE instance_id = $2")
            .bind(&flags_json)
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("instance not found: {id}"));
        }
        Ok(())
    }

    async fn update_instance_payload(
        &self,
        id: Uuid,
        payload: &str,
        hash: &[u8; 32],
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE process_instances SET domain_payload = $1, domain_payload_hash = $2 WHERE instance_id = $3",
        )
        .bind(payload)
        .bind(&hash[..])
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(anyhow!("instance not found: {id}"));
        }
        Ok(())
    }

    // ── Fibers ──

    async fn save_fiber(&self, instance_id: Uuid, fiber: &Fiber) -> Result<()> {
        let stack = serde_json::to_value(&fiber.stack)?;
        let regs = serde_json::to_value(&fiber.regs)?;
        let wait_state = serde_json::to_value(&fiber.wait)?;

        sqlx::query(
            r#"
            INSERT INTO fibers (instance_id, fiber_id, pc, stack, regs, wait_state, loop_epoch)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (instance_id, fiber_id) DO UPDATE SET
                pc = EXCLUDED.pc,
                stack = EXCLUDED.stack,
                regs = EXCLUDED.regs,
                wait_state = EXCLUDED.wait_state,
                loop_epoch = EXCLUDED.loop_epoch
            "#,
        )
        .bind(instance_id)
        .bind(fiber.fiber_id)
        .bind(fiber.pc as i32)
        .bind(&stack)
        .bind(&regs)
        .bind(&wait_state)
        .bind(fiber.loop_epoch as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn load_fiber(&self, instance_id: Uuid, fiber_id: Uuid) -> Result<Option<Fiber>> {
        let row = sqlx::query(
            "SELECT fiber_id, pc, stack, regs, wait_state, loop_epoch FROM fibers WHERE instance_id = $1 AND fiber_id = $2",
        )
        .bind(instance_id)
        .bind(fiber_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => Ok(None),
            Some(row) => {
                use sqlx::Row;
                let pc: i32 = row.get("pc");
                let stack_json: serde_json::Value = row.get("stack");
                let regs_json: serde_json::Value = row.get("regs");
                let wait_json: serde_json::Value = row.get("wait_state");
                let loop_epoch: i32 = row.get("loop_epoch");

                Ok(Some(Fiber {
                    fiber_id: row.get("fiber_id"),
                    pc: pc as u32,
                    stack: serde_json::from_value(stack_json)?,
                    regs: regs_from_json(regs_json)?,
                    wait: serde_json::from_value(wait_json)?,
                    loop_epoch: loop_epoch as u32,
                }))
            }
        }
    }

    async fn load_fibers(&self, instance_id: Uuid) -> Result<Vec<Fiber>> {
        let rows = sqlx::query(
            "SELECT fiber_id, pc, stack, regs, wait_state, loop_epoch FROM fibers WHERE instance_id = $1",
        )
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await?;

        let mut fibers = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            let pc: i32 = row.get("pc");
            let stack_json: serde_json::Value = row.get("stack");
            let regs_json: serde_json::Value = row.get("regs");
            let wait_json: serde_json::Value = row.get("wait_state");
            let loop_epoch: i32 = row.get("loop_epoch");

            fibers.push(Fiber {
                fiber_id: row.get("fiber_id"),
                pc: pc as u32,
                stack: serde_json::from_value(stack_json)?,
                regs: regs_from_json(regs_json)?,
                wait: serde_json::from_value(wait_json)?,
                loop_epoch: loop_epoch as u32,
            });
        }
        Ok(fibers)
    }

    async fn delete_fiber(&self, instance_id: Uuid, fiber_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM fibers WHERE instance_id = $1 AND fiber_id = $2")
            .bind(instance_id)
            .bind(fiber_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_all_fibers(&self, instance_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM fibers WHERE instance_id = $1")
            .bind(instance_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Join barriers ──

    async fn join_arrive(&self, instance_id: Uuid, join_id: JoinId) -> Result<u16> {
        let row = sqlx::query(
            r#"
            INSERT INTO join_barriers (instance_id, join_id, arrive_count)
            VALUES ($1, $2, 1)
            ON CONFLICT (instance_id, join_id) DO UPDATE
                SET arrive_count = join_barriers.arrive_count + 1
            RETURNING arrive_count
            "#,
        )
        .bind(instance_id)
        .bind(join_id as i32)
        .fetch_one(&self.pool)
        .await?;

        use sqlx::Row;
        let count: i16 = row.get("arrive_count");
        Ok(count as u16)
    }

    async fn join_reset(&self, instance_id: Uuid, join_id: JoinId) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO join_barriers (instance_id, join_id, arrive_count)
            VALUES ($1, $2, 0)
            ON CONFLICT (instance_id, join_id) DO UPDATE
                SET arrive_count = 0
            "#,
        )
        .bind(instance_id)
        .bind(join_id as i32)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn join_delete_all(&self, instance_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM join_barriers WHERE instance_id = $1")
            .bind(instance_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ── Dedupe cache ──

    async fn dedupe_get(&self, key: &str) -> Result<Option<JobCompletion>> {
        let row = sqlx::query("SELECT completion FROM dedupe_cache WHERE job_key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            None => Ok(None),
            Some(row) => {
                use sqlx::Row;
                let json: serde_json::Value = row.get("completion");
                Ok(Some(serde_json::from_value(json)?))
            }
        }
    }

    async fn dedupe_put(&self, key: &str, completion: &JobCompletion) -> Result<()> {
        let json = serde_json::to_value(completion)?;
        sqlx::query(
            r#"
            INSERT INTO dedupe_cache (job_key, completion)
            VALUES ($1, $2)
            ON CONFLICT (job_key) DO UPDATE SET completion = EXCLUDED.completion
            "#,
        )
        .bind(key)
        .bind(&json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Job queue ──

    async fn enqueue_job(&self, activation: &JobActivation) -> Result<()> {
        let orch_flags = serde_json::to_value(&activation.orch_flags)?;

        sqlx::query(
            r#"
            INSERT INTO job_queue (
                job_key, process_instance_id, task_type, service_task_id,
                domain_payload, domain_payload_hash, orch_flags, retries_remaining
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&activation.job_key)
        .bind(activation.process_instance_id)
        .bind(&activation.task_type)
        .bind(&activation.service_task_id)
        .bind(&activation.domain_payload)
        .bind(&activation.domain_payload_hash[..])
        .bind(&orch_flags)
        .bind(activation.retries_remaining as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn dequeue_jobs(&self, task_types: &[String], max: usize) -> Result<Vec<JobActivation>> {
        let rows = sqlx::query(
            r#"
            WITH claimed AS (
                SELECT job_key
                FROM job_queue
                WHERE status = 'pending'
                  AND task_type = ANY($1)
                ORDER BY created_at
                LIMIT $2
                FOR UPDATE SKIP LOCKED
            )
            UPDATE job_queue
            SET status = 'claimed', claimed_at = now()
            FROM claimed
            WHERE job_queue.job_key = claimed.job_key
            RETURNING job_queue.job_key,
                      job_queue.process_instance_id,
                      job_queue.task_type,
                      job_queue.service_task_id,
                      job_queue.domain_payload,
                      job_queue.domain_payload_hash,
                      job_queue.orch_flags,
                      job_queue.retries_remaining
            "#,
        )
        .bind(task_types)
        .bind(max as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            let hash: Vec<u8> = row.get("domain_payload_hash");
            let orch_flags_json: serde_json::Value = row.get("orch_flags");
            let retries: i32 = row.get("retries_remaining");

            result.push(JobActivation {
                job_key: row.get("job_key"),
                process_instance_id: row.get("process_instance_id"),
                task_type: row.get("task_type"),
                service_task_id: row.get("service_task_id"),
                domain_payload: row.get("domain_payload"),
                domain_payload_hash: bytes_to_hash(hash)?,
                orch_flags: serde_json::from_value(orch_flags_json)?,
                retries_remaining: retries as u32,
            });
        }
        Ok(result)
    }

    async fn ack_job(&self, job_key: &str) -> Result<()> {
        sqlx::query("DELETE FROM job_queue WHERE job_key = $1")
            .bind(job_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn cancel_jobs_for_instance(&self, instance_id: Uuid) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "DELETE FROM job_queue WHERE process_instance_id = $1 AND status IN ('pending', 'claimed') RETURNING job_key",
        )
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await?;

        use sqlx::Row;
        Ok(rows.iter().map(|r| r.get("job_key")).collect())
    }

    // ── Program store ──

    async fn store_program(&self, version: [u8; 32], program: &CompiledProgram) -> Result<()> {
        let json = serde_json::to_value(program)?;
        sqlx::query(
            r#"
            INSERT INTO compiled_programs (bytecode_version, program)
            VALUES ($1, $2)
            ON CONFLICT (bytecode_version) DO NOTHING
            "#,
        )
        .bind(&version[..])
        .bind(&json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn load_program(&self, version: [u8; 32]) -> Result<Option<CompiledProgram>> {
        let row = sqlx::query("SELECT program FROM compiled_programs WHERE bytecode_version = $1")
            .bind(&version[..])
            .fetch_optional(&self.pool)
            .await?;

        match row {
            None => Ok(None),
            Some(row) => {
                use sqlx::Row;
                let json: serde_json::Value = row.get("program");
                Ok(Some(serde_json::from_value(json)?))
            }
        }
    }

    // ── Dead-letter queue ──

    async fn dead_letter_put(
        &self,
        name: u32,
        corr_key: &Value,
        payload: &[u8],
        ttl_ms: u64,
    ) -> Result<()> {
        let key = value_key(corr_key);
        let expires_at = chrono::Utc::now() + chrono::Duration::milliseconds(ttl_ms as i64);

        sqlx::query(
            r#"
            INSERT INTO dead_letter_queue (name, corr_key, payload, expires_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (name, corr_key) DO UPDATE SET
                payload = EXCLUDED.payload,
                expires_at = EXCLUDED.expires_at
            "#,
        )
        .bind(name as i32)
        .bind(&key)
        .bind(payload)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn dead_letter_take(&self, name: u32, corr_key: &Value) -> Result<Option<Vec<u8>>> {
        let key = value_key(corr_key);

        let row = sqlx::query(
            "DELETE FROM dead_letter_queue WHERE name = $1 AND corr_key = $2 AND expires_at > now() RETURNING payload",
        )
        .bind(name as i32)
        .bind(&key)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => Ok(None),
            Some(row) => {
                use sqlx::Row;
                Ok(Some(row.get("payload")))
            }
        }
    }

    // ── Event log ──

    async fn append_event(&self, instance_id: Uuid, event: &RuntimeEvent) -> Result<u64> {
        let event_json = serde_json::to_value(event)?;

        let row = sqlx::query(
            r#"
            WITH seq AS (
                INSERT INTO event_sequences (instance_id, next_seq)
                VALUES ($1, 1)
                ON CONFLICT (instance_id) DO UPDATE
                    SET next_seq = event_sequences.next_seq + 1
                RETURNING next_seq
            )
            INSERT INTO event_log (instance_id, seq, event)
            SELECT $1, seq.next_seq, $2
            FROM seq
            RETURNING seq
            "#,
        )
        .bind(instance_id)
        .bind(&event_json)
        .fetch_one(&self.pool)
        .await?;

        use sqlx::Row;
        let seq: i64 = row.get("seq");
        Ok(seq as u64)
    }

    async fn read_events(
        &self,
        instance_id: Uuid,
        from_seq: u64,
    ) -> Result<Vec<(u64, RuntimeEvent)>> {
        let rows = sqlx::query(
            "SELECT seq, event FROM event_log WHERE instance_id = $1 AND seq >= $2 ORDER BY seq",
        )
        .bind(instance_id)
        .bind(from_seq as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            let seq: i64 = row.get("seq");
            let event_json: serde_json::Value = row.get("event");
            let event: RuntimeEvent = serde_json::from_value(event_json)?;
            events.push((seq as u64, event));
        }
        Ok(events)
    }

    // ── Payload history ──

    async fn save_payload_version(
        &self,
        instance_id: Uuid,
        hash: &[u8; 32],
        payload: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO payload_history (instance_id, payload_hash, domain_payload)
            VALUES ($1, $2, $3)
            ON CONFLICT (instance_id, payload_hash) DO NOTHING
            "#,
        )
        .bind(instance_id)
        .bind(&hash[..])
        .bind(payload)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn load_payload_version(
        &self,
        instance_id: Uuid,
        hash: &[u8; 32],
    ) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT domain_payload FROM payload_history WHERE instance_id = $1 AND payload_hash = $2",
        )
        .bind(instance_id)
        .bind(&hash[..])
        .fetch_optional(&self.pool)
        .await?;

        match row {
            None => Ok(None),
            Some(row) => {
                use sqlx::Row;
                Ok(Some(row.get("domain_payload")))
            }
        }
    }

    // ── Incidents ──

    async fn save_incident(&self, incident: &Incident) -> Result<()> {
        let error_class = serde_json::to_value(&incident.error_class)?;
        let created_at = epoch_ms_to_datetime(incident.created_at);
        let resolved_at = incident.resolved_at.map(epoch_ms_to_datetime);

        sqlx::query(
            r#"
            INSERT INTO incidents (
                incident_id, process_instance_id, fiber_id, service_task_id,
                bytecode_addr, error_class, message, retry_count,
                created_at, resolved_at, resolution
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(incident.incident_id)
        .bind(incident.process_instance_id)
        .bind(incident.fiber_id)
        .bind(&incident.service_task_id)
        .bind(incident.bytecode_addr as i32)
        .bind(&error_class)
        .bind(&incident.message)
        .bind(incident.retry_count as i32)
        .bind(created_at)
        .bind(resolved_at)
        .bind(&incident.resolution)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn load_incidents(&self, instance_id: Uuid) -> Result<Vec<Incident>> {
        let rows = sqlx::query(
            r#"
            SELECT incident_id, process_instance_id, fiber_id, service_task_id,
                   bytecode_addr, error_class, message, retry_count,
                   EXTRACT(EPOCH FROM created_at) * 1000 AS created_at_ms,
                   EXTRACT(EPOCH FROM resolved_at) * 1000 AS resolved_at_ms,
                   resolution
            FROM incidents
            WHERE process_instance_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(instance_id)
        .fetch_all(&self.pool)
        .await?;

        let mut incidents = Vec::with_capacity(rows.len());
        for row in rows {
            use sqlx::Row;
            let bytecode_addr: i32 = row.get("bytecode_addr");
            let error_class_json: serde_json::Value = row.get("error_class");
            let retry_count: i32 = row.get("retry_count");
            let created_at_ms: f64 = row.get("created_at_ms");
            let resolved_at_ms: Option<f64> = row.get("resolved_at_ms");

            incidents.push(Incident {
                incident_id: row.get("incident_id"),
                process_instance_id: row.get("process_instance_id"),
                fiber_id: row.get("fiber_id"),
                service_task_id: row.get("service_task_id"),
                bytecode_addr: bytecode_addr as u32,
                error_class: serde_json::from_value(error_class_json)?,
                message: row.get("message"),
                retry_count: retry_count as u32,
                created_at: created_at_ms as i64,
                resolved_at: resolved_at_ms.map(|ms| ms as i64),
                resolution: row.get("resolution"),
            });
        }
        Ok(incidents)
    }
}

#[cfg(all(test, feature = "postgres"))]
mod tests {
    use super::*;
    use crate::engine::BpmnLiteEngine;
    use sqlx::PgPool;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    async fn setup() -> (PgPool, PostgresProcessStore) {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
        let pool = PgPool::connect(&url).await.expect("connect to db");

        // Run migrations
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");

        // Truncate all tables
        sqlx::query("TRUNCATE process_instances CASCADE")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("TRUNCATE compiled_programs")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("TRUNCATE job_queue")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("TRUNCATE dedupe_cache")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("TRUNCATE dead_letter_queue")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("TRUNCATE event_sequences")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("TRUNCATE event_log")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("TRUNCATE payload_history")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("TRUNCATE incidents")
            .execute(&pool)
            .await
            .unwrap();

        let store = PostgresProcessStore::new(pool.clone());
        (pool, store)
    }

    fn sha2_hash(data: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hasher.finalize().into()
    }

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
            counters: BTreeMap::from([(0, 5), (1, 10)]),
            join_expected: BTreeMap::from([(0, 3)]),
            state: ProcessState::Running,
            correlation_id: "runbook-entry-1".to_string(),
            created_at: 1700000000000,
        }
    }

    /// T-PG-1: Instance round-trip
    #[tokio::test]
    #[ignore]
    async fn test_pg_instance_round_trip() {
        let (_pool, store) = setup().await;
        let id = Uuid::now_v7();
        let inst = make_instance(id);

        store.save_instance(&inst).await.unwrap();
        let loaded = store.load_instance(id).await.unwrap().unwrap();

        assert_eq!(loaded.instance_id, id);
        assert_eq!(loaded.process_key, "test-process");
        assert_eq!(loaded.domain_payload, inst.domain_payload);
        assert_eq!(loaded.domain_payload_hash, inst.domain_payload_hash);
        assert_eq!(loaded.bytecode_version, [0u8; 32]);
        assert_eq!(loaded.flags.len(), 2);
        assert_eq!(loaded.flags[&0], Value::Bool(true));
        assert_eq!(loaded.flags[&1], Value::I64(42));
        assert_eq!(loaded.counters[&0], 5);
        assert_eq!(loaded.counters[&1], 10);
        assert_eq!(loaded.join_expected[&0], 3);
        assert_eq!(loaded.state, ProcessState::Running);
        assert_eq!(loaded.correlation_id, "runbook-entry-1");
        // Timestamp round-trip: allow 1s drift due to ms→timestamptz→ms
        assert!((loaded.created_at - inst.created_at).abs() < 1000);
    }

    /// T-PG-2: Fiber round-trip
    #[tokio::test]
    #[ignore]
    async fn test_pg_fiber_round_trip() {
        let (_pool, store) = setup().await;
        let iid = Uuid::now_v7();
        let fid = Uuid::now_v7();

        // Need an instance first (FK constraint)
        store.save_instance(&make_instance(iid)).await.unwrap();

        let mut fiber = Fiber::new(fid, 10);
        fiber.wait = WaitState::Job {
            job_key: "job-123".to_string(),
        };
        fiber.stack.push(Value::I64(99));
        fiber.loop_epoch = 3;

        store.save_fiber(iid, &fiber).await.unwrap();
        let loaded = store.load_fiber(iid, fid).await.unwrap().unwrap();

        assert_eq!(loaded.fiber_id, fid);
        assert_eq!(loaded.pc, 10);
        assert_eq!(
            loaded.wait,
            WaitState::Job {
                job_key: "job-123".to_string()
            }
        );
        assert_eq!(loaded.stack, vec![Value::I64(99)]);
        assert_eq!(loaded.loop_epoch, 3);
        // Verify regs padded to 8
        assert_eq!(loaded.regs.len(), 8);

        // Delete
        store.delete_fiber(iid, fid).await.unwrap();
        assert!(store.load_fiber(iid, fid).await.unwrap().is_none());
    }

    /// T-PG-3: Join barrier
    #[tokio::test]
    #[ignore]
    async fn test_pg_join_barrier() {
        let (_pool, store) = setup().await;
        let iid = Uuid::now_v7();
        store.save_instance(&make_instance(iid)).await.unwrap();

        let join_id: JoinId = 0;
        assert_eq!(store.join_arrive(iid, join_id).await.unwrap(), 1);
        assert_eq!(store.join_arrive(iid, join_id).await.unwrap(), 2);
        assert_eq!(store.join_arrive(iid, join_id).await.unwrap(), 3);

        store.join_reset(iid, join_id).await.unwrap();
        assert_eq!(store.join_arrive(iid, join_id).await.unwrap(), 1);
    }

    /// T-PG-4: Dedupe
    #[tokio::test]
    #[ignore]
    async fn test_pg_dedupe() {
        let (_pool, store) = setup().await;
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

        // Idempotent put
        store.dedupe_put("job-abc", &completion).await.unwrap();
    }

    /// T-PG-5: Job queue
    #[tokio::test]
    #[ignore]
    async fn test_pg_job_queue() {
        let (_pool, store) = setup().await;
        let task_type = "create_case".to_string();
        let iid = Uuid::now_v7();
        store.save_instance(&make_instance(iid)).await.unwrap();

        for i in 0..3 {
            store
                .enqueue_job(&JobActivation {
                    job_key: format!("job-{i}"),
                    process_instance_id: iid,
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

        // Ack one
        store.ack_job(&batch1[0].job_key).await.unwrap();

        // Dequeue remaining
        let batch2 = store.dequeue_jobs(&[task_type.clone()], 10).await.unwrap();
        assert_eq!(batch2.len(), 1);
    }

    /// T-PG-6: Event log
    #[tokio::test]
    #[ignore]
    async fn test_pg_event_log() {
        let (_pool, store) = setup().await;
        let iid = Uuid::now_v7();
        store.save_instance(&make_instance(iid)).await.unwrap();

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

    /// T-PG-7: Payload history
    #[tokio::test]
    #[ignore]
    async fn test_pg_payload_history() {
        let (_pool, store) = setup().await;
        let iid = Uuid::now_v7();
        store.save_instance(&make_instance(iid)).await.unwrap();

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

        let bad_hash = [0xFFu8; 32];
        assert!(store
            .load_payload_version(iid, &bad_hash)
            .await
            .unwrap()
            .is_none());
    }

    /// T-PG-8: Program store
    #[tokio::test]
    #[ignore]
    async fn test_pg_program_store() {
        let (_pool, store) = setup().await;

        let program = CompiledProgram {
            bytecode_version: sha2_hash("test-program"),
            program: vec![Instr::End],
            debug_map: BTreeMap::new(),
            join_plan: BTreeMap::new(),
            wait_plan: BTreeMap::new(),
            race_plan: BTreeMap::new(),
            boundary_map: BTreeMap::new(),
            write_set: BTreeMap::new(),
            task_manifest: vec![],
            error_route_map: BTreeMap::new(),
        };

        let version = program.bytecode_version;
        store.store_program(version, &program).await.unwrap();

        let loaded = store.load_program(version).await.unwrap().unwrap();
        assert_eq!(loaded.bytecode_version, version);
        assert_eq!(loaded.program.len(), 1);

        // Idempotent store
        store.store_program(version, &program).await.unwrap();
    }

    /// T-PG-9: Dead letter
    #[tokio::test]
    #[ignore]
    async fn test_pg_dead_letter() {
        let (_pool, store) = setup().await;
        let name = 1u32;
        let corr_key = Value::Str(42);
        let payload = b"test-payload";

        // Put with 5s TTL
        store
            .dead_letter_put(name, &corr_key, payload, 5000)
            .await
            .unwrap();

        // Take immediately — should succeed
        let taken = store.dead_letter_take(name, &corr_key).await.unwrap();
        assert_eq!(taken, Some(payload.to_vec()));

        // Take again — gone
        let taken2 = store.dead_letter_take(name, &corr_key).await.unwrap();
        assert!(taken2.is_none());

        // Put with 0ms TTL (already expired)
        store
            .dead_letter_put(name, &corr_key, payload, 0)
            .await
            .unwrap();

        // Small delay to ensure expires_at is in the past
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Take expired — should be None
        let taken3 = store.dead_letter_take(name, &corr_key).await.unwrap();
        assert!(taken3.is_none());
    }

    /// T-PG-10: Incidents
    #[tokio::test]
    #[ignore]
    async fn test_pg_incidents() {
        let (_pool, store) = setup().await;
        let iid = Uuid::now_v7();
        store.save_instance(&make_instance(iid)).await.unwrap();

        for i in 0..2 {
            store
                .save_incident(&Incident {
                    incident_id: Uuid::now_v7(),
                    process_instance_id: iid,
                    fiber_id: Uuid::now_v7(),
                    service_task_id: format!("task-{i}"),
                    bytecode_addr: i * 10,
                    error_class: ErrorClass::Transient,
                    message: format!("error {i}"),
                    retry_count: 0,
                    created_at: 1700000000000 + (i as i64 * 1000),
                    resolved_at: None,
                    resolution: None,
                })
                .await
                .unwrap();
        }

        let loaded = store.load_incidents(iid).await.unwrap();
        assert_eq!(loaded.len(), 2);
    }

    /// T-PG-11: Instance updates
    #[tokio::test]
    #[ignore]
    async fn test_pg_instance_updates() {
        let (_pool, store) = setup().await;
        let id = Uuid::now_v7();
        store.save_instance(&make_instance(id)).await.unwrap();

        // Update state
        let new_state = ProcessState::Completed { at: 1700001000000 };
        store
            .update_instance_state(id, new_state.clone())
            .await
            .unwrap();
        let loaded = store.load_instance(id).await.unwrap().unwrap();
        assert_eq!(loaded.state, new_state);

        // Update flags
        let new_flags = BTreeMap::from([(5, Value::Bool(false))]);
        store.update_instance_flags(id, &new_flags).await.unwrap();
        let loaded = store.load_instance(id).await.unwrap().unwrap();
        assert_eq!(loaded.flags.len(), 1);
        assert_eq!(loaded.flags[&5], Value::Bool(false));

        // Update payload
        let new_payload = r#"{"updated":true}"#;
        let new_hash = sha2_hash(new_payload);
        store
            .update_instance_payload(id, new_payload, &new_hash)
            .await
            .unwrap();
        let loaded = store.load_instance(id).await.unwrap().unwrap();
        assert_eq!(loaded.domain_payload, new_payload);
        assert_eq!(loaded.domain_payload_hash, new_hash);
    }

    /// T-PG-12: Teardown (delete_all_fibers + join_delete_all)
    #[tokio::test]
    #[ignore]
    async fn test_pg_teardown() {
        let (_pool, store) = setup().await;
        let iid = Uuid::now_v7();
        store.save_instance(&make_instance(iid)).await.unwrap();

        // Save 3 fibers
        for _ in 0..3 {
            let fid = Uuid::now_v7();
            store.save_fiber(iid, &Fiber::new(fid, 0)).await.unwrap();
        }

        // Save 2 join barriers
        store.join_arrive(iid, 0).await.unwrap();
        store.join_arrive(iid, 1).await.unwrap();

        // delete_all_fibers
        store.delete_all_fibers(iid).await.unwrap();
        let fibers = store.load_fibers(iid).await.unwrap();
        assert!(fibers.is_empty());

        // join_delete_all
        store.join_delete_all(iid).await.unwrap();
        // Arrive again — should start at 1
        assert_eq!(store.join_arrive(iid, 0).await.unwrap(), 1);
    }

    /// T-PG-13: Concurrent dequeue (SKIP LOCKED)
    #[tokio::test]
    #[ignore]
    async fn test_pg_concurrent_dequeue() {
        let (_pool, store) = setup().await;
        let store = Arc::new(store);
        let task_type = "concurrent_task".to_string();
        let iid = Uuid::now_v7();
        store.save_instance(&make_instance(iid)).await.unwrap();

        // Enqueue 3 jobs
        for i in 0..3 {
            store
                .enqueue_job(&JobActivation {
                    job_key: format!("conc-job-{i}"),
                    process_instance_id: iid,
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

        // Spawn 3 concurrent dequeue tasks
        let mut handles = Vec::new();
        for _ in 0..3 {
            let s = store.clone();
            let tt = task_type.clone();
            handles.push(tokio::spawn(async move {
                s.dequeue_jobs(&[tt], 1).await.unwrap()
            }));
        }

        let mut all_keys = Vec::new();
        for h in handles {
            let jobs = h.await.unwrap();
            for j in jobs {
                all_keys.push(j.job_key);
            }
        }

        // Exactly 3 jobs, no duplicates
        all_keys.sort();
        all_keys.dedup();
        assert_eq!(all_keys.len(), 3);
    }

    /// Minimal single-task BPMN for T-PG-14.
    const SMOKE_BPMN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL">
  <bpmn:process id="smoke_proc" isExecutable="true">
    <bpmn:startEvent id="start" />
    <bpmn:serviceTask id="task1" name="do_work" />
    <bpmn:endEvent id="end" />
    <bpmn:sequenceFlow id="f1" sourceRef="start" targetRef="task1" />
    <bpmn:sequenceFlow id="f2" sourceRef="task1" targetRef="end" />
  </bpmn:process>
</bpmn:definitions>"#;

    /// T-PG-14: Full engine smoke test
    #[tokio::test]
    #[ignore]
    async fn test_pg_full_engine_smoke() {
        let (_pool, store) = setup().await;
        let store = Arc::new(store);
        let engine = BpmnLiteEngine::new(store.clone());

        // Compile
        let compiled = engine.compile(SMOKE_BPMN).await.unwrap();
        let version = compiled.bytecode_version;

        // Start process
        let payload = r#"{"case_id":"test-123"}"#;
        let hash = crate::vm::compute_hash(payload);
        let instance_id = engine
            .start("smoke_proc", version, payload, hash, "test-corr-1")
            .await
            .unwrap();

        // Tick to advance (produces jobs)
        engine.tick_instance(instance_id).await.unwrap();

        // Get task types from compile result
        let task_types = compiled.task_types;
        assert!(
            !task_types.is_empty(),
            "program should have at least one task"
        );

        // Dequeue job
        let jobs = store.dequeue_jobs(&task_types, 1).await.unwrap();
        assert_eq!(jobs.len(), 1, "should have 1 job");

        let job = &jobs[0];

        // Complete job
        let completion_payload = r#"{"result":"ok"}"#;
        let completion_hash = crate::vm::compute_hash(completion_payload);
        engine
            .complete_job(
                &job.job_key,
                completion_payload,
                completion_hash,
                BTreeMap::new(),
            )
            .await
            .unwrap();

        // Tick again to advance past the completed job
        engine.tick_instance(instance_id).await.unwrap();

        // Check instance state
        let inst = store.load_instance(instance_id).await.unwrap().unwrap();

        // Read events — should have at least InstanceStarted
        let events = store.read_events(instance_id, 1).await.unwrap();
        assert!(
            events.len() >= 2,
            "should have multiple events, got {}",
            events.len()
        );

        // First event should be InstanceStarted
        match &events[0].1 {
            RuntimeEvent::InstanceStarted { .. } => {}
            other => panic!("expected InstanceStarted, got {:?}", other),
        }

        // Single-task process should be Completed after completing the one job
        assert!(
            matches!(inst.state, ProcessState::Completed { .. }),
            "expected Completed, got {:?}",
            inst.state
        );
    }

    /// T-PG-15: cancel_jobs_for_instance
    #[tokio::test]
    #[ignore]
    async fn test_pg_cancel_jobs_for_instance() {
        let (_pool, store) = setup().await;
        let task_type = "cancel_test".to_string();

        let iid_a = Uuid::now_v7();
        let iid_b = Uuid::now_v7();
        store.save_instance(&make_instance(iid_a)).await.unwrap();

        let mut inst_b = make_instance(iid_b);
        inst_b.instance_id = iid_b;
        store.save_instance(&inst_b).await.unwrap();

        // 2 jobs for instance A, 1 for instance B
        for i in 0..2 {
            store
                .enqueue_job(&JobActivation {
                    job_key: format!("cancel-a-{i}"),
                    process_instance_id: iid_a,
                    task_type: task_type.clone(),
                    service_task_id: format!("task-a-{i}"),
                    domain_payload: "{}".to_string(),
                    domain_payload_hash: [0u8; 32],
                    orch_flags: BTreeMap::new(),
                    retries_remaining: 3,
                })
                .await
                .unwrap();
        }
        store
            .enqueue_job(&JobActivation {
                job_key: "cancel-b-0".to_string(),
                process_instance_id: iid_b,
                task_type: task_type.clone(),
                service_task_id: "task-b-0".to_string(),
                domain_payload: "{}".to_string(),
                domain_payload_hash: [0u8; 32],
                orch_flags: BTreeMap::new(),
                retries_remaining: 3,
            })
            .await
            .unwrap();

        // Cancel instance A's jobs
        let cancelled = store.cancel_jobs_for_instance(iid_a).await.unwrap();
        assert_eq!(cancelled.len(), 2);

        // Dequeue remaining — should only get B's job
        let remaining = store.dequeue_jobs(&[task_type], 10).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].job_key, "cancel-b-0");
    }
}
