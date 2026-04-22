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

    /// Phase F.1 (Pattern B A1 remediation, 2026-04-22): bpmn.signal was a
    /// "fire-and-forget" gRPC call inside the verb body — a direct A1
    /// violation per three-plane v0.3 §11.2. This impl now defers the
    /// gRPC call to the outbox: the verb writes a `bpmn_signal` row into
    /// `public.outbox` inside the ambient transaction scope and returns
    /// `Void` synchronously. The drainer consumer (to be registered in
    /// `ob-poc-web::main` alongside `MaintenanceSpawnConsumer`) performs
    /// the actual `client.signal(...)` post-commit.
    ///
    /// Why outbox rather than two-phase:
    ///  - `bpmn.signal` has no return value — callers don't wait on the
    ///    signal to reach the BPMN service. Outbox deferral preserves
    ///    the synchronous contract while moving the external call out
    ///    of the verb body.
    ///  - Outbox deferral also preserves atomicity: if the enclosing
    ///    transaction rolls back, the outbox row is gone too — the
    ///    BPMN signal is never sent. A two-phase approach (fire gRPC
    ///    now, rollback later) would leak the signal to the BPMN
    ///    service even on outer-txn failure.
    ///
    /// Idempotency key:
    ///   `bpmn_signal:<instance_id>:<message_name>:<payload_hash>`
    /// Two identical signals within the same transaction collapse to
    /// one outbox row (the unique index on `(idempotency_key,
    /// effect_kind)` dedupes silently via `ON CONFLICT DO NOTHING`).
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instance_id = json_get_required_uuid(args, "instance-id")?;
        let message_name = json_extract_string(args, "message-name")?;
        let payload = json_extract_string_opt(args, "payload");

        let outbox_id = uuid::Uuid::new_v4();
        let trace_id = uuid::Uuid::new_v4();

        // Idempotency: BLAKE3 of (message_name || payload_bytes) keeps the
        // key bounded regardless of payload size.
        let payload_bytes = payload.as_deref().unwrap_or("").as_bytes();
        let mut hasher = blake3::Hasher::new();
        hasher.update(message_name.as_bytes());
        hasher.update(b"\x00"); // separator
        hasher.update(payload_bytes);
        let payload_hash = hasher.finalize().to_hex().to_string();
        let idempotency_key = format!(
            "bpmn_signal:{}:{}:{}",
            instance_id, message_name, &payload_hash[..16]
        );

        let outbox_payload = serde_json::json!({
            "instance_id": instance_id,
            "message_name": message_name,
            "payload": payload,
        });

        sqlx::query(
            r#"
            INSERT INTO public.outbox
                (id, trace_id, envelope_version, effect_kind, payload, idempotency_key, status)
            VALUES
                ($1, $2, $3, $4, $5, $6, 'pending')
            ON CONFLICT (idempotency_key, effect_kind) DO NOTHING
            "#,
        )
        .bind(outbox_id)
        .bind(trace_id)
        .bind(1i16) // EnvelopeVersion::CURRENT
        .bind("bpmn_signal")
        .bind(&outbox_payload)
        .bind(&idempotency_key)
        .execute(scope.executor())
        .await?;

        tracing::info!(
            %instance_id,
            %message_name,
            %idempotency_key,
            "bpmn.signal queued to public.outbox (Phase F.1 outbox deferral)"
        );

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

    /// Phase F.1 (2026-04-22): same outbox-deferral pattern as
    /// `BpmnSignal`. Fire-and-forget cancel — no return value, caller
    /// doesn't wait on the BPMN service to acknowledge.
    /// Idempotency key: `bpmn_cancel:<instance_id>:<reason-hash-16>`.
    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let instance_id = json_get_required_uuid(args, "instance-id")?;
        let reason = json_extract_string_opt(args, "reason")
            .unwrap_or_else(|| "Cancelled by operator".to_string());

        let outbox_id = uuid::Uuid::new_v4();
        let trace_id = uuid::Uuid::new_v4();

        let reason_hash = blake3::hash(reason.as_bytes()).to_hex().to_string();
        let idempotency_key = format!(
            "bpmn_cancel:{}:{}",
            instance_id,
            &reason_hash[..16]
        );

        let outbox_payload = serde_json::json!({
            "instance_id": instance_id,
            "reason": reason,
        });

        sqlx::query(
            r#"
            INSERT INTO public.outbox
                (id, trace_id, envelope_version, effect_kind, payload, idempotency_key, status)
            VALUES
                ($1, $2, $3, $4, $5, $6, 'pending')
            ON CONFLICT (idempotency_key, effect_kind) DO NOTHING
            "#,
        )
        .bind(outbox_id)
        .bind(trace_id)
        .bind(1i16)
        .bind("bpmn_cancel")
        .bind(&outbox_payload)
        .bind(&idempotency_key)
        .execute(scope.executor())
        .await?;

        tracing::info!(
            %instance_id,
            reason = %reason,
            %idempotency_key,
            "bpmn.cancel queued to public.outbox (Phase F.1 outbox deferral)"
        );

        Ok(VerbExecutionOutcome::Void)
    }
}

// =============================================================================
// bpmn.inspect
//
// Phase F.1 (2026-04-22, Pattern B ledger §3.1): gRPC call moved from
// the `execute` body into `pre_fetch`. The dispatcher calls `pre_fetch`
// BEFORE opening the transaction scope, so the external `client.inspect`
// round-trip happens outside the inner txn — A1 invariant satisfied.
//
// `pre_fetch` returns the inspection payload under the `_inspection`
// key; `execute` reads it back from args and formats the typed result
// with no I/O of its own.
// =============================================================================

pub struct BpmnInspect;

#[async_trait]
impl SemOsVerbOp for BpmnInspect {
    fn fqn(&self) -> &str {
        "bpmn.inspect"
    }

    async fn pre_fetch(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
    ) -> Result<Option<serde_json::Value>> {
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
        Ok(Some(serde_json::json!({
            "_inspection": serde_json::to_value(typed)?
        })))
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        // Pre-fetch populated `_inspection`. If it's missing something
        // went wrong upstream — surface that rather than silently
        // falling back to an in-txn gRPC call.
        let inspection = args
            .get("_inspection")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!(
                "bpmn.inspect: pre_fetch result missing — dispatcher did not \
                 merge `_inspection` into args. This indicates a dispatcher \
                 regression (Phase F.1 pre-fetch hook must run before execute)."
            ))?;
        Ok(VerbExecutionOutcome::Record(inspection))
    }
}
