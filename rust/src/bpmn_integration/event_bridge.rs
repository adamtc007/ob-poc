//! EventBridge — subscribes to bpmn-lite lifecycle events and translates them
//! into store updates + REPL signal forwarding.
//!
//! ## Event Mapping
//!
//! | BPMN event_type    | OutcomeEvent variant | Store action                          |
//! |--------------------|----------------------|---------------------------------------|
//! | `JobCompleted`     | `StepCompleted`      | (informational only)                  |
//! | `Completed`        | `ProcessCompleted`   | Correlation → Completed, resolve all  |
//! | `Cancelled`        | `ProcessCancelled`   | Correlation → Cancelled, resolve all  |
//! | `IncidentCreated`  | `IncidentCreated`    | Correlation → Failed                  |
//! | Other              | (ignored)            | —                                     |

use anyhow::{Context, Result};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::client::{lifecycle_event_from_proto, BpmnLifecycleEvent, BpmnLiteConnection};
use super::correlation::CorrelationStore;
use super::parked_tokens::ParkedTokenStore;
use super::types::{CorrelationStatus, OutcomeEvent};

// ---------------------------------------------------------------------------
// EventBridge
// ---------------------------------------------------------------------------

/// Subscribes to lifecycle events for a process instance and translates them
/// into store updates + outcome events.
pub struct EventBridge {
    bpmn_client: BpmnLiteConnection,
    correlations: CorrelationStore,
    parked_tokens: ParkedTokenStore,
}

impl EventBridge {
    pub fn new(
        bpmn_client: BpmnLiteConnection,
        correlations: CorrelationStore,
        parked_tokens: ParkedTokenStore,
    ) -> Self {
        Self {
            bpmn_client,
            correlations,
            parked_tokens,
        }
    }

    /// Subscribe to events for a process instance and forward outcomes.
    ///
    /// Runs until a terminal event is received or the channel is closed.
    /// If the gRPC stream drops before a terminal event, the bridge
    /// reconnects automatically with exponential backoff, filtering out
    /// already-seen events by sequence number.
    pub async fn subscribe_instance(
        &self,
        process_instance_id: Uuid,
        outcome_tx: mpsc::Sender<OutcomeEvent>,
    ) -> Result<()> {
        /// Maximum number of reconnect attempts before giving up.
        const MAX_RECONNECT_ATTEMPTS: u32 = 10;
        /// Initial backoff duration (doubles on each retry).
        const INITIAL_BACKOFF_MS: u64 = 250;
        /// Maximum backoff cap.
        const MAX_BACKOFF_MS: u64 = 30_000;

        let mut last_seen_seq: u64 = 0;
        let mut reconnect_attempts: u32 = 0;
        let mut saw_terminal = false;

        loop {
            match self
                .run_event_stream(
                    process_instance_id,
                    &outcome_tx,
                    &mut last_seen_seq,
                    &mut saw_terminal,
                )
                .await
            {
                Ok(()) => {
                    // Stream ended cleanly.
                    if saw_terminal {
                        tracing::info!(
                            process_instance_id = %process_instance_id,
                            last_seen_seq,
                            "EventBridge stream ended after terminal event"
                        );
                        return Ok(());
                    }

                    // Stream ended without terminal event — channel closed
                    // or receiver dropped.
                    if outcome_tx.is_closed() {
                        tracing::warn!(
                            process_instance_id = %process_instance_id,
                            "Outcome channel closed, stopping event bridge"
                        );
                        return Ok(());
                    }

                    // Stream ended prematurely — reconnect.
                    reconnect_attempts += 1;
                    if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
                        tracing::error!(
                            process_instance_id = %process_instance_id,
                            reconnect_attempts,
                            "EventBridge exceeded max reconnect attempts"
                        );
                        return Err(anyhow::anyhow!(
                            "EventBridge exceeded {} reconnect attempts",
                            MAX_RECONNECT_ATTEMPTS
                        ));
                    }

                    let backoff = std::cmp::min(
                        INITIAL_BACKOFF_MS * 2u64.pow(reconnect_attempts - 1),
                        MAX_BACKOFF_MS,
                    );
                    tracing::warn!(
                        process_instance_id = %process_instance_id,
                        reconnect_attempts,
                        last_seen_seq,
                        backoff_ms = backoff,
                        "EventBridge stream ended prematurely, reconnecting"
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                }
                Err(e) => {
                    // Stream error — reconnect with backoff.
                    reconnect_attempts += 1;
                    if reconnect_attempts > MAX_RECONNECT_ATTEMPTS {
                        tracing::error!(
                            process_instance_id = %process_instance_id,
                            reconnect_attempts,
                            error = %e,
                            "EventBridge exceeded max reconnect attempts after error"
                        );
                        return Err(e);
                    }

                    let backoff = std::cmp::min(
                        INITIAL_BACKOFF_MS * 2u64.pow(reconnect_attempts - 1),
                        MAX_BACKOFF_MS,
                    );
                    tracing::warn!(
                        process_instance_id = %process_instance_id,
                        reconnect_attempts,
                        last_seen_seq,
                        backoff_ms = backoff,
                        error = %e,
                        "EventBridge stream error, reconnecting"
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff)).await;
                }
            }
        }
    }

    /// Run a single event stream session, updating `last_seen_seq` as events
    /// are received. Returns `Ok(())` when the stream ends (either cleanly or
    /// prematurely). Sets `saw_terminal` to `true` if a terminal event arrives.
    async fn run_event_stream(
        &self,
        process_instance_id: Uuid,
        outcome_tx: &mpsc::Sender<OutcomeEvent>,
        last_seen_seq: &mut u64,
        saw_terminal: &mut bool,
    ) -> Result<()> {
        let mut stream = self
            .bpmn_client
            .subscribe_events(process_instance_id)
            .await
            .context("Failed to subscribe to events")?;

        tracing::info!(
            process_instance_id = %process_instance_id,
            from_seq = *last_seen_seq,
            "EventBridge subscribed to lifecycle events"
        );

        while let Some(proto_event) = stream.message().await.context("Event stream error")? {
            let event = lifecycle_event_from_proto(proto_event);

            // Skip events we've already seen (from previous stream session).
            if event.sequence < *last_seen_seq {
                continue;
            }

            // Track the highest sequence seen.
            if event.sequence >= *last_seen_seq {
                *last_seen_seq = event.sequence + 1;
            }

            if let Some(outcome) = Self::translate_event(&event) {
                // Check if this is a terminal event.
                if matches!(
                    outcome,
                    OutcomeEvent::ProcessCompleted { .. }
                        | OutcomeEvent::ProcessCancelled { .. }
                        | OutcomeEvent::IncidentCreated { .. }
                ) {
                    *saw_terminal = true;
                }

                // Update stores based on the event.
                self.handle_outcome(process_instance_id, &outcome).await;

                // Forward to REPL signal handler.
                if outcome_tx.send(outcome).await.is_err() {
                    tracing::warn!(
                        process_instance_id = %process_instance_id,
                        "Outcome channel closed, stopping event bridge"
                    );
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Translate a BPMN lifecycle event to an OutcomeEvent.
    ///
    /// Returns `None` for unrecognized or informational event types.
    pub fn translate_event(event: &BpmnLifecycleEvent) -> Option<OutcomeEvent> {
        let process_instance_id = Uuid::parse_str(&event.process_instance_id).unwrap_or_default();

        // Try to extract structured fields from payload_json.
        let payload: serde_json::Value =
            serde_json::from_str(&event.payload_json).unwrap_or_default();

        match event.event_type.as_str() {
            "JobCompleted" => Some(OutcomeEvent::StepCompleted {
                process_instance_id,
                job_key: payload
                    .get("job_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                task_type: payload
                    .get("task_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                result: payload.get("result").cloned().unwrap_or_default(),
            }),
            "JobFailed" => Some(OutcomeEvent::StepFailed {
                process_instance_id,
                job_key: payload
                    .get("job_key")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                task_type: payload
                    .get("task_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                error: payload
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&event.payload_json)
                    .to_string(),
            }),
            "Completed" => Some(OutcomeEvent::ProcessCompleted {
                process_instance_id,
            }),
            "Cancelled" => Some(OutcomeEvent::ProcessCancelled {
                process_instance_id,
                reason: payload
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("cancelled")
                    .to_string(),
            }),
            "IncidentCreated" => Some(OutcomeEvent::IncidentCreated {
                process_instance_id,
                service_task_id: payload
                    .get("service_task_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                error: payload
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&event.payload_json)
                    .to_string(),
            }),
            _ => {
                tracing::trace!(
                    event_type = event.event_type,
                    "Ignoring unrecognized event type"
                );
                None
            }
        }
    }

    /// Handle store updates triggered by an outcome event.
    async fn handle_outcome(&self, process_instance_id: Uuid, outcome: &OutcomeEvent) {
        match outcome {
            OutcomeEvent::ProcessCompleted { .. } => {
                // Correlation → Completed, resolve all parked tokens.
                self.update_correlation(process_instance_id, CorrelationStatus::Completed)
                    .await;
                self.resolve_all_tokens(process_instance_id).await;
            }
            OutcomeEvent::ProcessCancelled { .. } => {
                // Correlation → Cancelled, resolve all parked tokens.
                self.update_correlation(process_instance_id, CorrelationStatus::Cancelled)
                    .await;
                self.resolve_all_tokens(process_instance_id).await;
            }
            OutcomeEvent::IncidentCreated { .. } => {
                // Correlation → Failed (tokens remain waiting for manual resolution).
                self.update_correlation(process_instance_id, CorrelationStatus::Failed)
                    .await;
            }
            OutcomeEvent::StepCompleted { .. } | OutcomeEvent::StepFailed { .. } => {
                // Informational — no store updates needed.
            }
        }
    }

    /// Update the correlation status for a process instance.
    async fn update_correlation(&self, process_instance_id: Uuid, new_status: CorrelationStatus) {
        match self
            .correlations
            .find_by_process_instance(process_instance_id)
            .await
        {
            Ok(Some(record)) => {
                if let Err(e) = self
                    .correlations
                    .update_status(record.correlation_id, new_status)
                    .await
                {
                    tracing::error!(
                        process_instance_id = %process_instance_id,
                        error = %e,
                        "Failed to update correlation status"
                    );
                }
            }
            Ok(None) => {
                tracing::warn!(
                    process_instance_id = %process_instance_id,
                    "No correlation found for process instance"
                );
            }
            Err(e) => {
                tracing::error!(
                    process_instance_id = %process_instance_id,
                    error = %e,
                    "Failed to look up correlation"
                );
            }
        }
    }

    /// Resolve all parked tokens for a process instance.
    async fn resolve_all_tokens(&self, process_instance_id: Uuid) {
        match self
            .parked_tokens
            .resolve_all_for_instance(process_instance_id)
            .await
        {
            Ok(count) => {
                tracing::info!(
                    process_instance_id = %process_instance_id,
                    resolved_count = count,
                    "Resolved parked tokens"
                );
            }
            Err(e) => {
                tracing::error!(
                    process_instance_id = %process_instance_id,
                    error = %e,
                    "Failed to resolve parked tokens"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(event_type: &str) -> BpmnLifecycleEvent {
        BpmnLifecycleEvent {
            sequence: 1,
            event_type: event_type.to_string(),
            process_instance_id: Uuid::new_v4().to_string(),
            payload_json: "{}".to_string(),
        }
    }

    #[test]
    fn test_translate_job_completed() {
        let event = make_event("JobCompleted");
        let outcome = EventBridge::translate_event(&event);
        assert!(matches!(outcome, Some(OutcomeEvent::StepCompleted { .. })));
    }

    #[test]
    fn test_translate_job_failed() {
        let event = make_event("JobFailed");
        let outcome = EventBridge::translate_event(&event);
        assert!(matches!(outcome, Some(OutcomeEvent::StepFailed { .. })));
    }

    #[test]
    fn test_translate_completed() {
        let event = make_event("Completed");
        let outcome = EventBridge::translate_event(&event);
        assert!(matches!(
            outcome,
            Some(OutcomeEvent::ProcessCompleted { .. })
        ));
    }

    #[test]
    fn test_translate_cancelled() {
        let event = make_event("Cancelled");
        let outcome = EventBridge::translate_event(&event);
        assert!(matches!(
            outcome,
            Some(OutcomeEvent::ProcessCancelled { .. })
        ));
    }

    #[test]
    fn test_translate_incident_created() {
        let event = make_event("IncidentCreated");
        let outcome = EventBridge::translate_event(&event);
        assert!(matches!(
            outcome,
            Some(OutcomeEvent::IncidentCreated { .. })
        ));
    }

    #[test]
    fn test_translate_unknown_returns_none() {
        let event = make_event("SomethingElse");
        let outcome = EventBridge::translate_event(&event);
        assert!(outcome.is_none());
    }
}
