//! Placeholder resolution service.
//!
//! Handles creation, resolution, and querying of placeholder entities.

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::{
    CreatePlaceholderRequest, PlaceholderEntity, PlaceholderKindCount, PlaceholderResolutionResult,
    PlaceholderStatus, PlaceholderSummary, PlaceholderWithDetails, ResolvePlaceholderRequest,
};

/// Service for managing placeholder entities.
pub struct PlaceholderResolver {
    pool: PgPool,
}

impl PlaceholderResolver {
    /// Create a new resolver with the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a placeholder entity.
    ///
    /// This creates a stub entity record with placeholder_status = 'pending'.
    pub async fn create_placeholder(&self, request: CreatePlaceholderRequest) -> Result<Uuid> {
        let name = request
            .name_hint
            .unwrap_or_else(|| format!("[Placeholder: {}]", request.kind));

        let entity_id: Uuid = sqlx::query_scalar(
            r#"
            SELECT "ob-poc".create_placeholder_entity($1, $2, $3, $4)
            "#,
        )
        .bind(&request.kind)
        .bind(&name)
        .bind(request.cbu_id)
        .bind(request.description)
        .fetch_one(&self.pool)
        .await?;

        Ok(entity_id)
    }

    /// Resolve a placeholder to a real entity.
    ///
    /// This updates the placeholder status and optionally transfers role assignments.
    pub async fn resolve(
        &self,
        request: ResolvePlaceholderRequest,
    ) -> Result<PlaceholderResolutionResult> {
        let mut tx = self.pool.begin().await?;

        // Call the resolve function
        let (resolved_to, status, roles_transferred): (Uuid, String, i32) = sqlx::query_as(
            r#"
            SELECT * FROM "ob-poc".resolve_placeholder($1, $2, $3)
            "#,
        )
        .bind(request.placeholder_entity_id)
        .bind(request.resolved_entity_id)
        .bind(&request.resolved_by)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(PlaceholderResolutionResult {
            placeholder_entity_id: request.placeholder_entity_id,
            resolved_to_entity_id: resolved_to,
            status: status.parse()?,
            roles_transferred,
        })
    }

    /// Get a placeholder entity by ID.
    pub async fn get(&self, entity_id: Uuid) -> Result<Option<PlaceholderEntity>> {
        let row = sqlx::query_as::<_, PlaceholderEntityRow>(
            r#"
            SELECT
                entity_id,
                placeholder_status,
                placeholder_kind,
                placeholder_created_for,
                created_at,
                placeholder_resolved_at,
                placeholder_resolved_to,
                placeholder_resolved_by
            FROM "ob-poc".entities
            WHERE entity_id = $1
              AND placeholder_status IS NOT NULL
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.into()))
    }

    /// List pending placeholders for a CBU.
    pub async fn list_pending_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<PlaceholderWithDetails>> {
        let rows = sqlx::query_as::<_, PlaceholderWithDetailsRow>(
            r#"
            SELECT
                entity_id,
                placeholder_status,
                placeholder_kind,
                cbu_id,
                entity_name,
                cbu_name,
                kind_label,
                created_at
            FROM "ob-poc".v_pending_placeholders
            WHERE cbu_id = $1
            ORDER BY kind_label, entity_name
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// List all pending placeholders.
    pub async fn list_all_pending(&self) -> Result<Vec<PlaceholderWithDetails>> {
        let rows = sqlx::query_as::<_, PlaceholderWithDetailsRow>(
            r#"
            SELECT
                entity_id,
                placeholder_status,
                placeholder_kind,
                cbu_id,
                entity_name,
                cbu_name,
                kind_label,
                created_at
            FROM "ob-poc".v_pending_placeholders
            ORDER BY cbu_name, kind_label, entity_name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    /// Get placeholder summary for a CBU.
    pub async fn get_summary(&self, cbu_id: Uuid) -> Result<PlaceholderSummary> {
        let counts = sqlx::query_as::<_, (String, i64)>(
            r#"
            SELECT placeholder_kind, COUNT(*) as count
            FROM "ob-poc".entities
            WHERE placeholder_created_for = $1
              AND placeholder_status = 'pending'
            GROUP BY placeholder_kind
            ORDER BY placeholder_kind
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;

        let total: i64 = counts.iter().map(|(_, c)| c).sum();

        Ok(PlaceholderSummary {
            cbu_id,
            pending_count: total,
            by_kind: counts
                .into_iter()
                .map(|(kind, count)| PlaceholderKindCount { kind, count })
                .collect(),
        })
    }

    /// Check if an entity is a placeholder.
    pub async fn is_placeholder(&self, entity_id: Uuid) -> Result<bool> {
        let is_placeholder: bool = sqlx::query_scalar(
            r#"
            SELECT placeholder_status IS NOT NULL
            FROM "ob-poc".entities
            WHERE entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(false);

        Ok(is_placeholder)
    }

    /// Ensure an entity exists, creating a placeholder if not found.
    ///
    /// This is the core operation for `entity.ensure-or-placeholder`.
    pub async fn ensure_or_placeholder(
        &self,
        entity_ref: Option<Uuid>,
        kind: &str,
        cbu_id: Uuid,
        name_hint: Option<String>,
    ) -> Result<(Uuid, bool)> {
        // If we have a valid entity ref, check if it exists
        if let Some(ref_id) = entity_ref {
            let exists: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".entities WHERE entity_id = $1
                )
                "#,
            )
            .bind(ref_id)
            .fetch_one(&self.pool)
            .await?;

            if exists {
                return Ok((ref_id, false)); // Entity exists, not a placeholder
            }
        }

        // Create placeholder
        let entity_id = self
            .create_placeholder(CreatePlaceholderRequest {
                kind: kind.to_string(),
                cbu_id,
                name_hint,
                description: None,
            })
            .await?;

        Ok((entity_id, true)) // Created placeholder
    }
}

// Internal row types for sqlx mapping

#[derive(sqlx::FromRow)]
struct PlaceholderEntityRow {
    entity_id: Uuid,
    placeholder_status: Option<String>,
    placeholder_kind: Option<String>,
    placeholder_created_for: Option<Uuid>,
    created_at: chrono::DateTime<chrono::Utc>,
    placeholder_resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    placeholder_resolved_to: Option<Uuid>,
    placeholder_resolved_by: Option<String>,
}

impl From<PlaceholderEntityRow> for PlaceholderEntity {
    fn from(row: PlaceholderEntityRow) -> Self {
        PlaceholderEntity {
            entity_id: row.entity_id,
            status: row
                .placeholder_status
                .and_then(|s| s.parse().ok())
                .unwrap_or(PlaceholderStatus::Pending),
            kind: row.placeholder_kind.unwrap_or_default(),
            created_for_cbu_id: row.placeholder_created_for,
            created_at: row.created_at,
            resolved_at: row.placeholder_resolved_at,
            resolved_to_entity_id: row.placeholder_resolved_to,
            resolved_by: row.placeholder_resolved_by,
        }
    }
}

#[derive(sqlx::FromRow)]
struct PlaceholderWithDetailsRow {
    entity_id: Uuid,
    placeholder_status: String,
    placeholder_kind: String,
    cbu_id: Option<Uuid>,
    entity_name: String,
    cbu_name: Option<String>,
    kind_label: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<PlaceholderWithDetailsRow> for PlaceholderWithDetails {
    fn from(row: PlaceholderWithDetailsRow) -> Self {
        PlaceholderWithDetails {
            placeholder: PlaceholderEntity {
                entity_id: row.entity_id,
                status: row
                    .placeholder_status
                    .parse()
                    .unwrap_or(PlaceholderStatus::Pending),
                kind: row.placeholder_kind,
                created_for_cbu_id: row.cbu_id,
                created_at: row.created_at,
                resolved_at: None,
                resolved_to_entity_id: None,
                resolved_by: None,
            },
            entity_name: row.entity_name,
            cbu_name: row.cbu_name,
            kind_label: row.kind_label,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_status_parse() {
        assert_eq!(
            "pending".parse::<PlaceholderStatus>().unwrap(),
            PlaceholderStatus::Pending
        );
        assert_eq!(
            "resolved".parse::<PlaceholderStatus>().unwrap(),
            PlaceholderStatus::Resolved
        );
        assert_eq!(
            "verified".parse::<PlaceholderStatus>().unwrap(),
            PlaceholderStatus::Verified
        );
        assert!("invalid".parse::<PlaceholderStatus>().is_err());
    }

    #[test]
    fn test_placeholder_status_display() {
        assert_eq!(PlaceholderStatus::Pending.to_string(), "pending");
        assert_eq!(PlaceholderStatus::Resolved.to_string(), "resolved");
        assert_eq!(PlaceholderStatus::Verified.to_string(), "verified");
    }
}
