//! BPMN-Lite control verbs (5 plugin verbs) — `bpmn.{compile, start,
//! signal, cancel, inspect}`. Direct gRPC pass-throughs to the
//! bpmn-lite service, reached via `crate::bpmn_integration::client`.
//!
//! Phase 5c-migrate Phase B Pattern B slice #73: ported from
//! `CustomOperation` + `inventory::collect!` to `SemOsVerbOp`. Stays
//! in `ob-poc::domain_ops` because the client types live in
//! `ob-poc::bpmn_integration` (not upstream of `sem_os_postgres`).

use anyhow::Result;
use async_trait::async_trait;
use ob_poc_types::session_stack::SessionStackState;
use sem_os_postgres::ops::SemOsVerbOp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_get_required_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

// =============================================================================
// Result types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmnCompileResult {
    pub bytecode_version_hex: String,
    pub diagnostic_count: usize,
    pub diagnostics: Vec<BpmnDiagnosticResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmnDiagnosticResult {
    pub severity: String,
    pub message: String,
    pub element_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BpmnInspectResult {
    pub state: String,
    pub fiber_count: usize,
    pub wait_count: usize,
    pub bytecode_version_hex: String,
    pub domain_payload_hash: String,
}

fn get_bpmn_client() -> Result<crate::bpmn_integration::client::BpmnLiteConnection> {
    crate::bpmn_integration::client::BpmnLiteConnection::from_env()
}

// =============================================================================
// bpmn.compile
// =============================================================================

pub struct BpmnCompile;

#[async_trait]
impl SemOsVerbOp for BpmnCompile {
    fn fqn(&self) -> &str {
        "bpmn.compile"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(typed)?))
    }
}

// =============================================================================
// bpmn.start
// =============================================================================

pub struct BpmnStart;

#[async_trait]
impl SemOsVerbOp for BpmnStart {
    fn fqn(&self) -> &str {
        "bpmn.start"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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

        Ok(VerbExecutionOutcome::Uuid(instance_id))
    }
}

// =============================================================================
// bpmn.signal
// =============================================================================

pub struct BpmnSignal;

#[async_trait]
impl SemOsVerbOp for BpmnSignal {
    fn fqn(&self) -> &str {
        "bpmn.signal"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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

        Ok(VerbExecutionOutcome::Void)
    }
}

// =============================================================================
// bpmn.cancel
// =============================================================================

pub struct BpmnCancel;

#[async_trait]
impl SemOsVerbOp for BpmnCancel {
    fn fqn(&self) -> &str {
        "bpmn.cancel"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instance_id = json_get_required_uuid(args, "instance-id")?;
        let reason = json_extract_string_opt(args, "reason")
            .unwrap_or_else(|| "Cancelled by operator".to_string());

        let client = get_bpmn_client()?;
        client.cancel(instance_id, &reason).await?;

        Ok(VerbExecutionOutcome::Void)
    }
}

// =============================================================================
// bpmn.inspect
// =============================================================================

pub struct BpmnInspect;

#[async_trait]
impl SemOsVerbOp for BpmnInspect {
    fn fqn(&self) -> &str {
        "bpmn.inspect"
    }
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
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
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(typed)?))
    }
}
