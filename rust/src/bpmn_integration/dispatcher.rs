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
use super::pending_dispatches::PendingDispatchStore;
use super::types::{
    CorrelationRecord, CorrelationStatus, ExecutionRoute, ParkedToken, ParkedTokenStatus,
    PendingDispatch, PendingDispatchStatus,
};
use crate::repl::orchestrator_v2::{DslExecutionOutcome, DslExecutorV2};

#[cfg(feature = "database")]
use sqlx::PgPool;

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
    /// Pending dispatch queue for BPMN resilience.
    pending_dispatches: PendingDispatchStore,
    /// Database pool for dual-write bridge (outstanding_requests).
    #[cfg(feature = "database")]
    pool: Option<PgPool>,
}

impl WorkflowDispatcher {
    pub fn new(
        inner: Arc<dyn DslExecutorV2>,
        config: Arc<WorkflowConfigIndex>,
        bpmn_client: BpmnLiteConnection,
        correlations: CorrelationStore,
        parked_tokens: ParkedTokenStore,
        pending_dispatches: PendingDispatchStore,
    ) -> Self {
        Self {
            inner,
            config,
            bpmn_client,
            correlations,
            parked_tokens,
            pending_dispatches,
            #[cfg(feature = "database")]
            pool: None,
        }
    }

    /// Set the database pool for dual-write bridge support.
    #[cfg(feature = "database")]
    pub fn with_pool(mut self, pool: PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    /// Dual-write bridge: create a legacy `outstanding_requests` row alongside
    /// the BPMN process start. This keeps the legacy request-tracking system in
    /// sync during the transition to fully BPMN-orchestrated workflows.
    ///
    /// Best-effort — failures are logged but never block the main dispatch.
    #[cfg(feature = "database")]
    async fn try_dual_write_outstanding_request(
        &self,
        verb_fqn: &str,
        dsl: &str,
        process_instance_id: Uuid,
        domain_correlation_key: Option<&str>,
    ) {
        let Some(pool) = &self.pool else {
            return; // No pool configured — dual-write disabled
        };

        // Extract subject_id from the DSL args.  For document verbs this is
        // `subject-entity-id`; for KYC verbs it is `entity-id` or the case_id.
        let subject_id = Self::extract_arg_value(dsl, "subject-entity-id")
            .or_else(|| Self::extract_arg_value(dsl, "entity-id"))
            .and_then(|s| Uuid::parse_str(&s).ok());

        let case_id = domain_correlation_key.and_then(|k| Uuid::parse_str(k).ok());

        // Derive request_type from the verb domain (e.g., "document" → "DOCUMENT")
        let request_type = verb_fqn
            .split('.')
            .next()
            .unwrap_or("WORKFLOW")
            .to_uppercase();

        let request_subtype = verb_fqn.split('.').nth(1).unwrap_or("unknown").to_string();

        let result = sqlx::query!(
            r#"
            INSERT INTO kyc.outstanding_requests (
                subject_type, subject_id,
                case_id,
                request_type, request_subtype,
                request_details,
                requested_by_agent,
                created_by_verb
            ) VALUES (
                COALESCE($1, 'ENTITY'), $2,
                $3,
                $4, $5,
                $6,
                true,
                $7
            )
            RETURNING request_id
            "#,
            subject_id.map(|_| "ENTITY".to_string()),
            subject_id,
            case_id,
            request_type,
            request_subtype,
            serde_json::json!({
                "bpmn_process_instance_id": process_instance_id,
                "source": "dual_write_bridge",
                "verb": verb_fqn,
            }),
            verb_fqn,
        )
        .fetch_one(pool)
        .await;

        match result {
            Ok(row) => {
                tracing::info!(
                    verb = verb_fqn,
                    request_id = %row.request_id,
                    process_instance_id = %process_instance_id,
                    "Dual-write bridge: created outstanding_requests row"
                );
            }
            Err(e) => {
                tracing::warn!(
                    verb = verb_fqn,
                    process_instance_id = %process_instance_id,
                    error = %e,
                    "Dual-write bridge: failed to create outstanding_requests row (non-blocking)"
                );
            }
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

    /// Extract a named argument value from a DSL s-expression.
    ///
    /// Looks for `:field-name "value"` or `:field-name value` patterns.
    /// The field name in DSL uses kebab-case (`:case-id`) while the
    /// `correlation_field` from YAML uses snake_case (`case_id`).
    fn extract_arg_value(dsl: &str, field_name: &str) -> Option<String> {
        // Try both kebab-case and snake_case keyword forms
        let kebab = field_name.replace('_', "-");
        let snake = field_name.replace('-', "_");
        let keywords = [format!(":{}", kebab), format!(":{}", snake)];

        for keyword in &keywords {
            if let Some(pos) = dsl.find(keyword.as_str()) {
                let after_key = &dsl[pos + keyword.len()..];
                let trimmed = after_key.trim_start();
                if let Some(inner) = trimmed.strip_prefix('"') {
                    // Quoted value: extract until closing quote
                    if let Some(end) = inner.find('"') {
                        return Some(inner[..end].to_string());
                    }
                } else {
                    // Unquoted value: take until whitespace or closing paren
                    let value: String = trimmed
                        .chars()
                        .take_while(|c| !c.is_whitespace() && *c != ')')
                        .collect();
                    if !value.is_empty() && !value.starts_with(':') {
                        return Some(value);
                    }
                }
            }
        }
        None
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

        // 2. Look up pre-compiled bytecode version from the config registry.
        let bytecode_version = self
            .config
            .bytecode_for_process(&process_key)
            .map(|b| b.to_vec())
            .unwrap_or_default();

        let correlation_id = Uuid::new_v4();
        let correlation_key = format!("{}:{}:{}", runbook_id, entry_id, correlation_id);

        // 3. Extract domain_correlation_key from DSL args using the binding's
        //    correlation_field (e.g., "case_id" → `:case-id "uuid..."` → "uuid...").
        let domain_correlation_key = binding
            .correlation_field
            .as_deref()
            .and_then(|field| Self::extract_arg_value(dsl, field));

        if domain_correlation_key.is_some() {
            tracing::debug!(
                verb = verb_fqn,
                field = binding.correlation_field.as_deref().unwrap_or(""),
                key = domain_correlation_key.as_deref().unwrap_or(""),
                "Extracted domain correlation key from DSL args"
            );
        }

        // 4. Start BPMN process via gRPC.
        //    On failure, queue for retry instead of returning Failed.
        let dispatch_id = Uuid::new_v4();
        let (process_instance_id, queued) = match self
            .bpmn_client
            .start_process(StartProcessRequest {
                process_key: process_key.clone(),
                bytecode_version: bytecode_version.clone(),
                domain_payload: canonical_json.clone(),
                domain_payload_hash: hash.clone(),
                orch_flags: std::collections::HashMap::new(),
                correlation_id,
            })
            .await
        {
            Ok(id) => (id, false),
            Err(e) => {
                tracing::warn!(
                    verb = verb_fqn,
                    process_key = %process_key,
                    error = %e,
                    "BPMN gRPC unavailable — queueing dispatch for retry"
                );

                // Queue the dispatch for background retry.
                let pending = PendingDispatch {
                    dispatch_id,
                    payload_hash: hash.clone(),
                    verb_fqn: verb_fqn.to_string(),
                    process_key: process_key.clone(),
                    bytecode_version,
                    domain_payload: canonical_json.clone(),
                    dsl_source: dsl.to_string(),
                    entry_id,
                    runbook_id,
                    correlation_id,
                    correlation_key: correlation_key.clone(),
                    domain_correlation_key: domain_correlation_key.clone(),
                    status: PendingDispatchStatus::Pending,
                    attempts: 0,
                    last_error: Some(e.to_string()),
                    created_at: chrono::Utc::now(),
                    last_attempted_at: None,
                    dispatched_at: None,
                };

                if let Err(queue_err) = self.pending_dispatches.insert(&pending).await {
                    // If we can't even queue, THEN fail hard — infrastructure is down.
                    return DslExecutionOutcome::Failed(format!(
                        "BPMN service unavailable AND failed to queue dispatch: {}",
                        queue_err
                    ));
                }

                // Use dispatch_id as placeholder — PendingDispatchWorker will
                // patch with the real process_instance_id after successful dispatch.
                (dispatch_id, true)
            }
        };

        // 5. Record correlation: session ↔ process instance.
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
            domain_correlation_key: domain_correlation_key.clone(),
        };

        if let Err(e) = self.correlations.insert(&record).await {
            tracing::error!(
                "Failed to record correlation for process {}: {}",
                process_instance_id,
                e
            );
        }

        // 6. Park a token for the REPL entry.
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

        // 7. Dual-write bridge: create legacy outstanding_requests row.
        #[cfg(feature = "database")]
        self.try_dual_write_outstanding_request(
            verb_fqn,
            dsl,
            process_instance_id,
            domain_correlation_key.as_deref(),
        )
        .await;

        let message = if queued {
            format!(
                "Verb '{}' queued for BPMN workflow '{}' (BPMN service temporarily unavailable, dispatch {})",
                verb_fqn, process_key, dispatch_id
            )
        } else {
            format!(
                "Verb '{}' dispatched to BPMN workflow '{}' (instance: {})",
                verb_fqn, process_key, process_instance_id
            )
        };

        tracing::info!(
            verb = verb_fqn,
            process_key = %process_key,
            process_instance_id = %process_instance_id,
            correlation_key = %correlation_key,
            queued = queued,
            "Orchestrated verb {}",
            if queued { "queued" } else { "dispatched" }
        );

        DslExecutionOutcome::Parked {
            task_id: process_instance_id,
            correlation_key,
            timeout: None,
            message,
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

    // ── extract_arg_value tests ──────────────────────────────────────────

    #[test]
    fn test_extract_arg_value_quoted() {
        let dsl = r#"(kyc.open-case :case-id "abc-123" :entity-id "def-456")"#;
        assert_eq!(
            WorkflowDispatcher::extract_arg_value(dsl, "case_id"),
            Some("abc-123".to_string())
        );
        assert_eq!(
            WorkflowDispatcher::extract_arg_value(dsl, "entity_id"),
            Some("def-456".to_string())
        );
    }

    #[test]
    fn test_extract_arg_value_unquoted() {
        let dsl = "(kyc.open-case :case-id abc-123 :status open)";
        assert_eq!(
            WorkflowDispatcher::extract_arg_value(dsl, "case_id"),
            Some("abc-123".to_string())
        );
    }

    #[test]
    fn test_extract_arg_value_snake_case_field() {
        // Field name in snake_case, DSL keyword in kebab-case
        let dsl = r#"(document.solicit :subject-entity-id "uuid-here")"#;
        assert_eq!(
            WorkflowDispatcher::extract_arg_value(dsl, "subject_entity_id"),
            Some("uuid-here".to_string())
        );
    }

    #[test]
    fn test_extract_arg_value_kebab_case_field() {
        // Field name already in kebab-case
        let dsl = r#"(document.solicit :case-id "uuid-here")"#;
        assert_eq!(
            WorkflowDispatcher::extract_arg_value(dsl, "case-id"),
            Some("uuid-here".to_string())
        );
    }

    #[test]
    fn test_extract_arg_value_missing() {
        let dsl = r#"(kyc.open-case :entity-id "abc")"#;
        assert_eq!(WorkflowDispatcher::extract_arg_value(dsl, "case_id"), None);
    }

    #[test]
    fn test_extract_arg_value_at_end() {
        let dsl = r#"(kyc.open-case :case-id "final-value")"#;
        assert_eq!(
            WorkflowDispatcher::extract_arg_value(dsl, "case_id"),
            Some("final-value".to_string())
        );
    }

    // ── correlation_field in WorkflowBinding tests ───────────────────────

    #[test]
    fn test_register_from_durable_stores_correlation_field() {
        use dsl_core::config::types::{DurableConfig, DurableRuntime};
        use std::collections::BTreeMap;

        let mut index = WorkflowConfigIndex::from_config(&super::super::config::WorkflowConfig {
            workflows: vec![],
        });

        let durable = DurableConfig {
            runtime: DurableRuntime::BpmnLite,
            process_key: "test-process".to_string(),
            correlation_field: "entity_id".to_string(),
            task_bindings: BTreeMap::new(),
            timeout: None,
            escalation: None,
        };

        index.register_from_durable_config("test.verb", &durable);

        let binding = index.binding_for_verb("test.verb").unwrap();
        assert_eq!(binding.correlation_field, Some("entity_id".to_string()));
    }
}
