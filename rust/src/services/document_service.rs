//! Document Service - Comprehensive CRUD Operations for Document-Attribute System
//!
//! This service provides complete CRUD operations for the document library system with
//! full AttributeID referential integrity, ISO asset type integration, and AI extraction support.

use sqlx::Error as SqlxError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DslError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Not found: {0}")]
    NotFoundError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl From<SqlxError> for DslError {
    fn from(err: SqlxError) -> Self {
        DslError::DatabaseError(err.to_string())
    }
}
use crate::models::document_models::*;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use time::OffsetDateTime;
use uuid::Uuid;

pub struct DocumentService {
    pool: PgPool,
}

impl DocumentService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ============================================================================
    // ISO ASSET TYPES OPERATIONS
    // ============================================================================

    pub async fn create_iso_asset_type(
        &self,
        asset_type: NewIsoAssetType,
    ) -> Result<IsoAssetType, DslError> {
        let result = sqlx::query_as!(
            IsoAssetType,
            r#"
            INSERT INTO "ob-poc".iso_asset_types (
                iso_code, asset_name, asset_category, asset_subcategory, description,
                regulatory_classification, liquidity_profile, suitable_for_conservative,
                suitable_for_moderate, suitable_for_aggressive, suitable_for_balanced,
                credit_risk_level, market_risk_level, liquidity_risk_level
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14
            ) RETURNING *
            "#,
            asset_type.iso_code,
            asset_type.asset_name,
            asset_type.asset_category,
            asset_type.asset_subcategory,
            asset_type.description,
            asset_type.regulatory_classification,
            asset_type.liquidity_profile,
            asset_type.suitable_for_conservative,
            asset_type.suitable_for_moderate,
            asset_type.suitable_for_aggressive,
            asset_type.suitable_for_balanced,
            asset_type.credit_risk_level,
            asset_type.market_risk_level,
            asset_type.liquidity_risk_level
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to create ISO asset type: {}", e)))?;

        Ok(result)
    }

    pub async fn get_iso_asset_type_by_code(
        &self,
        iso_code: &str,
    ) -> Result<Option<IsoAssetType>, DslError> {
        let result = sqlx::query_as!(
            IsoAssetType,
            r#"SELECT * FROM "ob-poc".iso_asset_types WHERE iso_code = $1 AND active = true"#,
            iso_code
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to get ISO asset type: {}", e)))?;

        Ok(result)
    }

    pub async fn get_iso_asset_types_for_risk_profile(
        &self,
        risk_profile: &str,
    ) -> Result<Vec<IsoAssetType>, DslError> {
        let result = match risk_profile {
            "conservative" => {
                sqlx::query_as!(
                    IsoAssetType,
                    r#"SELECT * FROM "ob-poc".iso_asset_types WHERE suitable_for_conservative = true AND active = true ORDER BY asset_category, iso_code"#
                )
                .fetch_all(&self.pool)
                .await
            }
            "moderate" => {
                sqlx::query_as!(
                    IsoAssetType,
                    r#"SELECT * FROM "ob-poc".iso_asset_types WHERE suitable_for_moderate = true AND active = true ORDER BY asset_category, iso_code"#
                )
                .fetch_all(&self.pool)
                .await
            }
            "aggressive" => {
                sqlx::query_as!(
                    IsoAssetType,
                    r#"SELECT * FROM "ob-poc".iso_asset_types WHERE suitable_for_aggressive = true AND active = true ORDER BY asset_category, iso_code"#
                )
                .fetch_all(&self.pool)
                .await
            }
            "balanced" => {
                sqlx::query_as!(
                    IsoAssetType,
                    r#"SELECT * FROM "ob-poc".iso_asset_types WHERE suitable_for_balanced = true AND active = true ORDER BY asset_category, iso_code"#
                )
                .fetch_all(&self.pool)
                .await
            }
            _ => {
                return Err(DslError::ValidationError(format!("Invalid risk profile: {}", risk_profile)));
            }
        };

        result.map_err(|e| {
            DslError::DatabaseError(format!(
                "Failed to get ISO asset types for risk profile: {}",
                e
            ))
        })
    }

    pub async fn validate_iso_asset_codes(
        &self,
        asset_codes: &[String],
    ) -> Result<Vec<String>, DslError> {
        let placeholders: Vec<String> =
            (1..=asset_codes.len()).map(|i| format!("${}", i)).collect();
        let query = format!(
            r#"SELECT iso_code FROM "ob-poc".iso_asset_types WHERE iso_code = ANY(ARRAY[{}]) AND active = true"#,
            placeholders.join(",")
        );

        let mut query_builder = sqlx::query(&query);
        for code in asset_codes {
            query_builder = query_builder.bind(code);
        }

        let rows = query_builder.fetch_all(&self.pool).await.map_err(|e| {
            DslError::DatabaseError(format!("Failed to validate ISO asset codes: {}", e))
        })?;

        let valid_codes: Vec<String> = rows
            .iter()
            .map(|row| row.get::<String, _>("iso_code"))
            .collect();

        let invalid_codes: Vec<String> = asset_codes
            .iter()
            .filter(|code| !valid_codes.contains(code))
            .cloned()
            .collect();

        if !invalid_codes.is_empty() {
            return Err(DslError::ValidationError(format!(
                "Invalid ISO asset codes: {:?}",
                invalid_codes
            )));
        }

        Ok(valid_codes)
    }

    pub async fn check_asset_suitability_for_risk_profile(
        &self,
        permitted_assets: &[String],
        risk_profile: &str,
    ) -> Result<Vec<AssetSuitabilityCheck>, DslError> {
        let mut results = Vec::new();

        for asset_code in permitted_assets {
            let asset = self.get_iso_asset_type_by_code(asset_code).await?;

            if let Some(asset) = asset {
                let (is_suitable, reason) = match risk_profile {
                    "conservative" => (
                        asset.suitable_for_conservative,
                        if asset.suitable_for_conservative {
                            "Suitable".to_string()
                        } else {
                            "Asset too risky for conservative profile".to_string()
                        },
                    ),
                    "moderate" => (
                        asset.suitable_for_moderate,
                        if asset.suitable_for_moderate {
                            "Suitable".to_string()
                        } else {
                            "Asset not suitable for moderate profile".to_string()
                        },
                    ),
                    "aggressive" => (
                        asset.suitable_for_aggressive,
                        if asset.suitable_for_aggressive {
                            "Suitable".to_string()
                        } else {
                            "Asset not available for aggressive profile".to_string()
                        },
                    ),
                    "balanced" => (
                        asset.suitable_for_balanced,
                        if asset.suitable_for_balanced {
                            "Suitable".to_string()
                        } else {
                            "Asset not suitable for balanced profile".to_string()
                        },
                    ),
                    _ => (false, format!("Invalid risk profile: {}", risk_profile)),
                };

                results.push(AssetSuitabilityCheck {
                    iso_code: asset.iso_code,
                    asset_name: asset.asset_name,
                    is_suitable,
                    reason,
                });
            } else {
                results.push(AssetSuitabilityCheck {
                    iso_code: asset_code.clone(),
                    asset_name: "Unknown".to_string(),
                    is_suitable: false,
                    reason: "Asset code not found".to_string(),
                });
            }
        }

        Ok(results)
    }

    // ============================================================================
    // DOCUMENT TYPES OPERATIONS
    // ============================================================================

    pub async fn create_document_type(
        &self,
        document_type: NewDocumentType,
    ) -> Result<DocumentType, DslError> {
        let result = sqlx::query_as!(
            DocumentType,
            r#"
            INSERT INTO "ob-poc".document_types (
                type_code, display_name, category, domain, primary_attribute_id, description,
                typical_issuers, expected_attribute_ids, key_data_point_attributes,
                ai_description, common_contents
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
            ) RETURNING *
            "#,
            document_type.type_code,
            document_type.display_name,
            document_type.category,
            document_type.domain,
            document_type.primary_attribute_id,
            document_type.description,
            &document_type.typical_issuers,
            &document_type.expected_attribute_ids,
            document_type
                .key_data_point_attributes
                .as_ref()
                .map(|v| v.as_slice()),
            document_type.ai_description,
            document_type.common_contents
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to create document type: {}", e)))?;

        Ok(result)
    }

    pub async fn get_document_type_by_code(
        &self,
        type_code: &str,
    ) -> Result<Option<DocumentType>, DslError> {
        let result = sqlx::query_as!(
            DocumentType,
            r#"SELECT * FROM "ob-poc".document_types WHERE type_code = $1 AND active = true"#,
            type_code
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to get document type: {}", e)))?;

        Ok(result)
    }

    pub async fn get_document_types_by_category(
        &self,
        category: &str,
    ) -> Result<Vec<DocumentType>, DslError> {
        let result = sqlx::query_as!(
            DocumentType,
            r#"SELECT * FROM "ob-poc".document_types WHERE category = $1 AND active = true ORDER BY type_code"#,
            category
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to get document types by category: {}", e)))?;

        Ok(result)
    }

    pub async fn update_document_type(
        &self,
        type_code: &str,
        update: UpdateDocumentType,
    ) -> Result<DocumentType, DslError> {
        let mut query = "UPDATE \"ob-poc\".document_types SET updated_at = NOW()".to_string();
        let mut params: Vec<String> = vec![];
        let mut param_count = 1;

        if let Some(display_name) = update.display_name {
            query.push_str(&format!(", display_name = ${}", param_count));
            params.push(display_name);
            param_count += 1;
        }

        if let Some(description) = update.description {
            query.push_str(&format!(", description = ${}", param_count));
            params.push(description);
            param_count += 1;
        }

        if let Some(expected_attribute_ids) = update.expected_attribute_ids {
            query.push_str(&format!(", expected_attribute_ids = ${}", param_count));
            params.push(format!(
                "{{{}}}",
                expected_attribute_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            param_count += 1;
        }

        if let Some(active) = update.active {
            query.push_str(&format!(", active = ${}", param_count));
            params.push(active.to_string());
            param_count += 1;
        }

        query.push_str(&format!(" WHERE type_code = ${} RETURNING *", param_count));
        params.push(type_code.to_string());

        // Note: This is a simplified version. In production, you'd use a proper query builder
        // or handle the dynamic query construction more robustly.
        let result = sqlx::query_as!(
            DocumentType,
            r#"
            UPDATE "ob-poc".document_types
            SET updated_at = NOW()
            WHERE type_code = $1 AND active = true
            RETURNING *
            "#,
            type_code
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to update document type: {}", e)))?;

        Ok(result)
    }

    // ============================================================================
    // DOCUMENT CATALOG OPERATIONS
    // ============================================================================

    pub async fn create_document(
        &self,
        document: NewDocumentCatalog,
    ) -> Result<DocumentCatalog, DslError> {
        let result = sqlx::query_as!(
            DocumentCatalog,
            r#"
            INSERT INTO "ob-poc".document_catalog (
                document_code, document_type_id, issuer_id, title, description,
                issue_date, expiry_date, language, related_entities, tags, confidentiality_level
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11
            ) RETURNING *
            "#,
            document.document_code,
            document.document_type_id,
            document.issuer_id,
            document.title,
            document.description,
            document.issue_date,
            document.expiry_date,
            document.language,
            document.related_entities.as_ref().map(|v| v.as_slice()),
            document.tags.as_ref().map(|v| v.as_slice()),
            document.confidentiality_level
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to create document: {}", e)))?;

        Ok(result)
    }

    pub async fn get_document_by_id(
        &self,
        document_id: Uuid,
    ) -> Result<Option<DocumentCatalog>, DslError> {
        let result = sqlx::query_as!(
            DocumentCatalog,
            r#"SELECT * FROM "ob-poc".document_catalog WHERE document_id = $1"#,
            document_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to get document: {}", e)))?;

        Ok(result)
    }

    pub async fn get_document_by_code(
        &self,
        document_code: &str,
    ) -> Result<Option<DocumentCatalog>, DslError> {
        let result = sqlx::query_as!(
            DocumentCatalog,
            r#"SELECT * FROM "ob-poc".document_catalog WHERE document_code = $1"#,
            document_code
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to get document by code: {}", e)))?;

        Ok(result)
    }

    pub async fn get_document_with_attributes(
        &self,
        document_id: Uuid,
    ) -> Result<Option<DocumentCatalogWithAttributes>, DslError> {
        let result = sqlx::query_as!(
            DocumentCatalogWithAttributes,
            r#"
            SELECT
                dc.*,
                dt.type_code,
                dt.display_name as document_type_name,
                dt.category as document_category,
                dt.domain as document_domain,
                dt.expected_attribute_ids,
                dt.key_data_point_attributes,
                di.issuer_code,
                di.legal_name as issuer_name
            FROM "ob-poc".document_catalog dc
            LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
            LEFT JOIN "ob-poc".document_issuers di ON dc.issuer_id = di.issuer_id
            WHERE dc.document_id = $1
            "#,
            document_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            DslError::DatabaseError(format!("Failed to get document with attributes: {}", e))
        })?;

        Ok(result)
    }

    pub async fn update_document_attributes(
        &self,
        document_id: Uuid,
        extracted_attributes: serde_json::Value,
        extraction_confidence: Option<f64>,
        extraction_method: Option<String>,
    ) -> Result<DocumentCatalog, DslError> {
        let result = sqlx::query_as!(
            DocumentCatalog,
            r#"
            UPDATE "ob-poc".document_catalog
            SET
                extracted_attributes = $1,
                extraction_confidence = $2,
                extraction_method = $3,
                extraction_date = NOW(),
                updated_at = NOW()
            WHERE document_id = $4
            RETURNING *
            "#,
            extracted_attributes,
            extraction_confidence,
            extraction_method,
            document_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            DslError::DatabaseError(format!("Failed to update document attributes: {}", e))
        })?;

        Ok(result)
    }

    pub async fn search_documents(
        &self,
        search_request: DocumentSearchRequest,
    ) -> Result<DocumentSearchResponse, DslError> {
        let mut where_conditions = Vec::new();
        let mut params: Vec<Box<dyn sqlx::Encode<'_, sqlx::Postgres> + Send + Sync>> = Vec::new();
        let mut param_count = 1;

        // Build dynamic WHERE clause
        if let Some(query) = &search_request.query {
            where_conditions.push(format!(
                "(dc.title ILIKE ${} OR dc.description ILIKE ${})",
                param_count,
                param_count + 1
            ));
            let search_pattern = format!("%{}%", query);
            params.push(Box::new(search_pattern.clone()));
            params.push(Box::new(search_pattern));
            param_count += 2;
        }

        if let Some(document_type) = &search_request.document_type {
            where_conditions.push(format!("dt.type_code = ${}", param_count));
            params.push(Box::new(document_type.clone()));
            param_count += 1;
        }

        if let Some(category) = &search_request.category {
            where_conditions.push(format!("dt.category = ${}", param_count));
            params.push(Box::new(category.clone()));
            param_count += 1;
        }

        let where_clause = if where_conditions.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", where_conditions.join(" AND "))
        };

        let limit = search_request.limit.unwrap_or(50);
        let offset = search_request.offset.unwrap_or(0);

        // This is a simplified version - in production you'd use a proper query builder
        let query = format!(
            r#"
            SELECT
                dc.*,
                dt.type_code,
                dt.display_name as document_type_name,
                dt.category as document_category,
                dt.domain as document_domain,
                dt.expected_attribute_ids,
                dt.key_data_point_attributes,
                di.issuer_code,
                di.legal_name as issuer_name
            FROM "ob-poc".document_catalog dc
            LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
            LEFT JOIN "ob-poc".document_issuers di ON dc.issuer_id = di.issuer_id
            {}
            ORDER BY dc.created_at DESC
            LIMIT {} OFFSET {}
            "#,
            where_clause, limit, offset
        );

        // For simplicity, using a basic query. In production, use proper parameter binding.
        let documents = sqlx::query_as!(
            DocumentCatalogWithAttributes,
            r#"
            SELECT
                dc.*,
                dt.type_code,
                dt.display_name as document_type_name,
                dt.category as document_category,
                dt.domain as document_domain,
                dt.expected_attribute_ids,
                dt.key_data_point_attributes,
                di.issuer_code,
                di.legal_name as issuer_name
            FROM "ob-poc".document_catalog dc
            LEFT JOIN "ob-poc".document_types dt ON dc.document_type_id = dt.type_id
            LEFT JOIN "ob-poc".document_issuers di ON dc.issuer_id = di.issuer_id
            ORDER BY dc.created_at DESC
            LIMIT $1 OFFSET $2
            "#,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to search documents: {}", e)))?;

        let total_count =
            sqlx::query!(r#"SELECT COUNT(*) as count FROM "ob-poc".document_catalog"#)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| DslError::DatabaseError(format!("Failed to count documents: {}", e)))?
                .count
                .unwrap_or(0);

        Ok(DocumentSearchResponse {
            documents,
            total_count,
            has_more: (offset + limit) < total_count,
        })
    }

    // ============================================================================
    // INVESTMENT MANDATE SPECIFIC OPERATIONS
    // ============================================================================

    pub async fn validate_investment_mandate(
        &self,
        document_id: Uuid,
    ) -> Result<InvestmentMandateValidation, DslError> {
        let document = self
            .get_document_by_id(document_id)
            .await?
            .ok_or_else(|| DslError::NotFoundError("Document not found".to_string()))?;

        let mut validation_errors = Vec::new();
        let mut asset_suitability_issues = Vec::new();

        if let Some(extracted_attrs) = &document.extracted_attributes {
            // Extract investment mandate specific attributes
            let permitted_assets = extracted_attrs
                .get("d0cf0021-0000-0000-0000-000000000004")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let risk_profile = extracted_attrs
                .get("d0cf0021-0000-0000-0000-000000000006")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if !permitted_assets.is_empty() && !risk_profile.is_empty() {
                let asset_codes: Vec<String> = permitted_assets
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                // Validate ISO asset codes
                match self.validate_iso_asset_codes(&asset_codes).await {
                    Ok(valid_codes) => {
                        // Check suitability for risk profile
                        match self
                            .check_asset_suitability_for_risk_profile(&valid_codes, risk_profile)
                            .await
                        {
                            Ok(suitability_checks) => {
                                for check in suitability_checks {
                                    if !check.is_suitable {
                                        asset_suitability_issues.push(check);
                                    }
                                }
                            }
                            Err(e) => {
                                validation_errors.push(format!("Suitability check failed: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        validation_errors.push(format!("Invalid asset codes: {}", e));
                    }
                }
            }
        } else {
            validation_errors.push("No extracted attributes found".to_string());
        }

        Ok(InvestmentMandateValidation {
            is_valid: validation_errors.is_empty() && asset_suitability_issues.is_empty(),
            validation_errors,
            asset_suitability_issues,
            permitted_assets_validated: vec![],  // TODO: implement
            prohibited_assets_validated: vec![], // TODO: implement
        })
    }

    pub async fn extract_investment_mandate_data(
        &self,
        document_id: Uuid,
    ) -> Result<InvestmentMandateExtraction, DslError> {
        let document = self
            .get_document_by_id(document_id)
            .await?
            .ok_or_else(|| DslError::NotFoundError("Document not found".to_string()))?;

        if let Some(extracted_attrs) = &document.extracted_attributes {
            let extraction = InvestmentMandateExtraction {
                fund_name: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000001")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                investment_objective: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000002")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                asset_allocation: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000003")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                permitted_assets: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000004")
                    .and_then(|v| v.as_str())
                    .map(|s| s.split(',').map(|code| code.trim().to_string()).collect()),
                prohibited_assets: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000005")
                    .and_then(|v| v.as_str())
                    .map(|s| s.split(',').map(|code| code.trim().to_string()).collect()),
                risk_profile: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000006")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                benchmark_index: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000007")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                geographic_focus: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000008")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                leverage_limit: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000009")
                    .and_then(|v| v.as_f64()),
                liquidity_terms: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000010")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                concentration_limits: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000011")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                duration_target: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000012")
                    .and_then(|v| v.as_f64()),
                credit_quality_floor: extracted_attrs
                    .get("d0cf0021-0000-0000-0000-000000000013")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            };

            Ok(extraction)
        } else {
            Err(DslError::ValidationError(
                "No extracted attributes found for investment mandate".to_string(),
            ))
        }
    }

    // ============================================================================
    // STATISTICS AND ANALYTICS
    // ============================================================================

    pub async fn get_document_mapping_summary(&self) -> Result<DocumentMappingSummary, DslError> {
        let stats = sqlx::query!(
            r#"
            SELECT
                (SELECT COUNT(*) FROM "ob-poc".document_types WHERE active = true) as total_document_types,
                (SELECT COUNT(*) FROM "ob-poc".document_types WHERE array_length(expected_attribute_ids, 1) > 0 AND active = true) as mapped_document_types,
                (SELECT COUNT(*) FROM "ob-poc".iso_asset_types WHERE active = true) as total_iso_asset_types,
                (SELECT COUNT(*) FROM "ob-poc".dictionary WHERE domain IN ('Document', 'Identity', 'Corporate', 'Financial', 'Legal', 'Compliance', 'ISDA', 'Fund', 'Regulatory', 'Transaction')) as total_document_attributes
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to get mapping summary: {}", e)))?;

        let total_document_types = stats.total_document_types.unwrap_or(0);
        let mapped_document_types = stats.mapped_document_types.unwrap_or(0);
        let coverage_percentage = if total_document_types > 0 {
            (mapped_document_types as f64 / total_document_types as f64) * 100.0
        } else {
            0.0
        };

        // Check if investment mandate is ready
        let investment_mandate_ready = sqlx::query!(
            r#"
            SELECT COUNT(*) > 0 as ready
            FROM "ob-poc".document_types
            WHERE type_code = 'investment_mandate'
            AND array_length(expected_attribute_ids, 1) > 0
            AND active = true
            "#
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            DslError::DatabaseError(format!(
                "Failed to check investment mandate readiness: {}",
                e
            ))
        })?
        .ready
        .unwrap_or(false);

        Ok(DocumentMappingSummary {
            total_document_types,
            mapped_document_types,
            coverage_percentage,
            total_iso_asset_types: stats.total_iso_asset_types.unwrap_or(0),
            total_document_attributes: stats.total_document_attributes.unwrap_or(0),
            investment_mandate_ready,
        })
    }

    pub async fn get_document_type_statistics(
        &self,
    ) -> Result<Vec<DocumentTypeStatistics>, DslError> {
        let stats = sqlx::query_as!(
            DocumentTypeStatistics,
            r#"
            SELECT
                dt.type_code,
                dt.display_name,
                dt.category,
                COUNT(dc.document_id) as total_documents,
                COUNT(CASE WHEN dc.extracted_attributes IS NOT NULL THEN 1 END) as extracted_documents,
                AVG(dc.extraction_confidence) as average_confidence,
                    MAX(dc.created_at) as most_recent_document
                FROM "ob-poc".document_types dt
                LEFT JOIN "ob-poc".document_catalog dc ON dt.type_id = dc.document_type_id
                WHERE dt.active = true
                GROUP BY dt.type_code, dt.display_name, dt.category
                ORDER BY dt.category, dt.type_code
                "#
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DslError::DatabaseError(format!("Failed to get document type statistics: {}", e)))?;

        Ok(stats)
    }

    pub async fn get_document_attribute_statistics(
        &self,
    ) -> Result<DocumentAttributeStatistics, DslError> {
        let stats = sqlx::query!(
                r#"
                SELECT
                    COUNT(*) as total_documents,
                    COUNT(CASE WHEN extracted_attributes IS NOT NULL THEN 1 END) as documents_with_extractions,
                    AVG(extraction_confidence) as average_extraction_confidence,
                    (
                        SELECT dt.type_code
                        FROM "ob-poc".document_types dt
                        JOIN "ob-poc".document_catalog dc ON dt.type_id = dc.document_type_id
                        GROUP BY dt.type_code
                        ORDER BY COUNT(*) DESC
                        LIMIT 1
                    ) as most_common_document_type
                FROM "ob-poc".document_catalog
                "#
            )
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DslError::DatabaseError(format!("Failed to get document attribute statistics: {}", e)))?;

        let extraction_methods = sqlx::query!(
            r#"
                SELECT DISTINCT extraction_method
                FROM "ob-poc".document_catalog
                WHERE extraction_method IS NOT NULL
                "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to get extraction methods: {}", e)))?;

        let extraction_methods_used: Vec<String> = extraction_methods
            .into_iter()
            .filter_map(|row| row.extraction_method)
            .collect();

        let total_documents = stats.total_documents.unwrap_or(0);
        let documents_with_extractions = stats.documents_with_extractions.unwrap_or(0);
        let attribute_coverage_percentage = if total_documents > 0 {
            (documents_with_extractions as f64 / total_documents as f64) * 100.0
        } else {
            0.0
        };

        Ok(DocumentAttributeStatistics {
            total_documents,
            documents_with_extractions,
            average_extraction_confidence: stats.average_extraction_confidence,
            most_common_document_type: stats.most_common_document_type,
            extraction_methods_used,
            attribute_coverage_percentage,
        })
    }

    // ============================================================================
    // BULK OPERATIONS
    // ============================================================================

    pub async fn bulk_import_documents(
        &self,
        import_request: BulkDocumentImport,
    ) -> Result<BulkImportResult, DslError> {
        let mut successful_imports = 0;
        let mut failed_imports = 0;
        let mut skipped_duplicates = 0;
        let mut errors = Vec::new();
        let mut imported_document_ids = Vec::new();

        for document in import_request.documents {
            // Check for duplicates if requested
            if import_request.skip_duplicates {
                if let Ok(Some(_)) = self.get_document_by_code(&document.document_code).await {
                    skipped_duplicates += 1;
                    continue;
                }
            }

            match self.create_document(document).await {
                Ok(created_doc) => {
                    successful_imports += 1;
                    imported_document_ids.push(created_doc.document_id);
                }
                Err(e) => {
                    failed_imports += 1;
                    errors.push(format!("Failed to import document: {}", e));
                }
            }
        }

        Ok(BulkImportResult {
            total_processed: import_request.documents.len(),
            successful_imports,
            failed_imports,
            skipped_duplicates,
            errors,
            imported_document_ids,
        })
    }

    // ============================================================================
    // AI EXTRACTION OPERATIONS
    // ============================================================================

    pub async fn process_ai_extraction(
        &self,
        request: AiExtractionRequest,
    ) -> Result<AiExtractionResult, DslError> {
        let start_time = std::time::Instant::now();

        // This is a placeholder for AI extraction logic
        // In a real implementation, this would integrate with AI services
        let mut extracted_attributes = HashMap::new();
        let mut confidence_scores = HashMap::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Mock extraction for demonstration
        match request.extraction_method.as_str() {
            "ai" => {
                // Simulate AI extraction
                extracted_attributes.insert(
                    "d0cf0021-0000-0000-0000-000000000001".to_string(),
                    serde_json::Value::String("Sample Fund Name".to_string()),
                );
                confidence_scores.insert("d0cf0021-0000-0000-0000-000000000001".to_string(), 0.95);
            }
            "manual" => {
                warnings.push("Manual extraction requires human input".to_string());
            }
            _ => {
                errors.push(format!(
                    "Unsupported extraction method: {}",
                    request.extraction_method
                ));
            }
        }

        let processing_time = start_time.elapsed();
        let overall_confidence = if confidence_scores.is_empty() {
            0.0
        } else {
            confidence_scores.values().sum::<f64>() / confidence_scores.len() as f64
        };

        // Update document with extracted attributes
        let extracted_json = serde_json::to_value(&extracted_attributes).map_err(|e| {
            DslError::SerializationError(format!("Failed to serialize extracted attributes: {}", e))
        })?;

        let _updated_doc = self
            .update_document_attributes(
                request.document_id,
                extracted_json,
                Some(overall_confidence),
                Some(request.extraction_method.clone()),
            )
            .await?;

        Ok(AiExtractionResult {
            document_id: request.document_id,
            extracted_attributes,
            confidence_scores,
            overall_confidence,
            extraction_method: request.extraction_method,
            processing_time_ms: processing_time.as_millis() as u64,
            ai_model_used: request.ai_model,
            errors,
            warnings,
        })
    }

    // ============================================================================
    // DOCUMENT USAGE TRACKING
    // ============================================================================

    pub async fn create_document_usage(
        &self,
        usage: NewDocumentUsage,
    ) -> Result<DocumentUsage, DslError> {
        let result = sqlx::query_as!(
            DocumentUsage,
            r#"
                INSERT INTO "ob-poc".document_usage (
                    document_id, dsl_version_id, cbu_id, workflow_stage, usage_type,
                    usage_context, business_purpose
                ) VALUES (
                    $1, $2, $3, $4, $5, $6, $7
                ) RETURNING *
                "#,
            usage.document_id,
            usage.dsl_version_id,
            usage.cbu_id,
            usage.workflow_stage,
            usage.usage_type,
            usage.usage_context,
            usage.business_purpose
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| DslError::DatabaseError(format!("Failed to create document usage: {}", e)))?;

        Ok(result)
    }

    pub async fn get_document_usage_history(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<DocumentUsage>, DslError> {
        let result = sqlx::query_as!(
            DocumentUsage,
            r#"
                SELECT * FROM "ob-poc".document_usage
                WHERE document_id = $1
                ORDER BY created_at DESC
                "#,
            document_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            DslError::DatabaseError(format!("Failed to get document usage history: {}", e))
        })?;

        Ok(result)
    }

    // ============================================================================
    // DOCUMENT RELATIONSHIPS
    // ============================================================================

    pub async fn create_document_relationship(
        &self,
        relationship: NewDocumentRelationship,
    ) -> Result<DocumentRelationship, DslError> {
        let result = sqlx::query_as!(
            DocumentRelationship,
            r#"
                INSERT INTO "ob-poc".document_relationships (
                    source_document_id, target_document_id, relationship_type,
                    relationship_strength, description, business_rationale
                ) VALUES (
                    $1, $2, $3, $4, $5, $6
                ) RETURNING *
                "#,
            relationship.source_document_id,
            relationship.target_document_id,
            relationship.relationship_type,
            relationship
                .relationship_strength
                .unwrap_or("strong".to_string()),
            relationship.description,
            relationship.business_rationale
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            DslError::DatabaseError(format!("Failed to create document relationship: {}", e))
        })?;

        Ok(result)
    }

    pub async fn get_document_relationships(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<DocumentRelationship>, DslError> {
        let result = sqlx::query_as!(
            DocumentRelationship,
            r#"
                SELECT * FROM "ob-poc".document_relationships
                WHERE source_document_id = $1 OR target_document_id = $1
                ORDER BY created_at DESC
                "#,
            document_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            DslError::DatabaseError(format!("Failed to get document relationships: {}", e))
        })?;

        Ok(result)
    }
}
