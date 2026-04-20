//! JobWorker — long-poll loop that activates jobs from bpmn-lite,
//! executes the corresponding ob-poc verb, and completes/fails the job.
//!
//! ## Execution Model (Forth-style)
//!
//! - **PUSH is persisted**: On job activation, upsert a `JobFrame` (dedupe key).
//! - **POP is idempotent**: On redelivery, return cached completion.
//! - Deduplication prevents re-execution of already-completed jobs.
//!
//! ## Lifecycle
//!
//! ```text
//! JobWorker::run(shutdown_rx) loop:
//!   1. Collect task_types from WorkflowConfig
//!   2. Long-poll via ActivateJobs (streaming, timeout_ms)
//!   3. For each job:
//!      a. Dedupe: JobFrameStore.upsert() — if already completed, skip
//!      b. Look up task_type → verb_fqn via TaskBinding
//!      c. Build DSL from domain_payload + verb_fqn
//!      d. Execute via inner DslExecutorV2
//!      e. Success: complete_job gRPC → mark_completed
//!      f. Failure: fail_job gRPC → mark_failed
//!   4. Sleep 1s if no jobs (backoff)
//! ```

use std::sync::Arc;
use tokio::sync::watch;
use uuid::Uuid;

use super::canonical::canonical_json_with_hash;
use super::client::{BpmnLiteConnection, CompleteJobRequest, JobActivation};
use super::config::WorkflowConfigIndex;
use super::job_frames::JobFrameStore;
use super::types::{JobFrame, JobFrameStatus};
use crate::sequencer::DslExecutorV2;

/// Default long-poll timeout in milliseconds.
const DEFAULT_POLL_TIMEOUT_MS: i64 = 30_000;

/// Maximum jobs to activate per poll cycle.
const DEFAULT_MAX_JOBS: i32 = 10;

/// Backoff sleep when no jobs are available.
const BACKOFF_SLEEP_MS: u64 = 1_000;

// ---------------------------------------------------------------------------
// JobWorker
// ---------------------------------------------------------------------------

/// Long-poll job worker that bridges bpmn-lite jobs to ob-poc verb execution.
pub struct JobWorker {
    /// Worker identity (for logging and job tracking).
    worker_id: String,
    /// gRPC connection to bpmn-lite.
    bpmn_client: BpmnLiteConnection,
    /// Workflow config for task_type → verb lookup.
    config: Arc<WorkflowConfigIndex>,
    /// Job frame store for deduplication.
    job_frames: JobFrameStore,
    /// Inner executor for running verbs.
    executor: Arc<dyn DslExecutorV2>,
}

impl JobWorker {
    pub fn new(
        worker_id: String,
        bpmn_client: BpmnLiteConnection,
        config: Arc<WorkflowConfigIndex>,
        job_frames: JobFrameStore,
        executor: Arc<dyn DslExecutorV2>,
    ) -> Self {
        Self {
            worker_id,
            bpmn_client,
            config,
            job_frames,
            executor,
        }
    }

    /// Run the worker loop until the shutdown signal is received.
    pub async fn run(&self, mut shutdown_rx: watch::Receiver<bool>) {
        tracing::info!(
            worker_id = %self.worker_id,
            "JobWorker started"
        );

        loop {
            // Check shutdown signal.
            if *shutdown_rx.borrow() {
                tracing::info!(worker_id = %self.worker_id, "JobWorker shutting down");
                break;
            }

            match self.poll_and_execute().await {
                Ok(jobs_processed) => {
                    if jobs_processed == 0 {
                        // No jobs — backoff before next poll.
                        tokio::select! {
                            _ = tokio::time::sleep(tokio::time::Duration::from_millis(BACKOFF_SLEEP_MS)) => {},
                            _ = shutdown_rx.changed() => {
                                tracing::info!(worker_id = %self.worker_id, "JobWorker shutting down (during backoff)");
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        worker_id = %self.worker_id,
                        error = %e,
                        "JobWorker poll cycle failed, backing off"
                    );
                    // Longer backoff on error.
                    tokio::select! {
                        _ = tokio::time::sleep(tokio::time::Duration::from_millis(BACKOFF_SLEEP_MS * 5)) => {},
                        _ = shutdown_rx.changed() => break,
                    }
                }
            }
        }

        tracing::info!(worker_id = %self.worker_id, "JobWorker stopped");
    }

    /// Execute a single poll cycle. Returns the number of jobs processed.
    pub async fn poll_and_execute(&self) -> anyhow::Result<usize> {
        let task_types = self.config.all_task_types();
        if task_types.is_empty() {
            return Ok(0);
        }

        // Long-poll for jobs.
        let jobs = self
            .bpmn_client
            .activate_jobs(
                task_types,
                DEFAULT_MAX_JOBS,
                DEFAULT_POLL_TIMEOUT_MS,
                &self.worker_id,
            )
            .await?;

        let count = jobs.len();
        for job in jobs {
            self.process_job(job).await;
        }

        Ok(count)
    }

    /// Process a single activated job.
    async fn process_job(&self, job: JobActivation) {
        let job_key = &job.job_key;
        let task_type = &job.task_type;

        // 1. Dedupe check: upsert job frame.
        let frame = JobFrame {
            job_key: job_key.clone(),
            process_instance_id: Uuid::parse_str(&job.process_instance_id).unwrap_or_default(),
            task_type: task_type.clone(),
            worker_id: self.worker_id.clone(),
            status: JobFrameStatus::Active,
            activated_at: chrono::Utc::now(),
            completed_at: None,
            attempts: 1,
        };

        match self.job_frames.upsert(&frame).await {
            Ok(true) => {
                // New job — proceed with execution.
                tracing::debug!(job_key, task_type, "Job activated (new)");
            }
            Ok(false) => {
                // Already seen — check if completed or dead-lettered.
                if let Ok(Some(existing)) = self.job_frames.find_by_job_key(job_key).await {
                    if existing.status == JobFrameStatus::Completed {
                        tracing::info!(
                            job_key,
                            task_type,
                            "Job already completed (dedupe), skipping"
                        );
                        return;
                    }
                    if existing.status == JobFrameStatus::DeadLettered {
                        tracing::info!(job_key, task_type, "Job already dead-lettered, skipping");
                        return;
                    }
                }
                tracing::debug!(job_key, task_type, "Job re-activated (retry)");
            }
            Err(e) => {
                tracing::error!(job_key, error = %e, "Failed to upsert job frame");
                // Continue anyway — execution without frame is better than no execution.
            }
        }

        // 2. Look up task_type → verb_fqn.
        let (_, task_binding) = match self.config.binding_for_task_type(task_type) {
            Some(binding) => binding,
            None => {
                tracing::error!(job_key, task_type, "No task binding found, failing job");
                self.fail_job_rpc(
                    job_key,
                    "UNKNOWN_TASK_TYPE",
                    &format!("No task binding for task_type '{}'", task_type),
                )
                .await;
                let _ = self.job_frames.mark_failed(job_key).await;
                return;
            }
        };

        let verb_fqn = &task_binding.verb_fqn;

        // 3. Build DSL from domain payload.
        let dsl = build_dsl_from_payload(verb_fqn, &job.domain_payload);

        // 4. Execute the verb.
        let entry_id = job.entry_id;
        let runbook_id = job.runbook_id;
        let outcome = self
            .executor
            .execute_v2(&dsl, entry_id, runbook_id, Some(job.session_stack.clone()))
            .await;

        match outcome {
            crate::sequencer::DslExecutionOutcome::Completed(result) => {
                // 5a. Success — complete the job via gRPC.
                let result_json = completion_payload_from_result(result);
                let (canonical, _) = canonical_json_with_hash(&result_json);

                if let Err(e) = self
                    .bpmn_client
                    .complete_job(CompleteJobRequest {
                        job_key: job_key.clone(),
                        domain_payload: canonical,
                        // The engine expects the hash of the current instance
                        // payload snapshot, not the new completion payload.
                        domain_payload_hash: job.domain_payload_hash.clone(),
                        orch_flags: std::collections::HashMap::new(),
                    })
                    .await
                {
                    tracing::error!(job_key, error = %e, "Failed to complete job via gRPC");
                }

                let _ = self.job_frames.mark_completed(job_key).await;
                tracing::info!(job_key, verb_fqn, "Job completed successfully");
            }
            crate::sequencer::DslExecutionOutcome::Failed(err) => {
                // 5b. Failure — fail the job via gRPC.
                self.fail_job_rpc(job_key, "VERB_EXECUTION_ERROR", &err)
                    .await;

                // Check if this job has exceeded the maximum retry count.
                // If so, promote to dead-letter queue instead of just marking failed.
                let max_retries = task_binding.max_retries;
                let attempts = self
                    .job_frames
                    .find_by_job_key(job_key)
                    .await
                    .ok()
                    .flatten()
                    .map(|f| f.attempts)
                    .unwrap_or(1);

                if attempts as u32 >= max_retries {
                    let _ = self.job_frames.mark_dead_lettered(job_key).await;
                    tracing::error!(
                        job_key,
                        verb_fqn,
                        attempts,
                        max_retries,
                        error = %err,
                        "Job exceeded max retries, promoted to dead-letter queue"
                    );
                } else {
                    let _ = self.job_frames.mark_failed(job_key).await;
                    tracing::warn!(
                        job_key,
                        verb_fqn,
                        attempts,
                        max_retries,
                        error = %err,
                        "Job failed (attempt {}/{})",
                        attempts,
                        max_retries
                    );
                }
            }
            crate::sequencer::DslExecutionOutcome::Parked { .. } => {
                // 5c. Parked — should not happen for job verbs (they're direct).
                //     Log a warning but don't complete the job — let it retry.
                tracing::warn!(
                    job_key,
                    verb_fqn,
                    "Job verb returned Parked (unexpected), not completing"
                );
            }
        }
    }

    /// Fail a job via gRPC.
    async fn fail_job_rpc(&self, job_key: &str, error_class: &str, message: &str) {
        if let Err(e) = self
            .bpmn_client
            .fail_job(job_key, error_class, message, 0)
            .await
        {
            tracing::error!(
                job_key,
                error = %e,
                "Failed to fail job via gRPC"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// DSL construction
// ---------------------------------------------------------------------------

/// Build a DSL s-expression from a verb FQN and a JSON domain payload.
///
/// Example: `build_dsl_from_payload("kyc.create-case", r#"{"entity_id":"abc"}"#)`
/// → `(kyc.create-case :entity-id "abc")`
///
/// Falls back to raw JSON embedding if the payload isn't a flat JSON object.
pub fn build_dsl_from_payload(verb_fqn: &str, domain_payload_json: &str) -> String {
    match serde_json::from_str(domain_payload_json) {
        Ok(serde_json::Value::Object(map)) => {
            if map.is_empty() {
                return format!("({})", verb_fqn);
            }
            let mut parts = vec![format!("({}", verb_fqn)];
            for (key, value) in &map {
                // Convert snake_case key to kebab-case for DSL.
                let kebab_key = key.replace('_', "-");
                let value_str = match value {
                    serde_json::Value::String(s) => dsl_string_literal(s),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => "nil".to_string(),
                    other => dsl_string_literal(
                        &serde_json::to_string(other).unwrap_or_else(|_| other.to_string()),
                    ),
                };
                parts.push(format!(":{} {}", kebab_key, value_str));
            }
            parts.push(")".to_string());
            parts.join(" ")
        }
        Ok(serde_json::Value::String(raw)) => {
            let trimmed = raw.trim();
            if trimmed.starts_with('(') && trimmed.ends_with(')') {
                tracing::debug!(
                    verb_fqn,
                    "Using canonical DSL string payload directly for BPMN job execution"
                );
                trimmed.to_string()
            } else {
                tracing::debug!(
                    verb_fqn,
                    "Falling back to raw :payload DSL embedding for string domain payload"
                );
                format!("({} :payload {})", verb_fqn, dsl_string_literal(&raw))
            }
        }
        _ => {
            tracing::debug!(
                verb_fqn,
                "Falling back to raw :payload DSL embedding for non-object domain payload"
            );
            // Non-object payload — embed as raw :payload argument.
            format!(
                "({} :payload {})",
                verb_fqn,
                dsl_string_literal(domain_payload_json)
            )
        }
    }
}

fn dsl_string_literal(raw: &str) -> String {
    format!("\"{}\"", raw.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Build the BPMN completion payload from a verb execution result.
///
/// Object-shaped results are passed through unchanged so correlation keys like
/// `case_id` remain top-level for later workflow steps. Scalar results are
/// wrapped under `"result"` to preserve a uniform JSON object payload.
fn completion_payload_from_result(result: serde_json::Value) -> serde_json::Value {
    match result {
        serde_json::Value::Object(map) => {
            if let Some(unwrapped) = unwrap_singleton_dsl_result(&map) {
                return unwrapped;
            }
            serde_json::Value::Object(map)
        }
        other => serde_json::Value::Object(serde_json::Map::from_iter(std::iter::once((
            "result".to_string(),
            other,
        )))),
    }
}

fn unwrap_singleton_dsl_result(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    let success = map.get("success")?.as_bool()?;
    if !success {
        return None;
    }

    let results = map.get("results")?.as_array()?;
    if results.len() != 1 {
        return None;
    }

    let result_entry = results.first()?.as_object()?;
    if let Some(value) = result_entry.get("value") {
        return Some(value.clone());
    }
    result_entry.get("result").cloned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_dsl_flat_object() {
        let dsl = build_dsl_from_payload(
            "kyc.create-case",
            r#"{"entity_id":"abc-123","case_type":"enhanced"}"#,
        );
        assert!(dsl.starts_with("(kyc.create-case"));
        assert!(dsl.contains(":entity-id \"abc-123\""));
        assert!(dsl.contains(":case-type \"enhanced\""));
        assert!(dsl.ends_with(')'));
    }

    #[test]
    fn test_build_dsl_with_numbers() {
        let dsl = build_dsl_from_payload("entity.update", r#"{"score":42,"active":true}"#);
        assert!(dsl.contains(":score 42"));
        assert!(dsl.contains(":active true"));
    }

    #[test]
    fn test_build_dsl_empty_object() {
        let dsl = build_dsl_from_payload("session.info", "{}");
        assert_eq!(dsl, "(session.info)");
    }

    #[test]
    fn test_build_dsl_non_object_fallback() {
        let dsl = build_dsl_from_payload("kyc.create-case", "\"just a string\"");
        assert!(dsl.starts_with("(kyc.create-case :payload"));
    }

    #[test]
    fn test_build_dsl_from_canonical_dsl_string_payload() {
        let dsl = build_dsl_from_payload(
            "kyc-case.create",
            r#""(kyc-case.create :cbu-id \"abc\" :case-type \"NEW_CLIENT\")""#,
        );
        assert_eq!(
            dsl,
            r#"(kyc-case.create :cbu-id "abc" :case-type "NEW_CLIENT")"#
        );
    }

    #[test]
    fn test_build_dsl_invalid_json_fallback() {
        let dsl = build_dsl_from_payload("kyc.create-case", "not json at all");
        assert!(dsl.starts_with("(kyc.create-case :payload"));
    }

    #[test]
    fn test_build_dsl_escapes_nested_json_values() {
        let dsl = build_dsl_from_payload(
            "document.solicit-batch",
            r#"{"bindings":{},"results":[{"type":"record","value":{"case_id":"abc","status":"INTAKE"}}],"steps_executed":1,"success":true}"#,
        );
        assert!(dsl.contains(r#":bindings "{}""#));
        assert!(dsl.contains(r#":results "[{\"type\":\"record\",\"value\":{\"case_id\":\"abc\",\"status\":\"INTAKE\"}}]""#));
        assert!(dsl.contains(":steps-executed 1"));
        assert!(dsl.contains(":success true"));
    }

    #[test]
    fn test_completion_payload_from_object_result_preserves_top_level_keys() {
        let payload = completion_payload_from_result(serde_json::json!({
            "case_id": "abc",
            "status": "INTAKE"
        }));
        assert_eq!(payload["case_id"], "abc");
        assert_eq!(payload["status"], "INTAKE");
        assert!(payload.get("result").is_none());
    }

    #[test]
    fn test_completion_payload_from_scalar_result_wraps_result_key() {
        let payload = completion_payload_from_result(serde_json::json!("done"));
        assert_eq!(payload["result"], "done");
    }

    #[test]
    fn test_completion_payload_unwraps_singleton_dsl_record_result() {
        let payload = completion_payload_from_result(serde_json::json!({
            "bindings": {},
            "results": [{
                "type": "record",
                "value": {
                    "case_id": "abc",
                    "cbu_id": "def",
                    "status": "INTAKE"
                }
            }],
            "steps_executed": 1,
            "success": true
        }));
        assert_eq!(payload["case_id"], "abc");
        assert_eq!(payload["cbu_id"], "def");
        assert_eq!(payload["status"], "INTAKE");
        assert!(payload.get("results").is_none());
    }
}
