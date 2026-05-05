//! Resource-owner dispatch consumers.
//!
//! These consumers close the dispatch/delivery split for service-resource
//! onboarding data requests. The dispatch consumer records `DISPATCHED`
//! only after the outbox row is drained, so a slice cannot reach
//! `awaiting_owner` merely because the request row was prepared.

use async_trait::async_trait;
use ob_poc_types::{ClaimedOutboxRow, OutboxEffectKind, OutboxProcessOutcome};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::consumer::AsyncOutboxConsumer;

#[derive(Debug, Deserialize)]
struct ResourceOwnerPayload {
    provisioning_request_id: Uuid,
    #[serde(default)]
    slice_id: Option<Uuid>,
    #[serde(default)]
    data_request_id: Option<Uuid>,
}

/// Consumer for `resource_owner_dispatch` outbox rows.
pub struct ResourceOwnerDispatchConsumer {
    pool: PgPool,
}

impl ResourceOwnerDispatchConsumer {
    /// Create a dispatch consumer backed by a Postgres pool.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let consumer = ResourceOwnerDispatchConsumer::new(pool.clone());
    /// ```
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AsyncOutboxConsumer for ResourceOwnerDispatchConsumer {
    fn effect_kind(&self) -> OutboxEffectKind {
        OutboxEffectKind::ResourceOwnerDispatch
    }

    fn label(&self) -> &str {
        "resource-owner-dispatch-v1"
    }

    async fn process(&self, row: ClaimedOutboxRow) -> OutboxProcessOutcome {
        let payload: ResourceOwnerPayload = match serde_json::from_value(row.payload) {
            Ok(payload) => payload,
            Err(error) => {
                return OutboxProcessOutcome::Terminal {
                    reason: format!("malformed resource_owner_dispatch payload: {error}"),
                };
            }
        };

        match mark_dispatched(&self.pool, payload).await {
            Ok(true) => OutboxProcessOutcome::Done,
            Ok(false) => OutboxProcessOutcome::Deduped,
            Err(error) => OutboxProcessOutcome::Retryable {
                reason: error.to_string(),
            },
        }
    }
}

/// Consumer for `resource_owner_stand_down` outbox rows.
pub struct ResourceOwnerStandDownConsumer {
    pool: PgPool,
}

impl ResourceOwnerStandDownConsumer {
    /// Create a stand-down consumer backed by a Postgres pool.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let consumer = ResourceOwnerStandDownConsumer::new(pool.clone());
    /// ```
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AsyncOutboxConsumer for ResourceOwnerStandDownConsumer {
    fn effect_kind(&self) -> OutboxEffectKind {
        OutboxEffectKind::ResourceOwnerStandDown
    }

    fn label(&self) -> &str {
        "resource-owner-stand-down-v1"
    }

    async fn process(&self, row: ClaimedOutboxRow) -> OutboxProcessOutcome {
        let payload: ResourceOwnerPayload = match serde_json::from_value(row.payload) {
            Ok(payload) => payload,
            Err(error) => {
                return OutboxProcessOutcome::Terminal {
                    reason: format!("malformed resource_owner_stand_down payload: {error}"),
                };
            }
        };

        match mark_stand_down(&self.pool, payload).await {
            Ok(true) => OutboxProcessOutcome::Done,
            Ok(false) => OutboxProcessOutcome::Deduped,
            Err(error) => OutboxProcessOutcome::Retryable {
                reason: error.to_string(),
            },
        }
    }
}

async fn mark_dispatched(pool: &PgPool, payload: ResourceOwnerPayload) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    let hash = format!("dispatched:{}", payload.provisioning_request_id);
    let existing: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT event_id FROM "ob-poc".provisioning_events WHERE content_hash = $1"#,
    )
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await?;
    if existing.is_some() {
        tx.commit().await?;
        return Ok(false);
    }

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".provisioning_events
            (event_id, request_id, direction, kind, payload, content_hash)
        VALUES (uuidv7(), $1, 'OUT', 'DISPATCHED', $2, $3)
        "#,
    )
    .bind(payload.provisioning_request_id)
    .bind(serde_json::json!({
        "slice_id": payload.slice_id,
        "data_request_id": payload.data_request_id,
    }))
    .bind(&hash)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE "ob-poc".provisioning_requests
        SET status = 'sent'
        WHERE request_id = $1
          AND status IN ('queued', 'ack')
        "#,
    )
    .bind(payload.provisioning_request_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE "ob-poc".cbu_resource_instances inst
        SET status = 'AWAITING_OWNER',
            updated_at = now()
        FROM "ob-poc".provisioning_requests pr
        WHERE pr.request_id = $1
          AND inst.instance_id = pr.instance_id
          AND inst.status IN ('PENDING', 'PROVISIONING')
        "#,
    )
    .bind(payload.provisioning_request_id)
    .execute(&mut *tx)
    .await?;

    if let Some(slice_id) = payload.slice_id {
        sqlx::query(
            r#"
            UPDATE "ob-poc".onboarding_data_request_slices
            SET slice_status = 'awaiting_owner',
                updated_at = now()
            WHERE slice_id = $1
              AND slice_status = 'dispatched'
            "#,
        )
        .bind(slice_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(true)
}

async fn mark_stand_down(pool: &PgPool, payload: ResourceOwnerPayload) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    let hash = format!("stand-down-delivered:{}", payload.provisioning_request_id);
    let existing: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT event_id FROM "ob-poc".provisioning_events WHERE content_hash = $1"#,
    )
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await?;
    if existing.is_some() {
        tx.commit().await?;
        return Ok(false);
    }

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".provisioning_events
            (event_id, request_id, direction, kind, payload, content_hash)
        VALUES (uuidv7(), $1, 'OUT', 'STAND_DOWN', $2, $3)
        "#,
    )
    .bind(payload.provisioning_request_id)
    .bind(serde_json::json!({
        "slice_id": payload.slice_id,
        "data_request_id": payload.data_request_id,
        "delivered": true,
    }))
    .bind(&hash)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        UPDATE "ob-poc".provisioning_requests
        SET status = 'cancelled'
        WHERE request_id = $1
        "#,
    )
    .bind(payload.provisioning_request_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(true)
}
