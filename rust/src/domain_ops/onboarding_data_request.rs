//! Onboarding data-request SemOS verbs.
//!
//! These Pattern B ops bridge SemOS execution to the in-crate
//! service-resource data-dictionary implementation.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{json_extract_string_opt, json_extract_uuid};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use sem_os_postgres::ops::SemOsVerbOp;

use crate::service_resources::OnboardingDataRequestService;

fn arg_uuid_opt(args: &Value, ctx: &VerbExecutionContext, name: &str) -> Option<Uuid> {
    args.get(name).and_then(Value::as_str).and_then(|s| {
        if let Some(symbol) = s.strip_prefix('@') {
            ctx.resolve(symbol)
        } else {
            Uuid::parse_str(s).ok()
        }
    })
}

fn arg_payload(args: &Value) -> Value {
    args.get("payload")
        .or_else(|| args.get("result"))
        .cloned()
        .unwrap_or_else(|| json!({}))
}

/// Compile the onboarding service-resource data request.
pub struct CompileDataRequest;

#[async_trait]
impl SemOsVerbOp for CompileDataRequest {
    fn fqn(&self) -> &str {
        "onboarding.compile-data-request"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let onboarding_request_id = json_extract_uuid(args, ctx, "onboarding-request-id")?;
        let service = OnboardingDataRequestService::new(scope.pool().clone());
        let result = service.compile_data_request(onboarding_request_id).await?;
        ctx.bind("data_request", result.data_request_id);
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Dispatch all currently ready slices for a data request.
pub struct DispatchReadySlices;

#[async_trait]
impl SemOsVerbOp for DispatchReadySlices {
    fn fqn(&self) -> &str {
        "onboarding.dispatch-ready-slices"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let data_request_id = json_extract_uuid(args, ctx, "data-request-id")?;
        let service = OnboardingDataRequestService::new(scope.pool().clone());
        let result = service.dispatch_ready_slices(data_request_id).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Confirm the owner-returned provisioning result.
pub struct ConfirmProvisioningResult;

#[async_trait]
impl SemOsVerbOp for ConfirmProvisioningResult {
    fn fqn(&self) -> &str {
        "service-resource.confirm-provisioning-result"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let provisioning_request_id = json_extract_uuid(args, ctx, "provisioning-request-id")?;
        let payload = arg_payload(args);
        let content_hash = json_extract_string_opt(args, "content-hash");
        let service = OnboardingDataRequestService::new(scope.pool().clone());
        let result = service
            .confirm_provisioning_result(provisioning_request_id, payload, content_hash)
            .await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Cancel all open slices for a data request.
pub struct CancelDataRequest;

#[async_trait]
impl SemOsVerbOp for CancelDataRequest {
    fn fqn(&self) -> &str {
        "onboarding.cancel-data-request"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let data_request_id = json_extract_uuid(args, ctx, "data-request-id")?;
        let service = OnboardingDataRequestService::new(scope.pool().clone());
        let result = service.cancel_data_request(data_request_id).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Cancel one open data-request slice.
pub struct CancelSlice;

#[async_trait]
impl SemOsVerbOp for CancelSlice {
    fn fqn(&self) -> &str {
        "onboarding.cancel-slice"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let slice_id = json_extract_uuid(args, ctx, "slice-id")?;
        let service = OnboardingDataRequestService::new(scope.pool().clone());
        let result = service.cancel_slice(slice_id).await?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Read one data request.
pub struct GetDataRequest;

#[async_trait]
impl SemOsVerbOp for GetDataRequest {
    fn fqn(&self) -> &str {
        "onboarding.get-data-request"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let data_request_id = json_extract_uuid(args, ctx, "data-request-id")?;
        let row = sqlx::query(
            r#"
            SELECT data_request_id, onboarding_request_id, deal_id, contract_id, cbu_id,
                   product_id, request_status, compiled_at, completed_at, cancelled_at,
                   blocking_reason
            FROM "ob-poc".onboarding_data_requests
            WHERE data_request_id = $1
            "#,
        )
        .bind(data_request_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("data request not found: {data_request_id}"))?;
        Ok(VerbExecutionOutcome::Record(json!({
            "data_request_id": row.get::<Uuid, _>("data_request_id"),
            "onboarding_request_id": row.get::<Uuid, _>("onboarding_request_id"),
            "deal_id": row.get::<Uuid, _>("deal_id"),
            "contract_id": row.get::<Uuid, _>("contract_id"),
            "cbu_id": row.get::<Uuid, _>("cbu_id"),
            "product_id": row.get::<Uuid, _>("product_id"),
            "request_status": row.get::<String, _>("request_status"),
            "compiled_at": row.get::<chrono::DateTime<chrono::Utc>, _>("compiled_at"),
            "completed_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("completed_at"),
            "cancelled_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("cancelled_at"),
            "blocking_reason": row.get::<Option<String>, _>("blocking_reason"),
        })))
    }
}

/// List data requests by optional CBU and status filters.
pub struct ListDataRequests;

#[async_trait]
impl SemOsVerbOp for ListDataRequests {
    fn fqn(&self) -> &str {
        "onboarding.list-data-requests"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = arg_uuid_opt(args, ctx, "cbu-id");
        let status = json_extract_string_opt(args, "status");
        let rows = sqlx::query(
            r#"
            SELECT data_request_id, onboarding_request_id, cbu_id, product_id, request_status,
                   compiled_at
            FROM "ob-poc".onboarding_data_requests
            WHERE ($1::uuid IS NULL OR cbu_id = $1)
              AND ($2::text IS NULL OR request_status = $2)
            ORDER BY compiled_at DESC
            LIMIT 200
            "#,
        )
        .bind(cbu_id)
        .bind(status)
        .fetch_all(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::RecordSet(
            rows.into_iter()
                .map(|row| {
                    json!({
                        "data_request_id": row.get::<Uuid, _>("data_request_id"),
                        "onboarding_request_id": row.get::<Uuid, _>("onboarding_request_id"),
                        "cbu_id": row.get::<Uuid, _>("cbu_id"),
                        "product_id": row.get::<Uuid, _>("product_id"),
                        "request_status": row.get::<String, _>("request_status"),
                        "compiled_at": row.get::<chrono::DateTime<chrono::Utc>, _>("compiled_at"),
                    })
                })
                .collect(),
        ))
    }
}

/// List slices for a data request.
pub struct ListSlices;

#[async_trait]
impl SemOsVerbOp for ListSlices {
    fn fqn(&self) -> &str {
        "onboarding.list-slices"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let data_request_id = json_extract_uuid(args, ctx, "data-request-id")?;
        let rows = sqlx::query(
            r#"
            SELECT slice_id, data_request_id, srdef_id, parameters, owner_system,
                   owner_principal_fqn, slice_status, blocking_reason,
                   provisioning_request_id
            FROM "ob-poc".onboarding_data_request_slices
            WHERE data_request_id = $1
            ORDER BY srdef_id, parameters::text
            "#,
        )
        .bind(data_request_id)
        .fetch_all(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::RecordSet(
            rows.into_iter()
                .map(|row| {
                    json!({
                        "slice_id": row.get::<Uuid, _>("slice_id"),
                        "data_request_id": row.get::<Uuid, _>("data_request_id"),
                        "srdef_id": row.get::<String, _>("srdef_id"),
                        "parameters": row.get::<Value, _>("parameters"),
                        "owner_system": row.get::<Option<String>, _>("owner_system"),
                        "owner_principal_fqn": row.get::<Option<String>, _>("owner_principal_fqn"),
                        "slice_status": row.get::<String, _>("slice_status"),
                        "blocking_reason": row.get::<Option<String>, _>("blocking_reason"),
                        "provisioning_request_id": row.get::<Option<Uuid>, _>("provisioning_request_id"),
                    })
                })
                .collect(),
        ))
    }
}

/// Read one slice.
pub struct GetSlice;

#[async_trait]
impl SemOsVerbOp for GetSlice {
    fn fqn(&self) -> &str {
        "onboarding.get-slice"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let slice_id = json_extract_uuid(args, ctx, "slice-id")?;
        let row = sqlx::query(
            r#"
            SELECT slice_id, data_request_id, onboarding_request_id, cbu_id, srdef_id,
                   resource_type_id, parameters, owner_system, owner_principal_fqn,
                   slice_status, blocking_reason, cbu_resource_instance_id,
                   provisioning_request_id
            FROM "ob-poc".onboarding_data_request_slices
            WHERE slice_id = $1
            "#,
        )
        .bind(slice_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("slice not found: {slice_id}"))?;
        Ok(VerbExecutionOutcome::Record(json!({
            "slice_id": row.get::<Uuid, _>("slice_id"),
            "data_request_id": row.get::<Uuid, _>("data_request_id"),
            "onboarding_request_id": row.get::<Uuid, _>("onboarding_request_id"),
            "cbu_id": row.get::<Uuid, _>("cbu_id"),
            "srdef_id": row.get::<String, _>("srdef_id"),
            "resource_type_id": row.get::<Option<Uuid>, _>("resource_type_id"),
            "parameters": row.get::<Value, _>("parameters"),
            "owner_system": row.get::<Option<String>, _>("owner_system"),
            "owner_principal_fqn": row.get::<Option<String>, _>("owner_principal_fqn"),
            "slice_status": row.get::<String, _>("slice_status"),
            "blocking_reason": row.get::<Option<String>, _>("blocking_reason"),
            "cbu_resource_instance_id": row.get::<Option<Uuid>, _>("cbu_resource_instance_id"),
            "provisioning_request_id": row.get::<Option<Uuid>, _>("provisioning_request_id"),
        })))
    }
}

/// List attributes for a slice.
pub struct GetSliceAttrs;

#[async_trait]
impl SemOsVerbOp for GetSliceAttrs {
    fn fqn(&self) -> &str {
        "onboarding.get-slice-attrs"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let slice_id = json_extract_uuid(args, ctx, "slice-id")?;
        let rows = sqlx::query(
            r#"
            SELECT slice_id, attr_id, attr_code, requirement_strength,
                   condition_expression, condition_status, source_policy,
                   evidence_policy, merged_constraints, value_status, value_ref,
                   value_observed_at, blocking_reason
            FROM "ob-poc".onboarding_data_request_attrs
            WHERE slice_id = $1
            ORDER BY attr_code
            "#,
        )
        .bind(slice_id)
        .fetch_all(scope.executor())
        .await?;
        Ok(VerbExecutionOutcome::RecordSet(
            rows.into_iter()
                .map(|row| {
                    json!({
                        "slice_id": row.get::<Uuid, _>("slice_id"),
                        "attr_id": row.get::<Uuid, _>("attr_id"),
                        "attr_code": row.get::<Option<String>, _>("attr_code"),
                        "requirement_strength": row.get::<String, _>("requirement_strength"),
                        "condition_expression": row.get::<Option<String>, _>("condition_expression"),
                        "condition_status": row.get::<String, _>("condition_status"),
                        "source_policy": row.get::<Value, _>("source_policy"),
                        "evidence_policy": row.get::<Value, _>("evidence_policy"),
                        "merged_constraints": row.get::<Value, _>("merged_constraints"),
                        "value_status": row.get::<String, _>("value_status"),
                        "value_ref": row.get::<Option<Value>, _>("value_ref"),
                        "value_observed_at": row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("value_observed_at"),
                        "blocking_reason": row.get::<Option<String>, _>("blocking_reason"),
                    })
                })
                .collect(),
        ))
    }
}

use sqlx::Row;
