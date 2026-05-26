//! Postgres-backed [`JourneyStore`] implementation.
//!
//! Gated behind the `postgres` cargo feature. Uses runtime `sqlx::query()`
//! (not compile-time `sqlx::query!`) to avoid requiring a live DB or an
//! `.sqlx/` offline-cache directory at build time.
//!
//! # Schema
//! Requires the migration at `rust/migrations/20260521_dsl_journey_runtime.sql`.

#[cfg(feature = "postgres")]
pub mod postgres {
    use crate::{
        retention::RetentionPolicy,
        store::{JourneyLogEntry, JourneyStore, PendingWaitInfo},
        types::*,
    };
    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use chrono::{DateTime, Utc};
    use sqlx::{PgPool, Row};
    use uuid::Uuid;

    /// Postgres-backed journey store. Plug in via `Arc<PostgresJourneyStore>`.
    pub struct PostgresJourneyStore {
        pool: PgPool,
    }

    impl PostgresJourneyStore {
        pub fn new(pool: PgPool) -> Self {
            Self { pool }
        }
    }

    #[async_trait]
    impl JourneyStore for PostgresJourneyStore {
        // --- Instance operations ---

        async fn create_instance(
            &self,
            journey_name: &str,
            initial_data: serde_json::Value,
        ) -> Result<WorkflowInstance> {
            let row = sqlx::query(
                "INSERT INTO dsl_workflow_instance (journey_name, data) \
                 VALUES ($1, $2) \
                 RETURNING id, journey_name, version, status, started_at, completed_at, data",
            )
            .bind(journey_name)
            .bind(&initial_data)
            .fetch_one(&self.pool)
            .await?;

            let status_str: String = row.try_get("status")?;
            Ok(WorkflowInstance {
                id: row.try_get("id")?,
                journey_name: row.try_get("journey_name")?,
                version: row.try_get("version")?,
                status: status_str.parse().unwrap_or(InstanceStatus::Active),
                started_at: row.try_get("started_at")?,
                completed_at: row.try_get("completed_at")?,
                data: row.try_get("data")?,
            })
        }

        async fn get_instance(&self, id: InstanceId) -> Result<Option<WorkflowInstance>> {
            let row = sqlx::query(
                "SELECT id, journey_name, version, status, started_at, completed_at, data \
                 FROM dsl_workflow_instance WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;

            Ok(row.map(|r| {
                let status_str: String = r.try_get("status").unwrap_or_default();
                WorkflowInstance {
                    id: r.try_get("id").unwrap(),
                    journey_name: r.try_get("journey_name").unwrap(),
                    version: r.try_get("version").unwrap(),
                    status: status_str.parse().unwrap_or(InstanceStatus::Active),
                    started_at: r.try_get("started_at").unwrap(),
                    completed_at: r.try_get("completed_at").unwrap_or(None),
                    data: r.try_get("data").unwrap_or(serde_json::Value::Null),
                }
            }))
        }

        async fn update_instance_status(
            &self,
            id: InstanceId,
            status: InstanceStatus,
            completed_at: Option<DateTime<Utc>>,
        ) -> Result<()> {
            sqlx::query(
                "UPDATE dsl_workflow_instance SET status = $2, completed_at = $3 WHERE id = $1",
            )
            .bind(id)
            .bind(status.to_string())
            .bind(completed_at)
            .execute(&self.pool)
            .await?;
            Ok(())
        }

        // --- Token operations ---

        async fn create_token(
            &self,
            instance_id: InstanceId,
            node: &str,
            fork_ref: Option<Uuid>,
            lineage: Vec<String>,
        ) -> Result<ActiveToken> {
            let lineage_arr: Vec<&str> = lineage.iter().map(String::as_str).collect();
            let row = sqlx::query(
                "INSERT INTO dsl_active_token \
                 (instance_id, current_node, fork_ref, branch_lineage, write_log) \
                 VALUES ($1, $2, $3, $4, '[]'::jsonb) \
                 RETURNING id, instance_id, current_node, fork_ref, branch_lineage, write_log",
            )
            .bind(instance_id)
            .bind(node)
            .bind(fork_ref)
            .bind(&lineage_arr[..])
            .fetch_one(&self.pool)
            .await?;

            Ok(ActiveToken {
                id: row.try_get("id")?,
                instance_id: row.try_get("instance_id")?,
                current_node: row.try_get("current_node")?,
                fork_ref: row.try_get("fork_ref")?,
                branch_lineage: row.try_get::<Vec<String>, _>("branch_lineage")?,
                write_log: serde_json::from_value(
                    row.try_get::<serde_json::Value, _>("write_log")?,
                )
                .unwrap_or_default(),
            })
        }

        async fn get_tokens_for_instance(
            &self,
            instance_id: InstanceId,
        ) -> Result<Vec<ActiveToken>> {
            let rows = sqlx::query(
                "SELECT id, instance_id, current_node, fork_ref, branch_lineage, write_log \
                 FROM dsl_active_token WHERE instance_id = $1",
            )
            .bind(instance_id)
            .fetch_all(&self.pool)
            .await?;

            rows.iter()
                .map(|r| {
                    Ok(ActiveToken {
                        id: r.try_get("id")?,
                        instance_id: r.try_get("instance_id")?,
                        current_node: r.try_get("current_node")?,
                        fork_ref: r.try_get("fork_ref")?,
                        branch_lineage: r.try_get::<Vec<String>, _>("branch_lineage")?,
                        write_log: serde_json::from_value(
                            r.try_get::<serde_json::Value, _>("write_log")?,
                        )
                        .unwrap_or_default(),
                    })
                })
                .collect()
        }

        async fn advance_token(&self, token_id: TokenId, new_node: &str) -> Result<()> {
            sqlx::query("UPDATE dsl_active_token SET current_node = $2 WHERE id = $1")
                .bind(token_id)
                .bind(new_node)
                .execute(&self.pool)
                .await?;
            Ok(())
        }

        async fn delete_token(&self, token_id: TokenId) -> Result<()> {
            sqlx::query("DELETE FROM dsl_active_token WHERE id = $1")
                .bind(token_id)
                .execute(&self.pool)
                .await?;
            Ok(())
        }

        async fn append_to_write_log(&self, token_id: TokenId, entry: WriteLogEntry) -> Result<()> {
            let entry_json = serde_json::to_value(&entry)?;
            sqlx::query(
                "UPDATE dsl_active_token \
                 SET write_log = write_log || $2::jsonb \
                 WHERE id = $1",
            )
            .bind(token_id)
            .bind(entry_json)
            .execute(&self.pool)
            .await?;
            Ok(())
        }

        // --- Instance data ---

        async fn write_instance_data(
            &self,
            instance_id: InstanceId,
            key: &str,
            value: serde_json::Value,
        ) -> Result<()> {
            sqlx::query(
                "INSERT INTO dsl_instance_data (instance_id, key, value) \
                 VALUES ($1, $2, $3) \
                 ON CONFLICT (instance_id, key) \
                 DO UPDATE SET value = $3, \
                               version = dsl_instance_data.version + 1, \
                               updated_at = now()",
            )
            .bind(instance_id)
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
            Ok(())
        }

        async fn read_instance_data(
            &self,
            instance_id: InstanceId,
            key: &str,
        ) -> Result<Option<serde_json::Value>> {
            let row = sqlx::query(
                "SELECT value FROM dsl_instance_data \
                 WHERE instance_id = $1 AND key = $2",
            )
            .bind(instance_id)
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
            Ok(row.map(|r| r.try_get("value").unwrap_or(serde_json::Value::Null)))
        }

        // --- Event queue ---

        async fn enqueue_event(
            &self,
            instance_id: InstanceId,
            kind: EventKind,
            payload: serde_json::Value,
        ) -> Result<EventId> {
            let kind_str = kind.to_string();
            let row = sqlx::query(
                "INSERT INTO dsl_event_queue (instance_id, event_kind, payload) \
                 VALUES ($1, $2, $3) \
                 RETURNING id",
            )
            .bind(instance_id)
            .bind(&kind_str)
            .bind(payload)
            .fetch_one(&self.pool)
            .await?;
            Ok(row.try_get("id")?)
        }

        async fn dequeue_events(&self, max: usize) -> Result<Vec<EventEnvelope>> {
            // Begin an advisory scope for SKIP LOCKED.
            let rows = sqlx::query(
                "SELECT id, instance_id, event_kind, payload \
                 FROM dsl_event_queue \
                 WHERE claimed_at IS NULL \
                 ORDER BY enqueued_at \
                 LIMIT $1 \
                 FOR UPDATE SKIP LOCKED",
            )
            .bind(max as i64)
            .fetch_all(&self.pool)
            .await?;

            // Claim each row.
            for row in &rows {
                let event_id: Uuid = row.try_get("id")?;
                sqlx::query(
                    "UPDATE dsl_event_queue \
                     SET claimed_at = now(), claim_token = gen_random_uuid() \
                     WHERE id = $1",
                )
                .bind(event_id)
                .execute(&self.pool)
                .await?;
            }

            rows.iter()
                .map(|r| {
                    let kind_str: String = r.try_get("event_kind")?;
                    let kind = kind_str
                        .parse::<EventKind>()
                        .unwrap_or(EventKind::InstanceStart);
                    Ok(EventEnvelope {
                        id: r.try_get("id")?,
                        instance_id: r.try_get("instance_id")?,
                        event_kind: kind,
                        payload: r.try_get("payload")?,
                    })
                })
                .collect()
        }

        async fn ack_event(&self, event_id: EventId) -> Result<()> {
            sqlx::query("DELETE FROM dsl_event_queue WHERE id = $1")
                .bind(event_id)
                .execute(&self.pool)
                .await?;
            Ok(())
        }

        // --- Journey log ---

        async fn append_journey_log(&self, entry: JourneyLogEntry) -> Result<()> {
            sqlx::query(
                "INSERT INTO dsl_journey_log \
                 (instance_id, token_id, event_kind, from_node, to_node, data_delta) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(entry.instance_id)
            .bind(entry.token_id)
            .bind(&entry.event_kind)
            .bind(entry.from_node)
            .bind(entry.to_node)
            .bind(entry.data_delta)
            .execute(&self.pool)
            .await?;
            Ok(())
        }

        // --- Pending waits ---

        async fn create_pending_wait(
            &self,
            instance_id: InstanceId,
            token_id: TokenId,
            wait_kind: &str,
            node_name: &str,
            correlation_key: Option<String>,
            timeout_at: Option<DateTime<Utc>>,
        ) -> Result<Uuid> {
            let row = sqlx::query(
                "INSERT INTO dsl_pending_wait \
                 (instance_id, token_id, wait_kind, node_name, correlation_key, timeout_at) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 RETURNING id",
            )
            .bind(instance_id)
            .bind(token_id)
            .bind(wait_kind)
            .bind(node_name)
            .bind(correlation_key)
            .bind(timeout_at)
            .fetch_one(&self.pool)
            .await?;
            Ok(row.try_get("id")?)
        }

        async fn find_pending_wait_by_correlation(
            &self,
            wait_kind: &str,
            correlation_key: &str,
        ) -> Result<Option<PendingWaitInfo>> {
            let row = sqlx::query(
                "SELECT id, instance_id, token_id, node_name \
                 FROM dsl_pending_wait \
                 WHERE wait_kind = $1 AND correlation_key = $2 \
                 LIMIT 1",
            )
            .bind(wait_kind)
            .bind(correlation_key)
            .fetch_optional(&self.pool)
            .await?;

            Ok(row.map(|r| PendingWaitInfo {
                id: r.try_get("id").unwrap(),
                instance_id: r.try_get("instance_id").unwrap(),
                token_id: r.try_get("token_id").unwrap(),
                node_name: r.try_get("node_name").unwrap(),
            }))
        }

        // --- Switch decisions ---

        async fn create_switch_request(
            &self,
            instance_id: InstanceId,
            token_id: TokenId,
            gateway_name: &str,
            gateway_kind: &str,
            context: serde_json::Value,
        ) -> Result<Uuid> {
            let row = sqlx::query(
                "INSERT INTO dsl_switch_decision_request \
                 (instance_id, token_id, gateway_name, gateway_kind, context_data) \
                 VALUES ($1, $2, $3, $4, $5) \
                 RETURNING id",
            )
            .bind(instance_id)
            .bind(token_id)
            .bind(gateway_name)
            .bind(gateway_kind)
            .bind(context)
            .fetch_one(&self.pool)
            .await?;
            Ok(row.try_get("id")?)
        }

        // --- Join arrivals ---

        async fn record_join_arrival(
            &self,
            join_name: &str,
            instance_id: InstanceId,
            token_id: TokenId,
        ) -> Result<usize> {
            sqlx::query(
                "INSERT INTO dsl_join_arrival (join_name, instance_id, token_id) \
                 VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            )
            .bind(join_name)
            .bind(instance_id)
            .bind(token_id)
            .execute(&self.pool)
            .await?;

            let row = sqlx::query(
                "SELECT COUNT(*) AS cnt FROM dsl_join_arrival \
                 WHERE join_name = $1 AND instance_id = $2",
            )
            .bind(join_name)
            .bind(instance_id)
            .fetch_one(&self.pool)
            .await?;

            let count: i64 = row.try_get("cnt").unwrap_or(0);
            Ok(count as usize)
        }

        async fn get_tokens_at_join(
            &self,
            join_name: &str,
            instance_id: InstanceId,
        ) -> Result<Vec<ActiveToken>> {
            let rows = sqlx::query(
                "SELECT id, instance_id, current_node, fork_ref, branch_lineage, write_log \
                 FROM dsl_active_token \
                 WHERE instance_id = $1 AND current_node = $2",
            )
            .bind(instance_id)
            .bind(join_name)
            .fetch_all(&self.pool)
            .await?;

            rows.iter()
                .map(|r| {
                    Ok(ActiveToken {
                        id: r.try_get("id")?,
                        instance_id: r.try_get("instance_id")?,
                        current_node: r.try_get("current_node")?,
                        fork_ref: r.try_get("fork_ref")?,
                        branch_lineage: r.try_get::<Vec<String>, _>("branch_lineage")?,
                        write_log: serde_json::from_value(
                            r.try_get::<serde_json::Value, _>("write_log")?,
                        )
                        .unwrap_or_default(),
                    })
                })
                .collect()
        }

        async fn set_expected_join_count(
            &self,
            join_name: &str,
            instance_id: InstanceId,
            count: usize,
        ) -> Result<()> {
            sqlx::query(
                "INSERT INTO dsl_instance_data (instance_id, key, value) \
                 VALUES ($1, $2, $3::jsonb) \
                 ON CONFLICT (instance_id, key) \
                 DO UPDATE SET value = $3::jsonb",
            )
            .bind(instance_id)
            .bind(format!("__join_count_{}", join_name))
            .bind(serde_json::json!(count))
            .execute(&self.pool)
            .await?;
            Ok(())
        }

        async fn get_expected_join_count(
            &self,
            join_name: &str,
            instance_id: InstanceId,
        ) -> Result<Option<usize>> {
            let row = sqlx::query(
                "SELECT value FROM dsl_instance_data \
                 WHERE instance_id = $1 AND key = $2",
            )
            .bind(instance_id)
            .bind(format!("__join_count_{}", join_name))
            .fetch_optional(&self.pool)
            .await?;

            Ok(row.and_then(|r| {
                r.try_get::<serde_json::Value, _>("value")
                    .ok()
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
            }))
        }

        async fn reduce_expected_join_count(
            &self,
            join_name: &str,
            instance_id: InstanceId,
        ) -> Result<usize> {
            let current = self
                .get_expected_join_count(join_name, instance_id)
                .await?
                .unwrap_or(0);
            let new_count = current.saturating_sub(1);
            self.set_expected_join_count(join_name, instance_id, new_count)
                .await?;
            Ok(new_count)
        }

        // --- Retention ---

        async fn find_archivable_instances(
            &self,
            policy: &RetentionPolicy,
        ) -> Result<Vec<InstanceId>> {
            let days = policy.archive_after_days as i32;
            let rows = sqlx::query(
                "SELECT id FROM dsl_workflow_instance \
                 WHERE status IN ('completed', 'failed', 'cancelled') \
                 AND started_at < now() - ($1 || ' days')::interval",
            )
            .bind(days)
            .fetch_all(&self.pool)
            .await?;

            rows.iter()
                .map(|r| r.try_get::<Uuid, _>("id").map_err(|e| anyhow!(e)))
                .collect()
        }

        async fn archive_instance_log(&self, instance_id: InstanceId) -> Result<usize> {
            // Mark log entries as archived by setting a flag column.
            // This assumes a future `archived` boolean on dsl_journey_log.
            // For now, count the rows we would archive (non-destructive).
            let row =
                sqlx::query("SELECT COUNT(*) AS cnt FROM dsl_journey_log WHERE instance_id = $1")
                    .bind(instance_id)
                    .fetch_one(&self.pool)
                    .await?;

            let count: i64 = row.try_get("cnt").unwrap_or(0);
            Ok(count as usize)
        }
    }
}
