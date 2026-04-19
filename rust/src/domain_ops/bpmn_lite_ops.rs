//! BPMN-Lite control operations — direct gRPC verbs for workflow management.
//!
//! These ops expose bpmn-lite gRPC service functionality as DSL verbs:
//! compile, start, signal, cancel, inspect.

use anyhow::Result;
use async_trait::async_trait;
use dsl_runtime_macros::register_custom_op;
use ob_poc_types::session_stack::SessionStackState;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::helpers::{json_extract_string, json_extract_string_opt, json_get_required_uuid};
use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// Result Types
// =============================================================================

/// Result of compiling a BPMN model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmnCompileResult {
    pub bytecode_version_hex: String,
    pub diagnostic_count: usize,
    pub diagnostics: Vec<BpmnDiagnosticResult>,
}

/// Single diagnostic from BPMN compilation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmnDiagnosticResult {
    pub severity: String,
    pub message: String,
    pub element_id: String,
}

/// Result of inspecting a BPMN process instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmnInspectResult {
    pub state: String,
    pub fiber_count: usize,
    pub wait_count: usize,
    pub bytecode_version_hex: String,
    pub domain_payload_hash: String,
}

// =============================================================================
// Helpers
// =============================================================================

fn get_required_string(verb_call: &VerbCall, key: &str) -> Result<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("Missing required argument :{}", key))
}

fn get_optional_string(verb_call: &VerbCall, key: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == key)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

fn get_required_uuid(verb_call: &VerbCall, key: &str) -> Result<Uuid> {
    let s = get_required_string(verb_call, key)?;
    Uuid::parse_str(&s).map_err(|e| anyhow::anyhow!("Invalid UUID for :{}: {}", key, e))
}

/// Get BpmnLiteConnection from env. Returns error if BPMN_LITE_GRPC_URL not set.
#[cfg(feature = "database")]
fn get_bpmn_client() -> Result<crate::bpmn_integration::client::BpmnLiteConnection> {
    crate::bpmn_integration::client::BpmnLiteConnection::from_env()
}

// =============================================================================
// bpmn.compile
// =============================================================================

#[register_custom_op]
pub struct BpmnCompileOp;

#[async_trait]
impl CustomOperation for BpmnCompileOp {
    fn domain(&self) -> &'static str {
        "bpmn"
    }
    fn verb(&self) -> &'static str {
        "compile"
    }
    fn rationale(&self) -> &'static str {
        "Calls bpmn-lite gRPC Compile RPC"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let bpmn_xml = get_required_string(verb_call, "bpmn-xml")?;
        let client = get_bpmn_client()?;
        let result = client.compile(&bpmn_xml).await?;

        let typed = BpmnCompileResult {
            bytecode_version_hex: hex::encode(&result.bytecode_version),
            diagnostic_count: result.diagnostics.len(),
            diagnostics: result
                .diagnostics
                .into_iter()
                .map(|d| BpmnDiagnosticResult {
                    severity: d.severity,
                    message: d.message,
                    element_id: d.element_id,
                })
                .collect(),
        };
        Ok(ExecutionResult::Record(serde_json::to_value(typed)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("bpmn.compile requires database feature"))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let bpmn_xml = json_extract_string(args, "bpmn-xml")?;
        let client = get_bpmn_client()?;
        let result = client.compile(&bpmn_xml).await?;

        let typed = BpmnCompileResult {
            bytecode_version_hex: hex::encode(&result.bytecode_version),
            diagnostic_count: result.diagnostics.len(),
            diagnostics: result
                .diagnostics
                .into_iter()
                .map(|d| BpmnDiagnosticResult {
                    severity: d.severity,
                    message: d.message,
                    element_id: d.element_id,
                })
                .collect(),
        };
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(typed)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// bpmn.start
// =============================================================================

#[register_custom_op]
pub struct BpmnStartOp;

#[async_trait]
impl CustomOperation for BpmnStartOp {
    fn domain(&self) -> &'static str {
        "bpmn"
    }
    fn verb(&self) -> &'static str {
        "start"
    }
    fn rationale(&self) -> &'static str {
        "Calls bpmn-lite gRPC StartProcess RPC"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let process_key = get_required_string(verb_call, "process-key")?;
        let payload = get_optional_string(verb_call, "payload").unwrap_or_else(|| "{}".to_string());

        let (canonical, hash) = crate::bpmn_integration::canonical::canonical_json_with_hash(
            &serde_json::from_str(&payload)
                .unwrap_or(serde_json::Value::Object(Default::default())),
        );

        let client = get_bpmn_client()?;
        let instance_id = client
            .start_process(crate::bpmn_integration::client::StartProcessRequest {
                process_key,
                bytecode_version: Vec::new(), // Service resolves by process_key
                domain_payload: canonical,
                domain_payload_hash: hash,
                session_stack: SessionStackState::default(),
                orch_flags: std::collections::HashMap::new(),
                correlation_id: Uuid::now_v7(),
                entry_id: Uuid::nil(),
                runbook_id: Uuid::nil(),
            })
            .await?;

        Ok(ExecutionResult::Uuid(instance_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("bpmn.start requires database feature"))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let process_key = json_extract_string(args, "process-key")?;
        let payload = json_extract_string_opt(args, "payload").unwrap_or_else(|| "{}".to_string());

        let (canonical, hash) = crate::bpmn_integration::canonical::canonical_json_with_hash(
            &serde_json::from_str(&payload)
                .unwrap_or(serde_json::Value::Object(Default::default())),
        );

        let client = get_bpmn_client()?;
        let instance_id = client
            .start_process(crate::bpmn_integration::client::StartProcessRequest {
                process_key,
                bytecode_version: Vec::new(),
                domain_payload: canonical,
                domain_payload_hash: hash,
                session_stack: SessionStackState::default(),
                orch_flags: std::collections::HashMap::new(),
                correlation_id: Uuid::now_v7(),
                entry_id: Uuid::nil(),
                runbook_id: Uuid::nil(),
            })
            .await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Uuid(
            instance_id,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// bpmn.signal
// =============================================================================

#[register_custom_op]
pub struct BpmnSignalOp;

#[async_trait]
impl CustomOperation for BpmnSignalOp {
    fn domain(&self) -> &'static str {
        "bpmn"
    }
    fn verb(&self) -> &'static str {
        "signal"
    }
    fn rationale(&self) -> &'static str {
        "Calls bpmn-lite gRPC Signal RPC"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let instance_id = get_required_uuid(verb_call, "instance-id")?;
        let message_name = get_required_string(verb_call, "message-name")?;
        let payload = get_optional_string(verb_call, "payload");

        let client = get_bpmn_client()?;
        client
            .signal(
                instance_id,
                &message_name,
                payload.as_ref().map(|p| p.as_bytes()),
            )
            .await?;

        Ok(ExecutionResult::Void)
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("bpmn.signal requires database feature"))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let instance_id = json_get_required_uuid(args, "instance-id")?;
        let message_name = json_extract_string(args, "message-name")?;
        let payload = json_extract_string_opt(args, "payload");

        let client = get_bpmn_client()?;
        client
            .signal(
                instance_id,
                &message_name,
                payload.as_ref().map(|p| p.as_bytes()),
            )
            .await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Void)
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// bpmn.cancel
// =============================================================================

#[register_custom_op]
pub struct BpmnCancelOp;

#[async_trait]
impl CustomOperation for BpmnCancelOp {
    fn domain(&self) -> &'static str {
        "bpmn"
    }
    fn verb(&self) -> &'static str {
        "cancel"
    }
    fn rationale(&self) -> &'static str {
        "Calls bpmn-lite gRPC Cancel RPC"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let instance_id = get_required_uuid(verb_call, "instance-id")?;
        let reason = get_optional_string(verb_call, "reason")
            .unwrap_or_else(|| "Cancelled by operator".to_string());

        let client = get_bpmn_client()?;
        client.cancel(instance_id, &reason).await?;

        Ok(ExecutionResult::Void)
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("bpmn.cancel requires database feature"))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let instance_id = json_get_required_uuid(args, "instance-id")?;
        let reason = json_extract_string_opt(args, "reason")
            .unwrap_or_else(|| "Cancelled by operator".to_string());

        let client = get_bpmn_client()?;
        client.cancel(instance_id, &reason).await?;

        Ok(dsl_runtime::VerbExecutionOutcome::Void)
    }

    fn is_migrated(&self) -> bool {
        true
    }
}

// =============================================================================
// bpmn.inspect
// =============================================================================

#[register_custom_op]
pub struct BpmnInspectOp;

#[async_trait]
impl CustomOperation for BpmnInspectOp {
    fn domain(&self) -> &'static str {
        "bpmn"
    }
    fn verb(&self) -> &'static str {
        "inspect"
    }
    fn rationale(&self) -> &'static str {
        "Calls bpmn-lite gRPC Inspect RPC"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        _pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let instance_id = get_required_uuid(verb_call, "instance-id")?;

        let client = get_bpmn_client()?;
        let inspection = client.inspect(instance_id).await?;

        let typed = BpmnInspectResult {
            state: inspection.state,
            fiber_count: inspection.fibers.len(),
            wait_count: inspection.waits.len(),
            bytecode_version_hex: hex::encode(&inspection.bytecode_version),
            domain_payload_hash: inspection.domain_payload_hash,
        };
        Ok(ExecutionResult::Record(serde_json::to_value(typed)?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("bpmn.inspect requires database feature"))
    }

    #[cfg(feature = "database")]
    async fn execute_json(
        &self,
        args: &serde_json::Value,
        _ctx: &mut dsl_runtime::VerbExecutionContext,
        _pool: &PgPool,
    ) -> Result<dsl_runtime::VerbExecutionOutcome> {
        let instance_id = json_get_required_uuid(args, "instance-id")?;

        let client = get_bpmn_client()?;
        let inspection = client.inspect(instance_id).await?;

        let typed = BpmnInspectResult {
            state: inspection.state,
            fiber_count: inspection.fibers.len(),
            wait_count: inspection.waits.len(),
            bytecode_version_hex: hex::encode(&inspection.bytecode_version),
            domain_payload_hash: inspection.domain_payload_hash,
        };
        Ok(dsl_runtime::VerbExecutionOutcome::Record(
            serde_json::to_value(typed)?,
        ))
    }

    fn is_migrated(&self) -> bool {
        true
    }
}
