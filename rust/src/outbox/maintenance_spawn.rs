//! Maintenance subprocess consumer (Phase 0g Pattern A → Phase 5e cutover).
//!
//! Closes the loop opened in Phase 0g: previously
//! `MaintenanceReindexEmbeddingsOp::execute_json` queued a row to
//! `public.outbox` and returned, leaving the actual subprocess
//! unspawned. This consumer drains those rows post-commit and runs
//! the subprocess.
//!
//! # Idempotency
//!
//! The drainer's claim path already enforces at-least-once. If the
//! subprocess has already run for this idempotency_key (e.g. because
//! the worker crashed mid-spawn and recycled the row), there is no
//! cheap way to detect that without an external state check —
//! `populate_embeddings` is itself idempotent (it re-embeds based on
//! a watermark in the DB), so a re-spawn is safe even if redundant.

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use ob_poc_types::{ClaimedOutboxRow, OutboxEffectKind, OutboxProcessOutcome};
use serde::Deserialize;
use tokio::process::Command;
use tokio::time::timeout;

use super::consumer::AsyncOutboxConsumer;

/// Default subprocess timeout. Embedding rebuilds typically run for
/// tens of seconds to a few minutes; 10 minutes is a generous ceiling
/// before we declare the row failed_retryable.
const DEFAULT_SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Deserialize)]
struct SpawnPayload {
    /// Executable to invoke. Currently always "cargo" — anchored here
    /// so we can extend to other binaries without touching the schema.
    command: String,
    /// CLI arguments passed verbatim.
    args: Vec<String>,
    /// Operator override flag — purely informational here, used by
    /// the queueing op to differentiate the idempotency key.
    #[serde(default)]
    #[allow(dead_code)]
    force: bool,
}

pub struct MaintenanceSpawnConsumer {
    timeout: Duration,
}

impl MaintenanceSpawnConsumer {
    pub fn new() -> Self {
        Self {
            timeout: DEFAULT_SUBPROCESS_TIMEOUT,
        }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for MaintenanceSpawnConsumer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AsyncOutboxConsumer for MaintenanceSpawnConsumer {
    fn effect_kind(&self) -> OutboxEffectKind {
        OutboxEffectKind::MaintenanceSpawn
    }

    fn label(&self) -> &str {
        "maintenance-spawn-v1"
    }

    async fn process(&self, row: ClaimedOutboxRow) -> OutboxProcessOutcome {
        let payload: SpawnPayload = match serde_json::from_value(row.payload) {
            Ok(p) => p,
            Err(e) => {
                return OutboxProcessOutcome::Terminal {
                    reason: format!("malformed maintenance_spawn payload: {e}"),
                };
            }
        };

        tracing::info!(
            id = %row.id,
            command = %payload.command,
            args = ?payload.args,
            "maintenance-spawn-v1: spawning subprocess"
        );

        let spawn_result = Command::new(&payload.command)
            .args(&payload.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        let child = match spawn_result {
            Ok(c) => c,
            Err(e) => {
                // Spawn failure is typically a missing binary or PATH
                // issue — treat as terminal so it shows up in alerts.
                return OutboxProcessOutcome::Terminal {
                    reason: format!("subprocess spawn failed: {e}"),
                };
            }
        };

        let wait = timeout(self.timeout, child.wait_with_output()).await;
        let output = match wait {
            Ok(Ok(out)) => out,
            Ok(Err(e)) => {
                return OutboxProcessOutcome::Retryable {
                    reason: format!("subprocess wait failed: {e}"),
                };
            }
            Err(_) => {
                return OutboxProcessOutcome::Retryable {
                    reason: format!("subprocess timed out after {}s", self.timeout.as_secs()),
                };
            }
        };

        if output.status.success() {
            tracing::info!(
                id = %row.id,
                stdout_bytes = output.stdout.len(),
                "maintenance-spawn-v1: subprocess completed"
            );
            OutboxProcessOutcome::Done
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let truncated: String = stderr.chars().take(2000).collect();
            tracing::warn!(
                id = %row.id,
                exit = ?output.status.code(),
                stderr = %truncated,
                "maintenance-spawn-v1: subprocess failed"
            );
            OutboxProcessOutcome::Retryable {
                reason: format!(
                    "subprocess exited with {:?}: {}",
                    output.status.code(),
                    truncated
                ),
            }
        }
    }
}
