//! Onboarding-scoped service-resource data dictionary.
//!
//! This module materialises a frozen per-onboarding dictionary from the
//! existing SRDEF discovery and attribute requirement substrate, then drives
//! the resource-owner dispatch/return loop through `provisioning_requests`,
//! `provisioning_events`, and `public.outbox`.

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use ob_poc_types::OutboxEffectKind;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::collections::HashMap;
use std::sync::LazyLock;
use uuid::Uuid;

static SIMPLE_CONDITION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*(==|!=)\s*'([^']*)'\s*$"#)
        .expect("valid condition regex")
});

/// Result returned by `onboarding.compile-data-request`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CompileDataRequestResult {
    pub data_request_id: Uuid,
    pub onboarding_request_id: Uuid,
    pub cbu_id: Uuid,
    pub status: String,
    pub slices_created: u64,
    pub attrs_created: u64,
    pub already_existed: bool,
}

/// Result returned after dispatching ready data-request slices.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DispatchReadySlicesResult {
    pub data_request_id: Uuid,
    pub dispatched_slices: u64,
    pub outbox_rows_created: u64,
}

/// Result returned by owner confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConfirmProvisioningResult {
    pub provisioning_request_id: Uuid,
    pub event_id: Option<Uuid>,
    pub was_duplicate: bool,
    pub new_status: String,
}

/// Result returned by cancellation verbs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CancellationResult {
    pub affected_slices: u64,
    pub outbox_rows_created: u64,
}

/// Service for compiling and progressing onboarding data requests.
pub(crate) struct OnboardingDataRequestService {
    pool: PgPool,
}

impl OnboardingDataRequestService {
    /// Create a service bound to a Postgres pool.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let service = OnboardingDataRequestService::new(pool.clone());
    /// ```
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Compile the frozen data dictionary for one deal onboarding request.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = service.compile_data_request(onboarding_request_id).await?;
    /// assert_eq!(result.onboarding_request_id, onboarding_request_id);
    /// ```
    pub(crate) async fn compile_data_request(
        &self,
        onboarding_request_id: Uuid,
    ) -> Result<CompileDataRequestResult> {
        let mut tx = self.pool.begin().await?;
        let request = load_onboarding_request(&mut tx, onboarding_request_id).await?;

        let existing = sqlx::query(
            r#"
            SELECT data_request_id, request_status
            FROM "ob-poc".onboarding_data_requests
            WHERE onboarding_request_id = $1
            "#,
        )
        .bind(onboarding_request_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(row) = existing {
            let data_request_id = row.get::<Uuid, _>("data_request_id");
            let status = row.get::<String, _>("request_status");
            tx.commit().await?;
            return Ok(CompileDataRequestResult {
                data_request_id,
                onboarding_request_id,
                cbu_id: request.cbu_id,
                status,
                slices_created: 0,
                attrs_created: 0,
                already_existed: true,
            });
        }

        let data_request_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".onboarding_data_requests
                (onboarding_request_id, deal_id, contract_id, cbu_id, product_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING data_request_id
            "#,
        )
        .bind(onboarding_request_id)
        .bind(request.deal_id)
        .bind(request.contract_id)
        .bind(request.cbu_id)
        .bind(request.product_id)
        .fetch_one(&mut *tx)
        .await?;

        let discoveries = load_active_discoveries(&mut tx, request.cbu_id).await?;
        let mut slices_created = 0_u64;
        let mut attrs_created = 0_u64;

        for discovery in discoveries {
            let discovery_snapshot_id =
                insert_discovery_snapshot(&mut tx, data_request_id, &discovery).await?;
            let slice_id = insert_slice(
                &mut tx,
                data_request_id,
                onboarding_request_id,
                request.cbu_id,
                discovery_snapshot_id,
                &discovery,
            )
            .await?;
            slices_created += 1;
            attrs_created +=
                insert_slice_attrs(&mut tx, slice_id, request.cbu_id, &discovery).await?;
            recompute_slice_status(&mut tx, slice_id).await?;
        }

        let status = recompute_request_status(&mut tx, data_request_id).await?;
        sqlx::query(
            r#"
            UPDATE "ob-poc".deal_onboarding_requests
            SET request_status = CASE
                    WHEN request_status = 'PENDING' THEN 'IN_PROGRESS'
                    ELSE request_status
                END,
                updated_at = now()
            WHERE request_id = $1
            "#,
        )
        .bind(onboarding_request_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(CompileDataRequestResult {
            data_request_id,
            onboarding_request_id,
            cbu_id: request.cbu_id,
            status,
            slices_created,
            attrs_created,
            already_existed: false,
        })
    }

    /// Dispatch all currently ready slices for a compiled data request.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = service.dispatch_ready_slices(data_request_id).await?;
    /// assert!(result.dispatched_slices <= result.outbox_rows_created);
    /// ```
    pub(crate) async fn dispatch_ready_slices(
        &self,
        data_request_id: Uuid,
    ) -> Result<DispatchReadySlicesResult> {
        let mut tx = self.pool.begin().await?;
        let slices = load_ready_slices(&mut tx, data_request_id).await?;
        let mut dispatched_slices = 0_u64;
        let mut outbox_rows_created = 0_u64;

        for slice in slices {
            if !slice.owner_dispatch_enabled {
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".onboarding_data_request_slices
                    SET slice_status = 'blocked',
                        blocking_reason = $1,
                        updated_at = now()
                    WHERE slice_id = $2
                    "#,
                )
                .bind(format!(
                    "resource owner principal {} is not dispatch-enabled",
                    slice
                        .owner_principal_fqn
                        .as_deref()
                        .unwrap_or("<unassigned>")
                ))
                .bind(slice.slice_id)
                .execute(&mut *tx)
                .await?;
                recompute_request_status(&mut tx, data_request_id).await?;
                continue;
            }

            let dispatch_key = format!(
                "resource-owner-dispatch:{}:{}",
                slice.data_request_id, slice.slice_id
            );
            let instance_id = ensure_resource_instance(&mut tx, &slice).await?;
            let provisioning_request_id =
                ensure_provisioning_request(&mut tx, &slice, instance_id, &dispatch_key).await?;
            let event_inserted = insert_provisioning_event(
                &mut tx,
                provisioning_request_id,
                "OUT",
                "REQUEST_PREPARED",
                json!({
                    "slice_id": slice.slice_id,
                    "data_request_id": slice.data_request_id,
                    "srdef_id": slice.srdef_id,
                    "parameters": slice.parameters,
                    "instance_id": instance_id,
                }),
                Some(&format!("request-prepared:{provisioning_request_id}")),
            )
            .await?;
            let outbox_inserted = insert_owner_outbox(
                &mut tx,
                OutboxEffectKind::ResourceOwnerDispatch,
                &dispatch_key,
                json!({
                    "provisioning_request_id": provisioning_request_id,
                    "slice_id": slice.slice_id,
                    "data_request_id": slice.data_request_id,
                    "owner_system": slice.owner_system,
                    "owner_principal_fqn": slice.owner_principal_fqn,
                    "dispatch_endpoint": slice.dispatch_endpoint,
                }),
            )
            .await?;

            sqlx::query(
                r#"
                UPDATE "ob-poc".onboarding_data_request_slices
                SET slice_status = 'dispatched',
                    cbu_resource_instance_id = $1,
                    provisioning_request_id = $2,
                    dispatched_at = COALESCE(dispatched_at, now()),
                    updated_at = now()
                WHERE slice_id = $3
                "#,
            )
            .bind(instance_id)
            .bind(provisioning_request_id)
            .bind(slice.slice_id)
            .execute(&mut *tx)
            .await?;

            if event_inserted || outbox_inserted {
                dispatched_slices += 1;
            }
            if outbox_inserted {
                outbox_rows_created += 1;
            }
        }

        recompute_request_status(&mut tx, data_request_id).await?;
        tx.commit().await?;

        Ok(DispatchReadySlicesResult {
            data_request_id,
            dispatched_slices,
            outbox_rows_created,
        })
    }

    /// Confirm a provisioning result returned by a resource owner.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = service
    ///     .confirm_provisioning_result(request_id, payload, Some(hash))
    ///     .await?;
    /// assert!(!result.new_status.is_empty());
    /// ```
    pub(crate) async fn confirm_provisioning_result(
        &self,
        provisioning_request_id: Uuid,
        payload: Value,
        content_hash: Option<String>,
    ) -> Result<ConfirmProvisioningResult> {
        let mut tx = self.pool.begin().await?;
        if let Some(hash) = content_hash.as_deref() {
            let existing: Option<Uuid> = sqlx::query_scalar(
                r#"SELECT event_id FROM "ob-poc".provisioning_events WHERE content_hash = $1"#,
            )
            .bind(hash)
            .fetch_optional(&mut *tx)
            .await?;
            if existing.is_some() {
                tx.commit().await?;
                return Ok(ConfirmProvisioningResult {
                    provisioning_request_id,
                    event_id: existing,
                    was_duplicate: true,
                    new_status: "duplicate".to_string(),
                });
            }
        }

        let row = load_provisioning_request(&mut tx, provisioning_request_id).await?;
        let result_status = payload
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("pending")
            .to_ascii_lowercase();
        let success = result_status == "active";
        let pending = result_status == "pending" || result_status == "ack";
        let new_request_status = if success {
            "completed"
        } else if pending {
            "ack"
        } else {
            "failed"
        };
        let event_kind = if success || pending {
            "RESULT"
        } else {
            "ERROR"
        };
        let event_id = insert_provisioning_event_returning(
            &mut tx,
            provisioning_request_id,
            "IN",
            event_kind,
            payload.clone(),
            content_hash.as_deref(),
        )
        .await?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".provisioning_requests
            SET status = $1,
                owner_ticket_id = COALESCE($2, owner_ticket_id)
            WHERE request_id = $3
            "#,
        )
        .bind(new_request_status)
        .bind(payload.get("owner_ticket_id").and_then(Value::as_str))
        .bind(provisioning_request_id)
        .execute(&mut *tx)
        .await?;

        if let Some(instance_id) = row.instance_id {
            let locator = resource_locator_from_payload(&payload);
            let instance_status = if success {
                "ACTIVE"
            } else if pending {
                "AWAITING_OWNER"
            } else {
                "FAILED"
            };
            sqlx::query(
                r#"
                UPDATE "ob-poc".cbu_resource_instances
                SET status = $1,
                    resource_locator = CASE WHEN $1 = 'ACTIVE' THEN $2 ELSE resource_locator END,
                    resource_url = CASE WHEN $1 = 'ACTIVE' THEN COALESCE($3, resource_url) ELSE resource_url END,
                    owner_ticket_id = COALESCE($4, owner_ticket_id),
                    instance_identifier = CASE WHEN $1 = 'ACTIVE' THEN COALESCE($5, instance_identifier) ELSE instance_identifier END,
                    last_request_id = $6,
                    last_event_at = now(),
                    activated_at = CASE WHEN $1 = 'ACTIVE' THEN COALESCE(activated_at, now()) ELSE activated_at END,
                    updated_at = now()
                WHERE instance_id = $7
                "#,
            )
            .bind(instance_status)
            .bind(locator)
            .bind(payload.get("resource_url").and_then(Value::as_str))
            .bind(payload.get("owner_ticket_id").and_then(Value::as_str))
            .bind(payload.get("native_key").and_then(Value::as_str))
            .bind(provisioning_request_id)
            .bind(instance_id)
            .execute(&mut *tx)
            .await?;
        }

        if let Some(slice_id) = row.slice_id {
            let slice_status = if success {
                "activated"
            } else if pending {
                "awaiting_owner"
            } else {
                "failed"
            };
            sqlx::query(
                r#"
                UPDATE "ob-poc".onboarding_data_request_slices
                SET slice_status = $1,
                    activated_at = CASE WHEN $1 = 'activated' THEN COALESCE(activated_at, now()) ELSE activated_at END,
                    blocking_reason = CASE WHEN $1 = 'failed' THEN COALESCE($2, blocking_reason) ELSE blocking_reason END,
                    updated_at = now()
                WHERE slice_id = $3
                "#,
            )
            .bind(slice_status)
            .bind(failure_reason(&payload))
            .bind(slice_id)
            .execute(&mut *tx)
            .await?;

            if let Some(data_request_id) = row.data_request_id {
                recompute_request_status(&mut tx, data_request_id).await?;
            }
        }

        tx.commit().await?;

        Ok(ConfirmProvisioningResult {
            provisioning_request_id,
            event_id: Some(event_id),
            was_duplicate: false,
            new_status: new_request_status.to_string(),
        })
    }

    /// Cancel all open slices for a data request.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = service.cancel_data_request(data_request_id).await?;
    /// assert!(result.affected_slices >= 0);
    /// ```
    pub(crate) async fn cancel_data_request(&self, data_request_id: Uuid) -> Result<CancellationResult> {
        self.cancel_where("data_request_id = $1", data_request_id)
            .await
    }

    /// Cancel one open data-request slice.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = service.cancel_slice(slice_id).await?;
    /// assert!(result.affected_slices <= 1);
    /// ```
    pub(crate) async fn cancel_slice(&self, slice_id: Uuid) -> Result<CancellationResult> {
        self.cancel_where("slice_id = $1", slice_id).await
    }

    async fn cancel_where(&self, predicate: &str, id: Uuid) -> Result<CancellationResult> {
        let mut tx = self.pool.begin().await?;
        let query = format!(
            r#"
            SELECT slice_id, data_request_id, provisioning_request_id, cbu_resource_instance_id
            FROM "ob-poc".onboarding_data_request_slices
            WHERE {predicate}
              AND slice_status NOT IN ('activated', 'failed', 'cancelled')
            "#
        );
        let rows = sqlx::query(&query).bind(id).fetch_all(&mut *tx).await?;

        let mut outbox_rows_created = 0_u64;
        for row in &rows {
            let slice_id = row.get::<Uuid, _>("slice_id");
            let data_request_id = row.get::<Uuid, _>("data_request_id");
            let provisioning_request_id = row.get::<Option<Uuid>, _>("provisioning_request_id");
            let instance_id = row.get::<Option<Uuid>, _>("cbu_resource_instance_id");

            sqlx::query(
                r#"
                UPDATE "ob-poc".onboarding_data_request_slices
                SET slice_status = 'cancelled',
                    cancelled_at = COALESCE(cancelled_at, now()),
                    updated_at = now()
                WHERE slice_id = $1
                "#,
            )
            .bind(slice_id)
            .execute(&mut *tx)
            .await?;

            if let Some(request_id) = provisioning_request_id {
                insert_provisioning_event(
                    &mut tx,
                    request_id,
                    "OUT",
                    "STAND_DOWN",
                    json!({ "slice_id": slice_id, "data_request_id": data_request_id }),
                    Some(&format!("stand-down:{request_id}")),
                )
                .await?;
                if insert_owner_outbox(
                    &mut tx,
                    OutboxEffectKind::ResourceOwnerStandDown,
                    &format!("resource-owner-stand-down:{data_request_id}:{slice_id}"),
                    json!({
                        "provisioning_request_id": request_id,
                        "slice_id": slice_id,
                        "data_request_id": data_request_id,
                    }),
                )
                .await?
                {
                    outbox_rows_created += 1;
                }
                sqlx::query(
                    r#"UPDATE "ob-poc".provisioning_requests SET status = 'cancelled' WHERE request_id = $1"#,
                )
                .bind(request_id)
                .execute(&mut *tx)
                .await?;
            }

            if let Some(instance_id) = instance_id {
                sqlx::query(
                    r#"
                    UPDATE "ob-poc".cbu_resource_instances
                    SET status = 'CANCELLED', updated_at = now()
                    WHERE instance_id = $1 AND status <> 'ACTIVE'
                    "#,
                )
                .bind(instance_id)
                .execute(&mut *tx)
                .await?;
            }

            recompute_request_status(&mut tx, data_request_id).await?;
        }

        tx.commit().await?;
        Ok(CancellationResult {
            affected_slices: rows.len() as u64,
            outbox_rows_created,
        })
    }
}

#[derive(Debug)]
struct OnboardingRequestRow {
    deal_id: Uuid,
    contract_id: Uuid,
    cbu_id: Uuid,
    product_id: Uuid,
}

#[derive(Debug)]
struct DiscoveryRow {
    discovery_id: Uuid,
    srdef_id: String,
    resource_type_id: Option<Uuid>,
    srdef_snapshot_id: Option<Uuid>,
    parameters: Value,
    triggered_by_intents: Value,
    owner_system: Option<String>,
    owner_principal_fqn: Option<String>,
    provisioning_strategy: Option<String>,
    l4_binding_required: bool,
    bound_application_id: Option<Uuid>,
    bound_application_instance_id: Option<Uuid>,
    snapshot: Value,
}

#[derive(Debug)]
struct SliceRow {
    slice_id: Uuid,
    data_request_id: Uuid,
    onboarding_request_id: Uuid,
    cbu_id: Uuid,
    srdef_id: String,
    resource_type_id: Option<Uuid>,
    parameters: Value,
    owner_system: Option<String>,
    owner_principal_fqn: Option<String>,
    dispatch_endpoint: Option<String>,
    owner_dispatch_enabled: bool,
}

#[derive(Debug)]
struct ProvisioningRequestRow {
    data_request_id: Option<Uuid>,
    slice_id: Option<Uuid>,
    instance_id: Option<Uuid>,
}

async fn load_onboarding_request(
    tx: &mut Transaction<'_, Postgres>,
    onboarding_request_id: Uuid,
) -> Result<OnboardingRequestRow> {
    let row = sqlx::query(
        r#"
        SELECT deal_id, contract_id, cbu_id, product_id
        FROM "ob-poc".deal_onboarding_requests
        WHERE request_id = $1
        "#,
    )
    .bind(onboarding_request_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| anyhow!("onboarding request not found: {onboarding_request_id}"))?;

    Ok(OnboardingRequestRow {
        deal_id: row.get("deal_id"),
        contract_id: row.get("contract_id"),
        cbu_id: row.get("cbu_id"),
        product_id: row.get("product_id"),
    })
}

async fn load_active_discoveries(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
) -> Result<Vec<DiscoveryRow>> {
    let rows = sqlx::query(
        r#"
        SELECT
            d.discovery_id,
            d.srdef_id,
            d.resource_type_id,
            srt.srdef_snapshot_id,
            COALESCE(d.parameters, '{}'::jsonb) AS parameters,
            COALESCE(d.triggered_by_intents, '[]'::jsonb) AS triggered_by_intents,
            srt.owner AS owner_system,
            COALESCE(srt.owner_principal_fqn, 'resource_owner:' || srt.owner) AS owner_principal_fqn,
            srt.provisioning_strategy,
            COALESCE(srt.l4_binding_required, FALSE) AS l4_binding_required,
            srt.bound_application_id,
            srt.bound_application_instance_id,
            jsonb_build_object(
                'discovery_id', d.discovery_id,
                'srdef_id', d.srdef_id,
                'resource_type_id', d.resource_type_id,
                'triggered_by_intents', d.triggered_by_intents,
                'discovery_rule', d.discovery_rule,
                'discovery_reason', d.discovery_reason,
                'parameters', COALESCE(d.parameters, '{}'::jsonb),
                'service_resource_type', to_jsonb(srt)
            ) AS snapshot
        FROM "ob-poc".srdef_discovery_reasons d
        LEFT JOIN "ob-poc".service_resource_types srt ON srt.resource_id = d.resource_type_id
        WHERE d.cbu_id = $1
          AND d.superseded_at IS NULL
        ORDER BY d.srdef_id, COALESCE(d.parameters, '{}'::jsonb)::text
        "#,
    )
    .bind(cbu_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| DiscoveryRow {
            discovery_id: row.get("discovery_id"),
            srdef_id: row.get("srdef_id"),
            resource_type_id: row.get("resource_type_id"),
            srdef_snapshot_id: row.get("srdef_snapshot_id"),
            parameters: row.get("parameters"),
            triggered_by_intents: row.get("triggered_by_intents"),
            owner_system: row.get("owner_system"),
            owner_principal_fqn: row.get("owner_principal_fqn"),
            provisioning_strategy: row.get("provisioning_strategy"),
            l4_binding_required: row.get("l4_binding_required"),
            bound_application_id: row.get("bound_application_id"),
            bound_application_instance_id: row.get("bound_application_instance_id"),
            snapshot: row.get("snapshot"),
        })
        .collect())
}

async fn insert_discovery_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    data_request_id: Uuid,
    discovery: &DiscoveryRow,
) -> Result<Uuid> {
    sqlx::query_scalar(
        r#"
        INSERT INTO "ob-poc".onboarding_data_request_discoveries
            (data_request_id, source_discovery_id, srdef_id, resource_type_id, parameters, discovery_snapshot)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (data_request_id, srdef_id, parameters) DO UPDATE
        SET discovery_snapshot = EXCLUDED.discovery_snapshot
        RETURNING discovery_snapshot_id
        "#,
    )
    .bind(data_request_id)
    .bind(discovery.discovery_id)
    .bind(&discovery.srdef_id)
    .bind(discovery.resource_type_id)
    .bind(&discovery.parameters)
    .bind(&discovery.snapshot)
    .fetch_one(&mut **tx)
    .await
    .context("insert onboarding discovery snapshot")
}

async fn insert_slice(
    tx: &mut Transaction<'_, Postgres>,
    data_request_id: Uuid,
    onboarding_request_id: Uuid,
    cbu_id: Uuid,
    discovery_snapshot_id: Uuid,
    discovery: &DiscoveryRow,
) -> Result<Uuid> {
    ensure_resource_owner_principal(tx, discovery).await?;
    let l4 = resolve_l4_binding(tx, discovery).await?;
    sqlx::query_scalar(
        r#"
        INSERT INTO "ob-poc".onboarding_data_request_slices
            (data_request_id, discovery_snapshot_id, onboarding_request_id, cbu_id,
             srdef_id, resource_type_id, srdef_snapshot_id, parameters, owner_system,
             owner_principal_fqn, application_id, application_instance_id,
             l4_binding_required, l4_binding_status, l4_blocking_reason,
             blocking_reason)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $15)
        ON CONFLICT (data_request_id, srdef_id, parameters) DO UPDATE
        SET discovery_snapshot_id = EXCLUDED.discovery_snapshot_id,
            srdef_snapshot_id = EXCLUDED.srdef_snapshot_id,
            application_id = EXCLUDED.application_id,
            application_instance_id = EXCLUDED.application_instance_id,
            l4_binding_required = EXCLUDED.l4_binding_required,
            l4_binding_status = EXCLUDED.l4_binding_status,
            l4_blocking_reason = EXCLUDED.l4_blocking_reason,
            blocking_reason = EXCLUDED.blocking_reason
        RETURNING slice_id
        "#,
    )
    .bind(data_request_id)
    .bind(discovery_snapshot_id)
    .bind(onboarding_request_id)
    .bind(cbu_id)
    .bind(&discovery.srdef_id)
    .bind(discovery.resource_type_id)
    .bind(discovery.srdef_snapshot_id)
    .bind(&discovery.parameters)
    .bind(&discovery.owner_system)
    .bind(&discovery.owner_principal_fqn)
    .bind(l4.application_id)
    .bind(l4.application_instance_id)
    .bind(l4.required)
    .bind(l4.status)
    .bind(l4.blocking_reason)
    .fetch_one(&mut **tx)
    .await
    .context("insert onboarding data-request slice")
}

#[derive(Debug)]
struct L4BindingResolution {
    required: bool,
    status: &'static str,
    application_id: Option<Uuid>,
    application_instance_id: Option<Uuid>,
    blocking_reason: Option<String>,
}

async fn resolve_l4_binding(
    tx: &mut Transaction<'_, Postgres>,
    discovery: &DiscoveryRow,
) -> Result<L4BindingResolution> {
    let requires_l4 = discovery.l4_binding_required
        && discovery
            .provisioning_strategy
            .as_deref()
            .is_some_and(|strategy| strategy == "request");
    if !requires_l4 {
        return Ok(L4BindingResolution {
            required: false,
            status: "not_required",
            application_id: discovery.bound_application_id,
            application_instance_id: discovery.bound_application_instance_id,
            blocking_reason: None,
        });
    }

    let Some(intent_id) = first_triggered_intent_id(&discovery.triggered_by_intents) else {
        return Ok(L4BindingResolution {
            required: true,
            status: "missing_live_binding",
            application_id: discovery.bound_application_id,
            application_instance_id: discovery.bound_application_instance_id,
            blocking_reason: Some(
                "L4 capability binding required but no triggering service intent was frozen"
                    .to_string(),
            ),
        });
    };

    let service_id: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT service_id FROM "ob-poc".service_intents WHERE intent_id = $1"#,
    )
    .bind(intent_id)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(service_id) = service_id else {
        return Ok(L4BindingResolution {
            required: true,
            status: "missing_live_binding",
            application_id: discovery.bound_application_id,
            application_instance_id: discovery.bound_application_instance_id,
            blocking_reason: Some(
                "L4 capability binding required but triggering service intent was not found"
                    .to_string(),
            ),
        });
    };

    let row = sqlx::query(
        r#"
        SELECT ai.application_id, ai.id AS application_instance_id
        FROM "ob-poc".capability_bindings cb
        JOIN "ob-poc".application_instances ai ON ai.id = cb.application_instance_id
        WHERE cb.service_id = $1
          AND cb.binding_status = 'LIVE'
          AND ai.lifecycle_status = 'ACTIVE'
          AND ($2::uuid IS NULL OR ai.application_id = $2)
          AND ($3::uuid IS NULL OR ai.id = $3)
        ORDER BY cb.promoted_live_at DESC NULLS LAST, cb.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(service_id)
    .bind(discovery.bound_application_id)
    .bind(discovery.bound_application_instance_id)
    .fetch_optional(&mut **tx)
    .await?;

    if let Some(row) = row {
        Ok(L4BindingResolution {
            required: true,
            status: "resolved",
            application_id: row.get("application_id"),
            application_instance_id: row.get("application_instance_id"),
            blocking_reason: None,
        })
    } else {
        Ok(L4BindingResolution {
            required: true,
            status: "missing_live_binding",
            application_id: discovery.bound_application_id,
            application_instance_id: discovery.bound_application_instance_id,
            blocking_reason: Some(format!(
                "L4 capability binding required but no LIVE binding on ACTIVE application instance exists for service {service_id}"
            )),
        })
    }
}

async fn ensure_resource_owner_principal(
    tx: &mut Transaction<'_, Postgres>,
    discovery: &DiscoveryRow,
) -> Result<()> {
    let Some(owner_system) = discovery.owner_system.as_deref() else {
        return Ok(());
    };
    let owner_principal_fqn = discovery
        .owner_principal_fqn
        .clone()
        .unwrap_or_else(|| format!("resource_owner:{owner_system}"));
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".resource_owner_principals
            (owner_principal_fqn, owner_system, display_name)
        VALUES ($1, $2, $2)
        ON CONFLICT (owner_principal_fqn) DO UPDATE
        SET owner_system = EXCLUDED.owner_system,
            display_name = COALESCE("ob-poc".resource_owner_principals.display_name, EXCLUDED.display_name),
            updated_at = now()
        "#,
    )
    .bind(owner_principal_fqn)
    .bind(owner_system)
    .execute(&mut **tx)
    .await
    .context("ensure resource owner principal")?;
    Ok(())
}

async fn insert_slice_attrs(
    tx: &mut Transaction<'_, Postgres>,
    slice_id: Uuid,
    cbu_id: Uuid,
    discovery: &DiscoveryRow,
) -> Result<u64> {
    let rows = sqlx::query(
        r#"
        SELECT
            rar.attribute_id AS attr_id,
            ar.id AS attr_code,
            COALESCE(rar.requirement_type, 'required') AS requirement_strength,
            rar.condition_expression,
            COALESCE(rar.source_policy, '[]'::jsonb) AS source_policy,
            COALESCE(rar.evidence_policy, '{}'::jsonb) AS evidence_policy,
            COALESCE(rar.constraints, '{}'::jsonb) AS merged_constraints,
            rar.default_value,
            cv.value AS value,
            cv.evidence_refs,
            cv.as_of AS value_observed_at
        FROM "ob-poc".resource_attribute_requirements rar
        JOIN "ob-poc".service_resource_types srt ON srt.resource_id = rar.resource_id
        JOIN "ob-poc".attribute_registry ar ON ar.uuid = rar.attribute_id
        LEFT JOIN "ob-poc".cbu_attr_values cv
          ON cv.cbu_id = $2 AND cv.attr_id = rar.attribute_id
        WHERE srt.srdef_id = $1
        ORDER BY rar.display_order, ar.id
        "#,
    )
    .bind(&discovery.srdef_id)
    .bind(cbu_id)
    .fetch_all(&mut **tx)
    .await?;

    let requirements: Vec<AttrRequirementRow> = rows
        .into_iter()
        .map(|row| AttrRequirementRow {
            attr_id: row.get("attr_id"),
            attr_code: row.get("attr_code"),
            requirement_strength: row.get("requirement_strength"),
            condition_expression: row.get("condition_expression"),
            source_policy: row.get("source_policy"),
            evidence_policy: row.get("evidence_policy"),
            merged_constraints: row.get("merged_constraints"),
            default_value: row
                .get::<Option<String>, _>("default_value")
                .and_then(|raw| serde_json::from_str(&raw).ok()),
            raw_default_value: row.get("default_value"),
            value: row.get("value"),
            evidence_refs: row.get("evidence_refs"),
            value_observed_at: row.get("value_observed_at"),
        })
        .collect();

    let values_by_code: HashMap<String, Value> = requirements
        .iter()
        .filter_map(|row| {
            let code = row.attr_code.as_ref()?;
            let value = row.value.as_ref().or(row.default_value.as_ref())?;
            Some((code.clone(), value.clone()))
        })
        .collect();

    let mut count = 0_u64;
    for row in requirements {
        let condition = evaluate_condition(row.condition_expression.as_deref(), &values_by_code);
        let effective_value = row.value.clone().or_else(|| row.default_value.clone());
        let applies = match condition {
            ConditionEvaluation::Unconditional
            | ConditionEvaluation::Satisfied
            | ConditionEvaluation::Pending => true,
            ConditionEvaluation::NotApplicable => false,
        };
        let value_status = if !applies {
            "not_applicable"
        } else if effective_value.is_some() {
            "present"
        } else {
            "missing"
        };
        let condition_status = condition.status();
        let (constraint_status, constraint_reason) =
            evaluate_constraints(&row.merged_constraints, effective_value.as_ref());
        let (evidence_status, evidence_reason) =
            evaluate_evidence(&row.evidence_policy, row.evidence_refs.as_ref(), applies);
        let blocking_reason = attr_blocking_reason(
            &row.requirement_strength,
            value_status,
            condition,
            constraint_reason,
            evidence_reason,
        );

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".onboarding_data_request_attrs
                (slice_id, attr_id, attr_code, requirement_strength, condition_expression,
                 condition_status, source_policy, evidence_policy, merged_constraints,
                 default_value, value_status, value_ref, value_observed_at, blocking_reason,
                 constraint_status, evidence_status, evaluation_detail)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::text::jsonb,
                    $11, $12, $13, $14, $15, $16, $17)
            ON CONFLICT (slice_id, attr_id) DO UPDATE
            SET requirement_strength = EXCLUDED.requirement_strength,
                condition_expression = EXCLUDED.condition_expression,
                condition_status = EXCLUDED.condition_status,
                source_policy = EXCLUDED.source_policy,
                evidence_policy = EXCLUDED.evidence_policy,
                merged_constraints = EXCLUDED.merged_constraints,
                default_value = EXCLUDED.default_value,
                value_status = EXCLUDED.value_status,
                value_ref = EXCLUDED.value_ref,
                value_observed_at = EXCLUDED.value_observed_at,
                blocking_reason = EXCLUDED.blocking_reason,
                constraint_status = EXCLUDED.constraint_status,
                evidence_status = EXCLUDED.evidence_status,
                evaluation_detail = EXCLUDED.evaluation_detail,
                updated_at = now()
            "#,
        )
        .bind(slice_id)
        .bind(row.attr_id)
        .bind(&row.attr_code)
        .bind(&row.requirement_strength)
        .bind(&row.condition_expression)
        .bind(condition_status)
        .bind(&row.source_policy)
        .bind(&row.evidence_policy)
        .bind(&row.merged_constraints)
        .bind(&row.raw_default_value)
        .bind(value_status)
        .bind(effective_value.map(|v| {
            json!({
                "source": if row.value.is_some() { "cbu_attr_values" } else { "default_value" },
                "value": v
            })
        }))
        .bind(row.value_observed_at)
        .bind(blocking_reason)
        .bind(constraint_status)
        .bind(evidence_status)
        .bind(json!({
            "condition": condition_status,
            "applies": applies,
        }))
        .execute(&mut **tx)
        .await?;
        count += 1;
    }

    Ok(count)
}

#[derive(Debug)]
struct AttrRequirementRow {
    attr_id: Uuid,
    attr_code: Option<String>,
    requirement_strength: String,
    condition_expression: Option<String>,
    source_policy: Value,
    evidence_policy: Value,
    merged_constraints: Value,
    default_value: Option<Value>,
    raw_default_value: Option<String>,
    value: Option<Value>,
    evidence_refs: Option<Value>,
    value_observed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy)]
enum ConditionEvaluation {
    Unconditional,
    Pending,
    Satisfied,
    NotApplicable,
}

impl ConditionEvaluation {
    fn status(self) -> &'static str {
        match self {
            ConditionEvaluation::Unconditional => "unconditional",
            ConditionEvaluation::Pending => "pending",
            ConditionEvaluation::Satisfied => "satisfied",
            ConditionEvaluation::NotApplicable => "not_applicable",
        }
    }
}

fn evaluate_condition(
    condition_expression: Option<&str>,
    values_by_code: &HashMap<String, Value>,
) -> ConditionEvaluation {
    let Some(expression) = condition_expression else {
        return ConditionEvaluation::Unconditional;
    };
    let Some(captures) = SIMPLE_CONDITION.captures(expression) else {
        return ConditionEvaluation::Pending;
    };
    let attr_code = captures.get(1).map(|m| m.as_str()).unwrap_or_default();
    let operator = captures.get(2).map(|m| m.as_str()).unwrap_or_default();
    let expected = captures.get(3).map(|m| m.as_str()).unwrap_or_default();
    let Some(actual) = values_by_code.get(attr_code) else {
        return ConditionEvaluation::Pending;
    };
    let actual = actual
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| actual.to_string().trim_matches('"').to_string());
    let matched = match operator {
        "==" => actual == expected,
        "!=" => actual != expected,
        _ => return ConditionEvaluation::Pending,
    };
    if matched {
        ConditionEvaluation::Satisfied
    } else {
        ConditionEvaluation::NotApplicable
    }
}

fn evaluate_constraints(
    constraints: &Value,
    value: Option<&Value>,
) -> (&'static str, Option<String>) {
    let Some(value) = value else {
        return ("not_evaluated", None);
    };
    if constraints
        .as_object()
        .is_none_or(serde_json::Map::is_empty)
    {
        return ("valid", None);
    }
    if let Some(expected_type) = constraints.get("type").and_then(Value::as_str) {
        let type_matches = match expected_type {
            "array" => value.is_array(),
            "boolean" => value.is_boolean(),
            "integer" => value.as_i64().is_some(),
            "number" => value.as_f64().is_some(),
            "string" => value.as_str().is_some(),
            _ => true,
        };
        if !type_matches {
            return (
                "invalid",
                Some(format!(
                    "value does not match required type {expected_type}"
                )),
            );
        }
    }
    if let Some(allowed) = constraints.get("enum").and_then(Value::as_array) {
        if !allowed.iter().any(|candidate| candidate == value) {
            return ("invalid", Some("value is outside allowed enum".to_string()));
        }
    }
    if let (Some(min_items), Some(items)) = (
        constraints.get("min_items").and_then(Value::as_u64),
        value.as_array(),
    ) {
        if items.len() < min_items as usize {
            return (
                "invalid",
                Some(format!("array has fewer than {min_items} items")),
            );
        }
    }
    if let (Some(pattern), Some(text)) = (
        constraints.get("pattern").and_then(Value::as_str),
        value.as_str(),
    ) {
        match Regex::new(pattern) {
            Ok(regex) if !regex.is_match(text) => {
                return (
                    "invalid",
                    Some("value does not match required pattern".to_string()),
                );
            }
            Err(error) => {
                return (
                    "invalid",
                    Some(format!("invalid constraint pattern: {error}")),
                )
            }
            Ok(_) => {}
        }
    }
    if let (Some(minimum), Some(number)) = (
        constraints.get("minimum").and_then(Value::as_f64),
        value.as_f64(),
    ) {
        if number < minimum {
            return (
                "invalid",
                Some(format!("number is below minimum {minimum}")),
            );
        }
    }
    if let (Some(maximum), Some(number)) = (
        constraints.get("maximum").and_then(Value::as_f64),
        value.as_f64(),
    ) {
        if number > maximum {
            return (
                "invalid",
                Some(format!("number is above maximum {maximum}")),
            );
        }
    }
    ("valid", None)
}

fn evaluate_evidence(
    evidence_policy: &Value,
    evidence_refs: Option<&Value>,
    applies: bool,
) -> (&'static str, Option<String>) {
    if !applies {
        return ("not_required", None);
    }
    let requires_document = evidence_policy
        .get("requires_document")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !requires_document {
        return ("not_required", None);
    }
    let provided = evidence_refs.is_some_and(|refs| match refs {
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
        Value::Null => false,
        _ => true,
    });
    if provided {
        ("provided", None)
    } else {
        (
            "required_missing",
            Some("required evidence is missing".to_string()),
        )
    }
}

fn attr_blocking_reason(
    requirement_strength: &str,
    value_status: &str,
    condition: ConditionEvaluation,
    constraint_reason: Option<String>,
    evidence_reason: Option<String>,
) -> Option<String> {
    if matches!(condition, ConditionEvaluation::Pending) {
        return Some("conditional requirement could not be evaluated".to_string());
    }
    if let Some(reason) = constraint_reason {
        return Some(reason);
    }
    if let Some(reason) = evidence_reason {
        return Some(reason);
    }
    let required_now = requirement_strength == "required"
        || (requirement_strength == "conditional"
            && matches!(condition, ConditionEvaluation::Satisfied));
    if required_now && value_status == "missing" {
        Some("required attribute value missing".to_string())
    } else {
        None
    }
}

async fn recompute_slice_status(
    tx: &mut Transaction<'_, Postgres>,
    slice_id: Uuid,
) -> Result<String> {
    let attr_blockers: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM "ob-poc".onboarding_data_request_attrs
        WHERE slice_id = $1
          AND blocking_reason IS NOT NULL
        "#,
    )
    .bind(slice_id)
    .fetch_one(&mut **tx)
    .await?;

    let l4_blocking_reason: Option<String> = sqlx::query_scalar(
        r#"
        SELECT l4_blocking_reason
        FROM "ob-poc".onboarding_data_request_slices
        WHERE slice_id = $1
        "#,
    )
    .bind(slice_id)
    .fetch_one(&mut **tx)
    .await?;

    let status = if l4_blocking_reason.is_some() {
        "blocked"
    } else if attr_blockers == 0 {
        "ready"
    } else {
        "collecting"
    };
    sqlx::query(
        r#"
        UPDATE "ob-poc".onboarding_data_request_slices
        SET slice_status = CASE
                WHEN slice_status IN ('activated', 'failed', 'cancelled', 'dispatched', 'awaiting_owner')
                    THEN slice_status
                ELSE $1
            END,
            ready_at = CASE WHEN $1 = 'ready' THEN COALESCE(ready_at, now()) ELSE ready_at END,
            blocking_reason = CASE
                WHEN $1 = 'ready' THEN NULL
                WHEN $2::text IS NOT NULL THEN $2
                ELSE 'required attributes missing'
            END,
            updated_at = now()
        WHERE slice_id = $3
        "#,
    )
    .bind(status)
    .bind(l4_blocking_reason)
    .bind(slice_id)
    .execute(&mut **tx)
    .await?;
    Ok(status.to_string())
}

async fn recompute_request_status(
    tx: &mut Transaction<'_, Postgres>,
    data_request_id: Uuid,
) -> Result<String> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) AS total,
            COUNT(*) FILTER (WHERE slice_status = 'activated') AS activated,
            COUNT(*) FILTER (WHERE slice_status = 'failed') AS failed,
            COUNT(*) FILTER (WHERE slice_status = 'cancelled') AS cancelled,
            COUNT(*) FILTER (WHERE slice_status IN ('dispatched', 'awaiting_owner')) AS waiting,
            COUNT(*) FILTER (WHERE slice_status = 'ready') AS ready
        FROM "ob-poc".onboarding_data_request_slices
        WHERE data_request_id = $1
        "#,
    )
    .bind(data_request_id)
    .fetch_one(&mut **tx)
    .await?;
    let total = row.get::<i64, _>("total");
    let activated = row.get::<i64, _>("activated");
    let failed = row.get::<i64, _>("failed");
    let cancelled = row.get::<i64, _>("cancelled");
    let waiting = row.get::<i64, _>("waiting");
    let ready = row.get::<i64, _>("ready");

    let status = if total == 0 {
        "collecting"
    } else if cancelled == total {
        "cancelled"
    } else if activated == total {
        "completed"
    } else if failed > 0 {
        "blocked"
    } else if waiting > 0 {
        "awaiting_owner"
    } else if ready == total {
        "ready_for_dispatch"
    } else {
        "collecting"
    };

    sqlx::query(
        r#"
        UPDATE "ob-poc".onboarding_data_requests
        SET request_status = $1,
            completed_at = CASE WHEN $1 = 'completed' THEN COALESCE(completed_at, now()) ELSE completed_at END,
            cancelled_at = CASE WHEN $1 = 'cancelled' THEN COALESCE(cancelled_at, now()) ELSE cancelled_at END,
            updated_at = now()
        WHERE data_request_id = $2
        "#,
    )
    .bind(status)
    .bind(data_request_id)
    .execute(&mut **tx)
    .await?;

    Ok(status.to_string())
}

async fn load_ready_slices(
    tx: &mut Transaction<'_, Postgres>,
    data_request_id: Uuid,
) -> Result<Vec<SliceRow>> {
    let rows = sqlx::query(
        r#"
        SELECT slice_id, data_request_id, onboarding_request_id, cbu_id, srdef_id,
               resource_type_id, parameters, s.owner_system, s.owner_principal_fqn,
               rop.dispatch_endpoint,
               COALESCE(rop.status = 'active' AND rop.dispatch_enabled, TRUE) AS owner_dispatch_enabled
        FROM "ob-poc".onboarding_data_request_slices s
        LEFT JOIN "ob-poc".resource_owner_principals rop
          ON rop.owner_principal_fqn = s.owner_principal_fqn
        WHERE s.data_request_id = $1
          AND s.slice_status = 'ready'
          AND s.provisioning_request_id IS NULL
        ORDER BY s.srdef_id, s.parameters::text
        "#,
    )
    .bind(data_request_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| SliceRow {
            slice_id: row.get("slice_id"),
            data_request_id: row.get("data_request_id"),
            onboarding_request_id: row.get("onboarding_request_id"),
            cbu_id: row.get("cbu_id"),
            srdef_id: row.get("srdef_id"),
            resource_type_id: row.get("resource_type_id"),
            parameters: row.get("parameters"),
            owner_system: row.get("owner_system"),
            owner_principal_fqn: row.get("owner_principal_fqn"),
            dispatch_endpoint: row.get("dispatch_endpoint"),
            owner_dispatch_enabled: row.get("owner_dispatch_enabled"),
        })
        .collect())
}

async fn ensure_resource_instance(
    tx: &mut Transaction<'_, Postgres>,
    slice: &SliceRow,
) -> Result<Uuid> {
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT instance_id
        FROM "ob-poc".cbu_resource_instances
        WHERE cbu_id = $1
          AND srdef_id = $2
          AND COALESCE(instance_config->'parameters', '{}'::jsonb) = $3
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(slice.cbu_id)
    .bind(&slice.srdef_id)
    .bind(&slice.parameters)
    .fetch_optional(&mut **tx)
    .await?
    {
        return Ok(existing);
    }

    let instance_id = Uuid::now_v7();
    let instance_url = format!("urn:ob-poc:instance:{instance_id}");
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".cbu_resource_instances
            (instance_id, cbu_id, resource_type_id, srdef_id, instance_url,
             instance_config, status)
        VALUES ($1, $2, $3, $4, $5, $6, 'PENDING')
        "#,
    )
    .bind(instance_id)
    .bind(slice.cbu_id)
    .bind(slice.resource_type_id)
    .bind(&slice.srdef_id)
    .bind(instance_url)
    .bind(json!({ "parameters": slice.parameters, "slice_id": slice.slice_id }))
    .execute(&mut **tx)
    .await?;

    Ok(instance_id)
}

async fn ensure_provisioning_request(
    tx: &mut Transaction<'_, Postgres>,
    slice: &SliceRow,
    instance_id: Uuid,
    dispatch_key: &str,
) -> Result<Uuid> {
    let attrs = sqlx::query(
        r#"
        SELECT attr_code, value_ref, requirement_strength
        FROM "ob-poc".onboarding_data_request_attrs
        WHERE slice_id = $1
        ORDER BY attr_code
        "#,
    )
    .bind(slice.slice_id)
    .fetch_all(&mut **tx)
    .await?;
    let attr_payload: Vec<Value> = attrs
        .into_iter()
        .map(|row| {
            json!({
                "attr_code": row.get::<Option<String>, _>("attr_code"),
                "value_ref": row.get::<Option<Value>, _>("value_ref"),
                "requirement_strength": row.get::<String, _>("requirement_strength"),
            })
        })
        .collect();

    sqlx::query_scalar(
        r#"
        INSERT INTO "ob-poc".provisioning_requests
            (cbu_id, srdef_id, instance_id, requested_by, request_payload, owner_system,
             parameters, onboarding_request_id, onboarding_data_request_id,
             onboarding_data_request_slice_id, owner_principal_fqn, dispatch_idempotency_key)
        VALUES ($1, $2, $3, 'system', $4, $5, $6, $7, $8, $9, $10, $11)
        ON CONFLICT (dispatch_idempotency_key) WHERE dispatch_idempotency_key IS NOT NULL
        DO UPDATE SET request_payload = EXCLUDED.request_payload
        RETURNING request_id
        "#,
    )
    .bind(slice.cbu_id)
    .bind(&slice.srdef_id)
    .bind(instance_id)
    .bind(json!({
        "attrs": attr_payload,
        "slice_id": slice.slice_id,
        "data_request_id": slice.data_request_id,
        "parameters": slice.parameters,
    }))
    .bind(slice.owner_system.as_deref().unwrap_or("UNKNOWN"))
    .bind(&slice.parameters)
    .bind(slice.onboarding_request_id)
    .bind(slice.data_request_id)
    .bind(slice.slice_id)
    .bind(&slice.owner_principal_fqn)
    .bind(dispatch_key)
    .fetch_one(&mut **tx)
    .await
    .context("insert provisioning request")
}

async fn insert_provisioning_event(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
    direction: &str,
    kind: &str,
    payload: Value,
    content_hash: Option<&str>,
) -> Result<bool> {
    let event_id =
        insert_provisioning_event_returning(tx, request_id, direction, kind, payload, content_hash)
            .await?;
    Ok(event_id != Uuid::nil())
}

async fn insert_provisioning_event_returning(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
    direction: &str,
    kind: &str,
    payload: Value,
    content_hash: Option<&str>,
) -> Result<Uuid> {
    if let Some(hash) = content_hash {
        if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT event_id FROM "ob-poc".provisioning_events WHERE content_hash = $1"#,
        )
        .bind(hash)
        .fetch_optional(&mut **tx)
        .await?
        {
            return Ok(existing);
        }
    }
    let event_id = Uuid::now_v7();
    sqlx::query(
        r#"
        INSERT INTO "ob-poc".provisioning_events
            (event_id, request_id, direction, kind, payload, content_hash)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(event_id)
    .bind(request_id)
    .bind(direction)
    .bind(kind)
    .bind(payload)
    .bind(content_hash)
    .execute(&mut **tx)
    .await?;
    Ok(event_id)
}

async fn insert_owner_outbox(
    tx: &mut Transaction<'_, Postgres>,
    kind: OutboxEffectKind,
    idempotency_key: &str,
    payload: Value,
) -> Result<bool> {
    let effect_kind = serde_json::to_value(kind)?
        .as_str()
        .ok_or_else(|| anyhow!("outbox effect kind did not serialize as string"))?
        .to_string();
    let result = sqlx::query(
        r#"
        INSERT INTO public.outbox
            (id, trace_id, envelope_version, effect_kind, payload, idempotency_key)
        VALUES ($1, $2, 1, $3, $4, $5)
        ON CONFLICT (idempotency_key, effect_kind) DO NOTHING
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(Uuid::now_v7())
    .bind(effect_kind)
    .bind(payload)
    .bind(idempotency_key)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() > 0)
}

async fn load_provisioning_request(
    tx: &mut Transaction<'_, Postgres>,
    request_id: Uuid,
) -> Result<ProvisioningRequestRow> {
    let row = sqlx::query(
        r#"
        SELECT onboarding_data_request_id, onboarding_data_request_slice_id, instance_id
        FROM "ob-poc".provisioning_requests
        WHERE request_id = $1
        "#,
    )
    .bind(request_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| anyhow!("provisioning request not found: {request_id}"))?;
    Ok(ProvisioningRequestRow {
        data_request_id: row.get("onboarding_data_request_id"),
        slice_id: row.get("onboarding_data_request_slice_id"),
        instance_id: row.get("instance_id"),
    })
}

fn resource_locator_from_payload(payload: &Value) -> Value {
    json!({
        "kind": payload
            .get("locator_kind")
            .and_then(Value::as_str)
            .unwrap_or("url"),
        "value": payload
            .get("resource_url")
            .or_else(|| payload.get("srid"))
            .or_else(|| payload.get("native_key"))
            .cloned()
            .unwrap_or(Value::Null),
        "identifier": payload
            .get("native_key")
            .or_else(|| payload.get("srid"))
            .cloned()
            .unwrap_or(Value::Null),
        "owner_ticket_id": payload
            .get("owner_ticket_id")
            .cloned()
            .unwrap_or(Value::Null),
    })
}

fn failure_reason(payload: &Value) -> Option<String> {
    payload
        .get("explain")
        .and_then(|explain| explain.get("message"))
        .and_then(Value::as_str)
        .or_else(|| payload.get("message").and_then(Value::as_str))
        .map(str::to_string)
}

fn first_triggered_intent_id(triggered_by_intents: &Value) -> Option<Uuid> {
    triggered_by_intents
        .as_array()
        .and_then(|intents| intents.first())
        .and_then(Value::as_str)
        .and_then(|raw| Uuid::parse_str(raw).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_simple_condition_from_defaulted_value() {
        let mut values = HashMap::new();
        values.insert("ssi_mode".to_string(), json!("standing"));

        let condition = evaluate_condition(Some("ssi_mode == 'standing'"), &values);
        assert!(matches!(condition, ConditionEvaluation::Satisfied));

        let condition = evaluate_condition(Some("ssi_mode != 'standing'"), &values);
        assert!(matches!(condition, ConditionEvaluation::NotApplicable));
    }

    #[test]
    fn validates_constraints_and_evidence_policy() {
        let (constraint_status, constraint_reason) = evaluate_constraints(
            &json!({ "type": "string", "pattern": "^[A-Z]{3}$" }),
            Some(&json!("GBP")),
        );
        assert_eq!(constraint_status, "valid");
        assert!(constraint_reason.is_none());

        let (constraint_status, constraint_reason) = evaluate_constraints(
            &json!({ "type": "string", "pattern": "^[A-Z]{3}$" }),
            Some(&json!("gbp")),
        );
        assert_eq!(constraint_status, "invalid");
        assert!(constraint_reason.is_some());

        let (evidence_status, evidence_reason) =
            evaluate_evidence(&json!({ "requires_document": true }), None, true);
        assert_eq!(evidence_status, "required_missing");
        assert!(evidence_reason.is_some());
    }

    #[test]
    fn pending_conditional_blocks_required_now() {
        let reason = attr_blocking_reason(
            "conditional",
            "missing",
            ConditionEvaluation::Pending,
            None,
            None,
        );
        assert_eq!(
            reason.as_deref(),
            Some("conditional requirement could not be evaluated")
        );
    }
}
