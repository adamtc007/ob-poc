use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

/// Persisted state override entry.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, PartialEq)]
pub struct StateOverride {
    pub id: Uuid,
    pub cbu_id: Uuid,
    pub case_id: Option<Uuid>,
    pub constellation_type: String,
    pub slot_path: String,
    pub computed_state: String,
    pub override_state: String,
    pub justification: String,
    pub authority: String,
    pub conditions: Option<String>,
    pub reducer_revision: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by: Option<String>,
    pub revoke_reason: Option<String>,
}

/// Request payload for creating a state override.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateOverrideRequest {
    pub cbu_id: Uuid,
    pub case_id: Option<Uuid>,
    pub constellation_type: String,
    pub slot_path: String,
    pub computed_state: String,
    pub override_state: String,
    pub justification: String,
    pub authority: String,
    pub conditions: Option<String>,
    pub reducer_revision: String,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Get the active override for a slot, if one exists.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_reg::reducer::get_active_override;
///
/// let cbu_id = Uuid::new_v4();
/// let _ = get_active_override(pool, cbu_id, None, "entity.primary").await?;
/// # Ok(())
/// # }
/// ```
pub async fn get_active_override(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    slot_path: &str,
) -> Result<Option<StateOverride>> {
    match sqlx::query_as::<_, StateOverride>(
        r#"
        SELECT
            id, cbu_id, case_id, constellation_type, slot_path,
            computed_state, override_state, justification, authority,
            conditions, reducer_revision, created_at, expires_at,
            revoked_at, revoked_by, revoke_reason
        FROM "ob-poc".state_overrides
        WHERE cbu_id = $1
          AND COALESCE(case_id, $2::uuid) = COALESCE($3, $2::uuid)
          AND slot_path = $4
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > now())
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(cbu_id)
    .bind(Uuid::parse_str(NIL_UUID)?)
    .bind(case_id)
    .bind(slot_path)
    .fetch_optional(pool)
    .await
    {
        Ok(value) => Ok(value),
        Err(error) if is_missing_relation_error(&error, "state_overrides") => Ok(None),
        Err(error) => Err(error).context("failed to load active reducer override"),
    }
}

pub(crate) async fn get_active_override_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    slot_path: &str,
) -> Result<Option<StateOverride>> {
    match sqlx::query_as::<_, StateOverride>(
        r#"
        SELECT
            id, cbu_id, case_id, constellation_type, slot_path,
            computed_state, override_state, justification, authority,
            conditions, reducer_revision, created_at, expires_at,
            revoked_at, revoked_by, revoke_reason
        FROM "ob-poc".state_overrides
        WHERE cbu_id = $1
          AND COALESCE(case_id, $2::uuid) = COALESCE($3, $2::uuid)
          AND slot_path = $4
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > now())
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(cbu_id)
    .bind(Uuid::parse_str(NIL_UUID)?)
    .bind(case_id)
    .bind(slot_path)
    .fetch_optional(&mut **tx)
    .await
    {
        Ok(value) => Ok(value),
        Err(error) if is_missing_relation_error(&error, "state_overrides") => Ok(None),
        Err(error) => Err(error).context("failed to load active reducer override"),
    }
}

/// Create a new state override.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_reg::reducer::{create_override, CreateOverrideRequest};
///
/// let req = CreateOverrideRequest {
///     cbu_id: Uuid::new_v4(),
///     case_id: None,
///     constellation_type: String::from("entity_kyc_lifecycle"),
///     slot_path: String::from("entity.primary"),
///     computed_state: String::from("filled"),
///     override_state: String::from("approved"),
///     justification: String::from("manual steward decision"),
///     authority: String::from("compliance"),
///     conditions: None,
///     reducer_revision: String::from("0123456789abcdef"),
///     expires_at: None,
/// };
/// let _ = create_override(pool, req).await?;
/// # Ok(())
/// # }
/// ```
pub async fn create_override(pool: &PgPool, req: CreateOverrideRequest) -> Result<StateOverride> {
    let id = Uuid::new_v4();
    let mut tx = pool
        .begin()
        .await
        .context("failed to open override transaction")?;
    let override_entry = sqlx::query_as::<_, StateOverride>(
        r#"
        INSERT INTO "ob-poc".state_overrides (
            id, cbu_id, case_id, constellation_type, slot_path,
            computed_state, override_state, justification, authority,
            conditions, reducer_revision, expires_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        RETURNING
            id, cbu_id, case_id, constellation_type, slot_path,
            computed_state, override_state, justification, authority,
            conditions, reducer_revision, created_at, expires_at,
            revoked_at, revoked_by, revoke_reason
        "#,
    )
    .bind(id)
    .bind(req.cbu_id)
    .bind(req.case_id)
    .bind(&req.constellation_type)
    .bind(&req.slot_path)
    .bind(&req.computed_state)
    .bind(&req.override_state)
    .bind(&req.justification)
    .bind(&req.authority)
    .bind(&req.conditions)
    .bind(&req.reducer_revision)
    .bind(req.expires_at)
    .fetch_one(&mut *tx)
    .await
    .context("failed to create reducer override")?;

    log_case_event(
        &mut tx,
        override_entry.case_id,
        "STATE_OVERRIDE_CREATED",
        json!({
            "override_id": override_entry.id,
            "slot_path": override_entry.slot_path,
            "constellation_type": override_entry.constellation_type,
            "computed_state": override_entry.computed_state,
            "override_state": override_entry.override_state,
            "authority": override_entry.authority,
            "reducer_revision": override_entry.reducer_revision,
        }),
        "RULE_ENGINE",
        Some(format!(
            "Reducer override created: {} -> {} ({})",
            override_entry.computed_state,
            override_entry.override_state,
            override_entry.justification
        )),
    )
    .await?;

    tx.commit()
        .await
        .context("failed to commit reducer override transaction")?;

    Ok(override_entry)
}

/// Revoke an existing override.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_reg::reducer::revoke_override;
///
/// revoke_override(pool, Uuid::new_v4(), "operator", "superseded").await?;
/// # Ok(())
/// # }
/// ```
pub async fn revoke_override(
    pool: &PgPool,
    override_id: Uuid,
    revoked_by: &str,
    reason: &str,
) -> Result<()> {
    let mut tx = pool
        .begin()
        .await
        .context("failed to open override transaction")?;
    let revoked = sqlx::query_as::<_, StateOverride>(
        r#"
        UPDATE "ob-poc".state_overrides
        SET revoked_at = now(),
            revoked_by = $2,
            revoke_reason = $3
        WHERE id = $1
          AND revoked_at IS NULL
        RETURNING
            id, cbu_id, case_id, constellation_type, slot_path,
            computed_state, override_state, justification, authority,
            conditions, reducer_revision, created_at, expires_at,
            revoked_at, revoked_by, revoke_reason
        "#,
    )
    .bind(override_id)
    .bind(revoked_by)
    .bind(reason)
    .fetch_optional(&mut *tx)
    .await
    .context("failed to revoke reducer override")?;

    let Some(revoked) = revoked else {
        return Err(anyhow!(
            "no active reducer override found for id {}",
            override_id
        ));
    };

    log_case_event(
        &mut tx,
        revoked.case_id,
        "STATE_OVERRIDE_REVOKED",
        json!({
            "override_id": revoked.id,
            "slot_path": revoked.slot_path,
            "constellation_type": revoked.constellation_type,
            "computed_state": revoked.computed_state,
            "override_state": revoked.override_state,
            "revoked_by": revoked_by,
            "reason": reason,
            "reducer_revision": revoked.reducer_revision,
        }),
        "RULE_ENGINE",
        Some(format!(
            "Reducer override revoked by {}: {}",
            revoked_by, reason
        )),
    )
    .await?;

    tx.commit()
        .await
        .context("failed to commit reducer override revocation transaction")?;

    Ok(())
}

fn is_missing_relation_error(error: &sqlx::Error, relation_name: &str) -> bool {
    match error {
        sqlx::Error::Database(db_error) => {
            db_error.code().as_deref() == Some("42P01")
                && db_error.message().contains(relation_name)
        }
        _ => false,
    }
}

/// List all active overrides for a CBU.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_reg::reducer::list_active_overrides;
///
/// let _ = list_active_overrides(pool, Uuid::new_v4()).await?;
/// # Ok(())
/// # }
/// ```
pub async fn list_active_overrides(pool: &PgPool, cbu_id: Uuid) -> Result<Vec<StateOverride>> {
    sqlx::query_as::<_, StateOverride>(
        r#"
        SELECT
            id, cbu_id, case_id, constellation_type, slot_path,
            computed_state, override_state, justification, authority,
            conditions, reducer_revision, created_at, expires_at,
            revoked_at, revoked_by, revoke_reason
        FROM "ob-poc".state_overrides
        WHERE cbu_id = $1
          AND revoked_at IS NULL
          AND (expires_at IS NULL OR expires_at > now())
        ORDER BY slot_path, created_at DESC
        "#,
    )
    .bind(cbu_id)
    .fetch_all(pool)
    .await
    .context("failed to list reducer overrides")
}

async fn log_case_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    case_id: Option<Uuid>,
    event_type: &str,
    event_data: serde_json::Value,
    actor_type: &str,
    comment: Option<String>,
) -> Result<()> {
    let Some(case_id) = case_id else {
        return Ok(());
    };

    sqlx::query(
        r#"
        INSERT INTO "ob-poc".case_events (
            event_id, case_id, event_type, event_data, actor_type, comment
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(case_id)
    .bind(event_type)
    .bind(event_data)
    .bind(actor_type)
    .bind(comment)
    .execute(&mut **tx)
    .await
    .context("failed to insert reducer case event")?;

    Ok(())
}
