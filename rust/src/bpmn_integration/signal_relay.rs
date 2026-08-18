//! SignalRelay — consumes EventBridge outcome events and relays terminal
//! events (ProcessCompleted, ProcessCancelled, IncidentCreated) to the
//! REPL orchestrator so that parked runbook entries are automatically resumed.
//!
//! ## Why a Separate Component?
//!
//! EventBridge owns the gRPC stream subscription and store updates.
//! Adding an orchestrator reference to EventBridge would create a circular
//! dependency. SignalRelay sits between the `outcome_tx` channel (produced
//! by EventBridge) and the orchestrator, keeping both components decoupled.

use std::sync::Arc;

use tokio::sync::mpsc;
use uuid::Uuid;

use super::correlation::CorrelationStore;
use super::parked_tokens::ParkedTokenStore;
use super::types::OutcomeEvent;
use crate::sequencer::ReplOrchestratorV2;

// ---------------------------------------------------------------------------
// SignalRelay
// ---------------------------------------------------------------------------

/// Consumes `OutcomeEvent`s from an EventBridge channel and relays terminal
/// events to the REPL orchestrator for automatic runbook resume.
pub struct SignalRelay {
    orchestrator: Arc<ReplOrchestratorV2>,
    correlations: CorrelationStore,
    parked_tokens: ParkedTokenStore,
}

impl SignalRelay {
    pub fn new(
        orchestrator: Arc<ReplOrchestratorV2>,
        correlations: CorrelationStore,
        parked_tokens: ParkedTokenStore,
    ) -> Self {
        Self {
            orchestrator,
            correlations,
            parked_tokens,
        }
    }

    /// Run the relay loop, consuming events until the channel closes.
    ///
    /// Typically spawned as a background `tokio::spawn` task per process
    /// instance alongside `EventBridge::subscribe_instance()`.
    pub async fn run(&self, mut outcome_rx: mpsc::Receiver<OutcomeEvent>) {
        while let Some(event) = outcome_rx.recv().await {
            self.handle_event(&event).await;
        }
        tracing::debug!("SignalRelay: outcome channel closed, exiting");
    }

    async fn handle_event(&self, event: &OutcomeEvent) {
        match event {
            OutcomeEvent::ProcessCompleted {
                process_instance_id,
            } => {
                self.relay_signal(*process_instance_id, "completed", None)
                    .await;
            }
            OutcomeEvent::ProcessCancelled {
                process_instance_id,
                reason,
            } => {
                self.relay_signal(*process_instance_id, "failed", Some(reason.clone()))
                    .await;
            }
            OutcomeEvent::IncidentCreated {
                process_instance_id,
                error,
                ..
            } => {
                self.relay_signal(*process_instance_id, "failed", Some(error.clone()))
                    .await;
            }
            // StepCompleted / StepFailed are informational — no relay needed.
            OutcomeEvent::StepCompleted { .. } | OutcomeEvent::StepFailed { .. } => {}
        }
    }

    /// Look up the correlation key for a process instance and signal the
    /// orchestrator.
    async fn relay_signal(&self, process_instance_id: Uuid, status: &str, error: Option<String>) {
        // 1. Look up correlation record.
        //    EventBridge already resolved parked tokens before sending to the
        //    channel, but the CorrelationRecord persists (status updated to
        //    Completed/Cancelled/Failed).
        let correlation = match self
            .correlations
            .find_by_process_instance(process_instance_id)
            .await
        {
            Ok(Some(c)) => c,
            Ok(None) => {
                tracing::warn!(
                    process_instance_id = %process_instance_id,
                    "SignalRelay: no correlation record found"
                );
                return;
            }
            Err(e) => {
                tracing::error!(
                    process_instance_id = %process_instance_id,
                    error = %e,
                    "SignalRelay: correlation lookup failed"
                );
                return;
            }
        };

        // 2. Reconstruct correlation_key using the same format as the
        //    dispatcher: "{runbook_id}:{entry_id}:{correlation_id}".
        let correlation_key = format!(
            "{}:{}:{}",
            correlation.runbook_id, correlation.entry_id, correlation.correlation_id
        );

        // 2b. For a clean completion, thread the typed result through.
        //     EventBridge already wrote it onto the token's result_payload
        //     when the driving JobCompleted event arrived (before this
        //     process's own Completed event, which carries no domain
        //     payload of its own) and has since flipped the token to
        //     'resolved' — read it back by process_instance_id, not
        //     find_by_correlation_key, which is scoped to 'waiting' only.
        let result = if status == "completed" {
            match self
                .parked_tokens
                .find_by_process_instance(process_instance_id)
                .await
            {
                Ok(Some(token)) => token.result_payload,
                Ok(None) => None,
                Err(e) => {
                    tracing::error!(
                        process_instance_id = %process_instance_id,
                        error = %e,
                        "SignalRelay: failed to look up parked token result"
                    );
                    None
                }
            }
        } else {
            None
        };

        // 3. Signal the orchestrator.
        match self
            .orchestrator
            .signal_completion(&correlation_key, status, result, error)
            .await
        {
            Ok(Some(_)) => {
                tracing::info!(
                    process_instance_id = %process_instance_id,
                    correlation_key = correlation_key,
                    status = status,
                    "SignalRelay: relayed signal to orchestrator"
                );
            }
            Ok(None) => {
                tracing::debug!(
                    process_instance_id = %process_instance_id,
                    correlation_key = correlation_key,
                    "SignalRelay: no session found for correlation key (already resumed?)"
                );
            }
            Err(e) => {
                tracing::error!(
                    process_instance_id = %process_instance_id,
                    correlation_key = correlation_key,
                    error = %e,
                    "SignalRelay: failed to relay signal"
                );
            }
        }
    }
}
