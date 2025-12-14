//! Entity Reference Resolution
//!
//! Resolves EntityRef (LEI, BIC, NAME, UUID) to entity_id UUID.
//! Used during trading profile materialization.

use sqlx::PgPool;
use uuid::Uuid;

use super::types::{EntityRef, EntityRefType};

/// Resolve EntityRef to entity_id UUID
///
/// Checks multiple tables based on ref_type:
/// - LEI: entity_funds, entity_manco, entity_settlement_identity
/// - BIC: entity_settlement_identity
/// - NAME: entities (fuzzy search)
/// - UUID: direct parse and verify exists
pub async fn resolve_entity_ref(
    pool: &PgPool,
    entity_ref: &EntityRef,
) -> Result<Uuid, ResolveError> {
    match entity_ref.ref_type {
        EntityRefType::Uuid => {
            // Direct UUID - just parse and verify exists
            let uuid = Uuid::parse_str(&entity_ref.value)
                .map_err(|_| ResolveError::InvalidUuid(entity_ref.value.clone()))?;
            verify_entity_exists(pool, uuid).await?;
            Ok(uuid)
        }
        EntityRefType::Lei => resolve_by_lei(pool, &entity_ref.value).await,
        EntityRefType::Bic => resolve_by_bic(pool, &entity_ref.value).await,
        EntityRefType::Name => resolve_by_name(pool, &entity_ref.value).await,
    }
}

/// Verify an entity exists by UUID
async fn verify_entity_exists(pool: &PgPool, entity_id: Uuid) -> Result<(), ResolveError> {
    let exists: Option<bool> = sqlx::query_scalar(
        r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entities WHERE entity_id = $1)"#,
    )
    .bind(entity_id)
    .fetch_optional(pool)
    .await?;

    if exists.unwrap_or(false) {
        Ok(())
    } else {
        Err(ResolveError::NotFound {
            ref_type: "UUID".to_string(),
            value: entity_id.to_string(),
            hint: "Entity with this UUID does not exist".to_string(),
        })
    }
}

/// Resolve entity by LEI
///
/// Checks three tables in priority order:
/// 1. entity_funds (most common for fund counterparties)
/// 2. entity_manco (management companies)
/// 3. entity_settlement_identity (banks, brokers)
async fn resolve_by_lei(pool: &PgPool, lei: &str) -> Result<Uuid, ResolveError> {
    let result: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT entity_id FROM "ob-poc".entity_funds WHERE lei = $1
        UNION
        SELECT entity_id FROM "ob-poc".entity_manco WHERE lei = $1
        UNION
        SELECT entity_id FROM custody.entity_settlement_identity WHERE lei = $1
        LIMIT 1
        "#,
    )
    .bind(lei)
    .fetch_optional(pool)
    .await?;

    result.ok_or_else(|| ResolveError::NotFound {
        ref_type: "LEI".to_string(),
        value: lei.to_string(),
        hint: "Ensure entity exists in entity_funds, entity_manco, or entity_settlement_identity"
            .to_string(),
    })
}

/// Resolve entity by BIC
///
/// Checks entity_settlement_identity.primary_bic
async fn resolve_by_bic(pool: &PgPool, bic: &str) -> Result<Uuid, ResolveError> {
    let result: Option<Uuid> = sqlx::query_scalar(
        "SELECT entity_id FROM custody.entity_settlement_identity WHERE primary_bic = $1 LIMIT 1",
    )
    .bind(bic)
    .fetch_optional(pool)
    .await?;

    result.ok_or_else(|| ResolveError::NotFound {
        ref_type: "BIC".to_string(),
        value: bic.to_string(),
        hint: "Ensure entity has settlement identity with this BIC".to_string(),
    })
}

/// Resolve entity by NAME (fuzzy search)
///
/// Uses ILIKE on entities.name for fuzzy matching
async fn resolve_by_name(pool: &PgPool, name: &str) -> Result<Uuid, ResolveError> {
    let result: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT entity_id FROM "ob-poc".entities WHERE name ILIKE $1 LIMIT 1"#,
    )
    .bind(format!("%{}%", name))
    .fetch_optional(pool)
    .await?;

    result.ok_or_else(|| ResolveError::NotFound {
        ref_type: "NAME".to_string(),
        value: name.to_string(),
        hint: "Entity not found by name search".to_string(),
    })
}

/// Entity resolution error
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("Invalid UUID: {0}")]
    InvalidUuid(String),

    #[error("Entity not found: {ref_type}={value}. {hint}")]
    NotFound {
        ref_type: String,
        value: String,
        hint: String,
    },

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_error_display() {
        let err = ResolveError::NotFound {
            ref_type: "LEI".to_string(),
            value: "ABC123".to_string(),
            hint: "Check entity_funds table".to_string(),
        };
        assert!(err.to_string().contains("LEI=ABC123"));
        assert!(err.to_string().contains("Check entity_funds table"));
    }

    #[test]
    fn test_invalid_uuid_error() {
        let err = ResolveError::InvalidUuid("not-a-uuid".to_string());
        assert!(err.to_string().contains("not-a-uuid"));
    }
}
