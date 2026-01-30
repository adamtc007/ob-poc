//! Document Bundle Database Service
//!
//! Handles database operations for applying bundles to CBUs.

use anyhow::{anyhow, Context, Result};
use sqlx::PgPool;
use tracing::{debug, info};
use uuid::Uuid;

use super::registry::DocsBundleRegistry;
use super::types::{AppliedBundle, ApplyBundleResult, BundleContext, CreatedRequirement};

/// Service for document bundle database operations
pub struct DocsBundleService {
    pool: PgPool,
    registry: DocsBundleRegistry,
}

impl DocsBundleService {
    /// Create a new service with the given pool and registry
    pub fn new(pool: PgPool, registry: DocsBundleRegistry) -> Self {
        Self { pool, registry }
    }

    /// Apply a document bundle to a CBU
    ///
    /// Creates document_requirements for each document in the bundle.
    /// Handles inheritance and required_if conditions.
    pub async fn apply_bundle(
        &self,
        cbu_id: Uuid,
        bundle_id: &str,
        context: &BundleContext,
        macro_id: Option<&str>,
        applied_by: Option<&str>,
    ) -> Result<ApplyBundleResult> {
        // Get bundle definition
        let bundle = self
            .registry
            .get(bundle_id)
            .ok_or_else(|| anyhow!("Bundle not found: {}", bundle_id))?;

        if !bundle.is_effective() {
            return Err(anyhow!(
                "Bundle {} is not currently effective (effective: {} to {:?})",
                bundle_id,
                bundle.effective_from,
                bundle.effective_to
            ));
        }

        // Get resolved documents (with inheritance)
        let resolved_docs = self
            .registry
            .get_resolved(bundle_id)
            .ok_or_else(|| anyhow!("Failed to resolve bundle: {}", bundle_id))?;

        debug!(
            "Applying bundle {} to CBU {}: {} documents",
            bundle_id,
            cbu_id,
            resolved_docs.len()
        );

        // Start transaction
        let mut tx = self.pool.begin().await?;

        // Record bundle application (upsert)
        let applied_bundle: AppliedBundle = sqlx::query_as(
            r#"
            INSERT INTO "ob-poc".applied_bundles (
                cbu_id, bundle_id, bundle_version, macro_id, applied_by
            ) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (cbu_id, bundle_id) DO UPDATE
                SET bundle_version = EXCLUDED.bundle_version,
                    macro_id = COALESCE(EXCLUDED.macro_id, applied_bundles.macro_id),
                    applied_at = NOW(),
                    applied_by = EXCLUDED.applied_by
            RETURNING
                applied_id,
                cbu_id,
                bundle_id,
                bundle_version,
                macro_id,
                applied_at,
                applied_by
            "#,
        )
        .bind(cbu_id)
        .bind(bundle_id)
        .bind(&bundle.version)
        .bind(macro_id)
        .bind(applied_by)
        .fetch_one(&mut *tx)
        .await
        .context("Failed to record bundle application")?;

        // Create document requirements
        let mut requirements = Vec::new();

        for doc in resolved_docs {
            // Evaluate required_if condition
            let should_create = match &doc.required_if {
                Some(condition) => context.evaluate(condition),
                None => true,
            };

            if !should_create {
                debug!(
                    "Skipping document {} (condition not met: {:?})",
                    doc.document_id, doc.required_if
                );
                continue;
            }

            // Create or update document requirement
            let required_state = if doc.required { "verified" } else { "received" };

            // Use upsert pattern - check if exists first, then insert or update
            // This works with our partial unique index
            let existing: Option<Uuid> = sqlx::query_scalar(
                r#"
                SELECT requirement_id
                FROM "ob-poc".document_requirements
                WHERE subject_cbu_id = $1
                  AND doc_type = $2
                  AND workflow_instance_id IS NULL
                  AND subject_entity_id IS NULL
                "#,
            )
            .bind(cbu_id)
            .bind(&doc.document_id)
            .fetch_optional(&mut *tx)
            .await?;

            let requirement_id: Uuid = match existing {
                Some(req_id) => {
                    // Update existing
                    sqlx::query(
                        r#"
                        UPDATE "ob-poc".document_requirements
                        SET updated_at = NOW()
                        WHERE requirement_id = $1
                        "#,
                    )
                    .bind(req_id)
                    .execute(&mut *tx)
                    .await?;
                    req_id
                }
                None => {
                    // Insert new
                    sqlx::query_scalar(
                        r#"
                        INSERT INTO "ob-poc".document_requirements (
                            subject_cbu_id,
                            doc_type,
                            required_state,
                            status
                        ) VALUES ($1, $2, $3, 'missing')
                        RETURNING requirement_id
                        "#,
                    )
                    .bind(cbu_id)
                    .bind(&doc.document_id)
                    .bind(required_state)
                    .fetch_one(&mut *tx)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to create requirement for document: {}",
                            doc.document_id
                        )
                    })?
                }
            };

            let created = CreatedRequirement {
                requirement_id,
                document_id: doc.document_id.clone(),
                document_name: doc.document_name.clone(),
                required: doc.required,
                status: "missing".to_string(),
            };

            debug!(
                "Created requirement {} for document {}",
                created.requirement_id, created.document_id
            );
            requirements.push(created);
        }

        tx.commit().await?;

        info!(
            "Applied bundle {} to CBU {}: {} requirements created",
            bundle_id,
            cbu_id,
            requirements.len()
        );

        Ok(ApplyBundleResult {
            applied_bundle,
            requirements,
        })
    }

    /// Get bundles applied to a CBU
    pub async fn get_applied_bundles(&self, cbu_id: Uuid) -> Result<Vec<AppliedBundle>> {
        let bundles: Vec<AppliedBundle> = sqlx::query_as(
            r#"
            SELECT
                applied_id,
                cbu_id,
                bundle_id,
                bundle_version,
                macro_id,
                applied_at,
                applied_by
            FROM "ob-poc".applied_bundles
            WHERE cbu_id = $1
            ORDER BY applied_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(bundles)
    }

    /// Check if a bundle has been applied to a CBU
    pub async fn is_bundle_applied(&self, cbu_id: Uuid, bundle_id: &str) -> Result<bool> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".applied_bundles
            WHERE cbu_id = $1 AND bundle_id = $2
            "#,
        )
        .bind(cbu_id)
        .bind(bundle_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count.unwrap_or(0) > 0)
    }

    /// Get the registry
    pub fn registry(&self) -> &DocsBundleRegistry {
        &self.registry
    }
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;

    // Integration tests would go here
    // Requires test database setup
}
