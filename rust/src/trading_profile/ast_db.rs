//! Database operations for TradingMatrixDocument (tree-based AST)
//!
//! This module provides database operations for loading and saving the tree-based
//! TradingMatrixDocument. The document IS the AST - stored as JSONB in the database.
//!
//! ## Design Philosophy
//!
//! The document is the single source of truth. No materialization to operational
//! tables is needed - the document structure directly serves the UI.

use anyhow::Result;
use ob_poc_types::trading_matrix::{DocumentStatus, TradingMatrixDocument};
use sqlx::PgPool;
use uuid::Uuid;

/// Errors for AST database operations
#[derive(Debug, thiserror::Error)]
pub enum AstDbError {
    #[error("Profile not found: {0}")]
    ProfileNotFound(Uuid),

    #[error("CBU not found: {0}")]
    CbuNotFound(Uuid),

    #[error("Profile is not in DRAFT status, cannot modify")]
    NotDraft,

    #[error("Profile already exists for CBU")]
    AlreadyExists,

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

// ============================================================================
// LOAD OPERATIONS
// ============================================================================

/// Load a TradingMatrixDocument by profile ID
pub async fn load_document(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<TradingMatrixDocument, AstDbError> {
    let row = sqlx::query!(
        r#"SELECT document FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        profile_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AstDbError::ProfileNotFound(profile_id))?;

    let doc: TradingMatrixDocument = serde_json::from_value(row.document)?;
    Ok(doc)
}

/// Load the active TradingMatrixDocument for a CBU
pub async fn load_active_document(
    pool: &PgPool,
    cbu_id: Uuid,
) -> Result<Option<(Uuid, TradingMatrixDocument)>, AstDbError> {
    let row = sqlx::query!(
        r#"
        SELECT profile_id, document
        FROM "ob-poc".cbu_trading_profiles
        WHERE cbu_id = $1 AND status = 'ACTIVE'
        ORDER BY version DESC
        LIMIT 1
        "#,
        cbu_id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let doc: TradingMatrixDocument = serde_json::from_value(r.document)?;
            Ok(Some((r.profile_id, doc)))
        }
        None => Ok(None),
    }
}

/// Load the working (DRAFT, VALIDATED, or PENDING_REVIEW) document for a CBU
pub async fn load_working_document(
    pool: &PgPool,
    cbu_id: Uuid,
) -> Result<Option<(Uuid, TradingMatrixDocument)>, AstDbError> {
    let row = sqlx::query!(
        r#"
        SELECT profile_id, document
        FROM "ob-poc".cbu_trading_profiles
        WHERE cbu_id = $1 AND status IN ('DRAFT', 'VALIDATED', 'PENDING_REVIEW')
        ORDER BY version DESC
        LIMIT 1
        "#,
        cbu_id
    )
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let doc: TradingMatrixDocument = serde_json::from_value(r.document)?;
            Ok(Some((r.profile_id, doc)))
        }
        None => Ok(None),
    }
}

/// Get profile status
pub async fn get_profile_status(pool: &PgPool, profile_id: Uuid) -> Result<String, AstDbError> {
    let row = sqlx::query!(
        r#"SELECT status FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        profile_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AstDbError::ProfileNotFound(profile_id))?;

    Ok(row.status)
}

/// Get CBU name for creating a new document
pub async fn get_cbu_name(pool: &PgPool, cbu_id: Uuid) -> Result<String, AstDbError> {
    let row = sqlx::query!(
        r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        cbu_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AstDbError::CbuNotFound(cbu_id))?;

    Ok(row.name)
}

// ============================================================================
// SAVE OPERATIONS
// ============================================================================

/// Compute SHA256 hash of document for change detection
fn compute_document_hash(doc: &TradingMatrixDocument) -> String {
    use sha2::{Digest, Sha256};
    let json = serde_json::to_string(doc).unwrap_or_default();
    let hash = Sha256::digest(json.as_bytes());
    format!("{:x}", hash)
}

/// Save a TradingMatrixDocument (update existing)
pub async fn save_document(
    pool: &PgPool,
    profile_id: Uuid,
    doc: &TradingMatrixDocument,
) -> Result<(), AstDbError> {
    let doc_json = serde_json::to_value(doc)?;
    let hash = compute_document_hash(doc);

    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET document = $2,
            document_hash = $3
        WHERE profile_id = $1
        "#,
        profile_id,
        doc_json,
        hash
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Ensure profile is in DRAFT status before modifying
pub async fn ensure_draft(pool: &PgPool, profile_id: Uuid) -> Result<(), AstDbError> {
    let status = get_profile_status(pool, profile_id).await?;
    if status != "DRAFT" {
        return Err(AstDbError::NotDraft);
    }
    Ok(())
}

// ============================================================================
// CREATE OPERATIONS
// ============================================================================

/// Create a new draft TradingMatrixDocument for a CBU
pub async fn create_draft(
    pool: &PgPool,
    cbu_id: Uuid,
    notes: Option<String>,
) -> Result<(Uuid, TradingMatrixDocument), AstDbError> {
    // Check for existing working version
    if load_working_document(pool, cbu_id).await?.is_some() {
        return Err(AstDbError::AlreadyExists);
    }

    // Get CBU name
    let cbu_name = get_cbu_name(pool, cbu_id).await?;

    // Create the document
    let mut doc = super::ast_builder::create_document(&cbu_id.to_string(), &cbu_name);
    super::ast_builder::initialize_categories(&mut doc);

    // Get next version number
    let version: i32 = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(MAX(version), 0) + 1
        FROM "ob-poc".cbu_trading_profiles
        WHERE cbu_id = $1
        "#,
        cbu_id
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(1);

    doc.version = version;

    let doc_json = serde_json::to_value(&doc)?;
    let hash = compute_document_hash(&doc);
    let profile_id = Uuid::now_v7();

    sqlx::query!(
        r#"
        INSERT INTO "ob-poc".cbu_trading_profiles
            (profile_id, cbu_id, version, status, document, document_hash, notes)
        VALUES ($1, $2, $3, 'DRAFT', $4, $5, $6)
        "#,
        profile_id,
        cbu_id,
        version,
        doc_json,
        hash,
        notes,
    )
    .execute(pool)
    .await?;

    Ok((profile_id, doc))
}

/// Create a new draft by cloning from an existing profile
pub async fn clone_to_draft(
    pool: &PgPool,
    source_profile_id: Uuid,
    target_cbu_id: Uuid,
    notes: Option<String>,
) -> Result<(Uuid, TradingMatrixDocument), AstDbError> {
    // Check for existing working version
    if load_working_document(pool, target_cbu_id).await?.is_some() {
        return Err(AstDbError::AlreadyExists);
    }

    // Load source document
    let mut doc = load_document(pool, source_profile_id).await?;

    // Get target CBU name
    let cbu_name = get_cbu_name(pool, target_cbu_id).await?;

    // Update document for new CBU
    doc.cbu_id = target_cbu_id.to_string();
    doc.cbu_name = cbu_name;
    doc.status = DocumentStatus::Draft;
    doc.created_at = Some(chrono::Utc::now().to_rfc3339());
    doc.updated_at = doc.created_at.clone();

    // Get next version number
    let version: i32 = sqlx::query_scalar!(
        r#"
        SELECT COALESCE(MAX(version), 0) + 1
        FROM "ob-poc".cbu_trading_profiles
        WHERE cbu_id = $1
        "#,
        target_cbu_id
    )
    .fetch_one(pool)
    .await?
    .unwrap_or(1);

    doc.version = version;

    let doc_json = serde_json::to_value(&doc)?;
    let hash = compute_document_hash(&doc);
    let profile_id = Uuid::now_v7();

    sqlx::query!(
        r#"
        INSERT INTO "ob-poc".cbu_trading_profiles
            (profile_id, cbu_id, version, status, document, document_hash, notes)
        VALUES ($1, $2, $3, 'DRAFT', $4, $5, $6)
        "#,
        profile_id,
        target_cbu_id,
        version,
        doc_json,
        hash,
        notes,
    )
    .execute(pool)
    .await?;

    Ok((profile_id, doc))
}

// ============================================================================
// STATUS TRANSITIONS
// ============================================================================

/// Mark a profile as validated (DRAFT -> VALIDATED)
pub async fn mark_validated(pool: &PgPool, profile_id: Uuid) -> Result<(), AstDbError> {
    let status = get_profile_status(pool, profile_id).await?;
    if status != "DRAFT" {
        return Err(AstDbError::NotDraft);
    }

    // Update document status
    let mut doc = load_document(pool, profile_id).await?;
    doc.status = DocumentStatus::Validated;
    doc.updated_at = Some(chrono::Utc::now().to_rfc3339());

    let doc_json = serde_json::to_value(&doc)?;
    let hash = compute_document_hash(&doc);

    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET status = 'VALIDATED',
            document = $2,
            document_hash = $3
        WHERE profile_id = $1
        "#,
        profile_id,
        doc_json,
        hash
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Activate a profile (VALIDATED -> ACTIVE, supersedes previous ACTIVE)
pub async fn activate_profile(
    pool: &PgPool,
    profile_id: Uuid,
    activated_by: Option<String>,
) -> Result<(), AstDbError> {
    let status = get_profile_status(pool, profile_id).await?;
    if status != "VALIDATED" && status != "PENDING_REVIEW" {
        return Err(AstDbError::NotDraft); // Reusing error for wrong status
    }

    // Get CBU ID
    let cbu_id: Uuid = sqlx::query_scalar!(
        r#"SELECT cbu_id FROM "ob-poc".cbu_trading_profiles WHERE profile_id = $1"#,
        profile_id
    )
    .fetch_one(pool)
    .await?;

    // Supersede any existing ACTIVE profile for this CBU
    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET status = 'SUPERSEDED'
        WHERE cbu_id = $1 AND status = 'ACTIVE'
        "#,
        cbu_id
    )
    .execute(pool)
    .await?;

    // Update document status
    let mut doc = load_document(pool, profile_id).await?;
    doc.status = DocumentStatus::Active;
    doc.updated_at = Some(chrono::Utc::now().to_rfc3339());

    let doc_json = serde_json::to_value(&doc)?;
    let hash = compute_document_hash(&doc);
    let now = chrono::Utc::now();

    sqlx::query!(
        r#"
        UPDATE "ob-poc".cbu_trading_profiles
        SET status = 'ACTIVE',
            document = $2,
            document_hash = $3,
            activated_at = $4,
            activated_by = $5
        WHERE profile_id = $1
        "#,
        profile_id,
        doc_json,
        hash,
        now,
        activated_by
    )
    .execute(pool)
    .await?;

    Ok(())
}

// ============================================================================
// LOAD + APPLY + SAVE HELPER
// ============================================================================

use super::ast_builder::AstBuildError;
use ob_poc_types::trading_matrix::TradingMatrixOp;

/// Load a document, apply an operation, and save it back
///
/// This is the main entry point for DSL verb handlers. It:
/// 1. Ensures the profile is in DRAFT status
/// 2. Loads the document
/// 3. Applies the operation via ast_builder
/// 4. Saves the document back
pub async fn apply_and_save(
    pool: &PgPool,
    profile_id: Uuid,
    op: TradingMatrixOp,
) -> Result<TradingMatrixDocument, ApplyError> {
    // Ensure draft status
    ensure_draft(pool, profile_id).await?;

    // Load document
    let mut doc = load_document(pool, profile_id).await?;

    // Apply operation
    super::ast_builder::apply_op(&mut doc, op)?;

    // Recompute leaf counts
    doc.compute_leaf_counts();

    // Save document
    save_document(pool, profile_id, &doc).await?;

    Ok(doc)
}

/// Combined error type for apply_and_save
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("Database error: {0}")]
    Db(#[from] AstDbError),

    #[error("AST build error: {0}")]
    Build(#[from] AstBuildError),
}

#[cfg(test)]
mod tests {
    // Tests would require database access - integration tests in separate file
}
