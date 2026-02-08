//! WorkflowDispatcher — routes verb execution through Direct or Orchestrated paths.
//!
//! Implements `DslExecutorV2` so it can be used as a drop-in replacement for the
//! standard executor in the V2 REPL pipeline. Orchestrated verbs are dispatched
//! through the bpmn-lite gRPC service; direct verbs delegate to the inner executor.

use std::sync::Arc;
use uuid::Uuid;

use super::canonical::canonical_json_with_hash;
use super::client::{BpmnLiteConnection, StartProcessRequest};
use super::config::WorkflowConfigIndex;
use super::correlation::CorrelationStore;
use super::parked_tokens::ParkedTokenStore;
use super::types::{
    CorrelationRecord, CorrelationStatus, ExecutionRoute, ParkedToken, ParkedTokenStatus,
};
use crate::repl::orchestrator_v2::{DslExecutionOutcome, DslExecutorV2};

// ---------------------------------------------------------------------------
// WorkflowDispatcher
// ---------------------------------------------------------------------------

/// Routes verb execution through Direct or Orchestrated paths.
///
/// - **Direct**: Delegates to the inner `DslExecutorV2` (e.g., `RealDslExecutor`).
/// - **Orchestrated**: Canonicalizes the payload, starts a BPMN process instance
///   via gRPC, records the correlation, parks a token, and returns `Parked`.
///
/// The orchestrator already handles `DslExecutionOutcome::Parked` — it calls
/// `runbook.park_entry()` and sets `RunbookStatus::Parked`. No orchestrator
/// changes are needed.
pub struct WorkflowDispatcher {
    /// Inner executor for direct verb execution.
    inner: Arc<dyn DslExecutorV2>,
    /// Workflow routing configuration.
    config: Arc<WorkflowConfigIndex>,
    /// gRPC connection to bpmn-lite.
    bpmn_client: BpmnLiteConnection,
    /// Correlation store (session ↔ process instance links).
    correlations: CorrelationStore,
    /// Parked token store (waiting REPL entries).
    parked_tokens: ParkedTokenStore,
}

impl WorkflowDispatcher {
    pub fn new(
        inner: Arc<dyn DslExecutorV2>,
        config: Arc<WorkflowConfigIndex>,
        bpmn_client: BpmnLiteConnection,
        correlations: CorrelationStore,
        parked_tokens: ParkedTokenStore,
    ) -> Self {
        Self {
            inner,
            config,
            bpmn_client,
            correlations,
            parked_tokens,
        }
    }

    /// Extract the verb FQN from a DSL string.
    ///
    /// Expects s-expression format: `(domain.verb :arg1 val1 ...)`
    /// Returns `None` if the DSL doesn't match the expected pattern.
    fn extract_verb_fqn(dsl: &str) -> Option<String> {
        let trimmed = dsl.trim();
        let inner = trimmed.strip_prefix('(')?.trim_start();
        // Take chars until whitespace or closing paren
        let verb: String = inner
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != ')')
            .collect();
        if verb.is_empty() {
            None
        } else {
            Some(verb)
        }
    }

    /// Execute an orchestrated verb: canonical hash → gRPC StartProcess → store → Parked.
    async fn execute_orchestrated(
        &self,
        dsl: &str,
        entry_id: Uuid,
        runbook_id: Uuid,
        verb_fqn: &str,
    ) -> DslExecutionOutcome {
        let binding = match self.config.binding_for_verb(verb_fqn) {
            Some(b) => b,
            None => {
                return DslExecutionOutcome::Failed(format!(
                    "No workflow binding for orchestrated verb '{}'",
                    verb_fqn
                ))
            }
        };

        let process_key = match &binding.process_key {
            Some(pk) => pk.clone(),
            None => {
                return DslExecutionOutcome::Failed(format!(
                    "Orchestrated verb '{}' has no process_key configured",
                    verb_fqn
                ))
            }
        };

        // 1. Canonical JSON + hash of the DSL payload.
        //    For now, wrap the raw DSL as a JSON string value.
        let payload_value = serde_json::Value::String(dsl.to_string());
        let (canonical_json, hash) = canonical_json_with_hash(&payload_value);

        // 2. We need bytecode — for now, use an empty placeholder.
        //    In production, the bytecode would be pre-compiled and cached.
        //    The bpmn-lite service looks up bytecode by process_key.
        let bytecode_version = Vec::new();

        let correlation_id = Uuid::new_v4();
        let correlation_key = format!("{}:{}:{}", runbook_id, entry_id, correlation_id);

        // 3. Start BPMN process via gRPC.
        let process_instance_id = match self
            .bpmn_client
            .start_process(StartProcessRequest {
                process_key: process_key.clone(),
                bytecode_version,
                domain_payload: canonical_json,
                domain_payload_hash: hash.clone(),
                orch_flags: std::collections::HashMap::new(),
                correlation_id,
            })
            .await
        {
            Ok(id) => id,
            Err(e) => {
                return DslExecutionOutcome::Failed(format!(
                    "Failed to start BPMN process '{}': {}",
                    process_key, e
                ))
            }
        };

        // 4. Record correlation: session ↔ process instance.
        //    Use Uuid::nil() for session_id since we don't have it at this layer.
        //    The orchestrator knows the session — the correlation_key encodes runbook:entry.
        let record = CorrelationRecord {
            correlation_id,
            process_instance_id,
            session_id: Uuid::nil(), // Resolved at server wiring layer
            runbook_id,
            entry_id,
            process_key: process_key.clone(),
            domain_payload_hash: hash,
            status: CorrelationStatus::Active,
            created_at: chrono::Utc::now(),
            completed_at: None,
            domain_correlation_key: None, // TODO: extract from DurableConfig.correlation_field when available
        };

        if let Err(e) = self.correlations.insert(&record).await {
            tracing::error!(
                "Failed to record correlation for process {}: {}",
                process_instance_id,
                e
            );
            // Don't fail the dispatch — the process is already started.
            // Correlation can be recovered via process inspection.
        }

        // 5. Park a token for the REPL entry.
        let token = ParkedToken {
            token_id: Uuid::new_v4(),
            correlation_key: correlation_key.clone(),
            session_id: Uuid::nil(), // Resolved at server wiring layer
            entry_id,
            process_instance_id,
            expected_signal: format!("process_completed:{}", process_key),
            status: ParkedTokenStatus::Waiting,
            created_at: chrono::Utc::now(),
            resolved_at: None,
            result_payload: None,
        };

        if let Err(e) = self.parked_tokens.insert(&token).await {
            tracing::error!("Failed to park token for entry {}: {}", entry_id, e);
        }

        tracing::info!(
            verb = verb_fqn,
            process_key = process_key,
            process_instance_id = %process_instance_id,
            correlation_key = correlation_key,
            "Orchestrated verb dispatched to BPMN-Lite"
        );

        DslExecutionOutcome::Parked {
            task_id: process_instance_id,
            correlation_key,
            timeout: None,
            message: format!(
                "Verb '{}' dispatched to BPMN workflow '{}' (instance: {})",
                verb_fqn, process_key, process_instance_id
            ),
        }
    }
}

#[async_trait::async_trait]
impl DslExecutorV2 for WorkflowDispatcher {
    async fn execute_v2(&self, dsl: &str, entry_id: Uuid, runbook_id: Uuid) -> DslExecutionOutcome {
        // 1. Extract verb FQN from DSL.
        let verb_fqn = match Self::extract_verb_fqn(dsl) {
            Some(fqn) => fqn,
            None => {
                // Can't parse verb — delegate to inner executor as-is.
                return self.inner.execute_v2(dsl, entry_id, runbook_id).await;
            }
        };

        // 2. Check route.
        match self.config.route_for_verb(&verb_fqn) {
            ExecutionRoute::Direct => {
                // Direct path — delegate to inner executor.
                self.inner.execute_v2(dsl, entry_id, runbook_id).await
            }
            ExecutionRoute::Orchestrated => {
                // Orchestrated path — dispatch through bpmn-lite.
                self.execute_orchestrated(dsl, entry_id, runbook_id, &verb_fqn)
                    .await
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

    #[test]
    fn test_extract_verb_fqn_simple() {
        assert_eq!(
            WorkflowDispatcher::extract_verb_fqn("(kyc.open-case :entity-id \"abc\")"),
            Some("kyc.open-case".to_string())
        );
    }

    #[test]
    fn test_extract_verb_fqn_no_args() {
        assert_eq!(
            WorkflowDispatcher::extract_verb_fqn("(session.info)"),
            Some("session.info".to_string())
        );
    }

    #[test]
    fn test_extract_verb_fqn_with_whitespace() {
        assert_eq!(
            WorkflowDispatcher::extract_verb_fqn("  ( cbu.create :name \"test\" )  "),
            Some("cbu.create".to_string())
        );
    }

    #[test]
    fn test_extract_verb_fqn_empty() {
        assert_eq!(WorkflowDispatcher::extract_verb_fqn(""), None);
        assert_eq!(WorkflowDispatcher::extract_verb_fqn("()"), None);
    }

    #[test]
    fn test_extract_verb_fqn_no_parens() {
        // Not s-expression format — returns None
        assert_eq!(WorkflowDispatcher::extract_verb_fqn("plain text"), None);
    }
}
