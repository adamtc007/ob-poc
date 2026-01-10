//! Matrix-Overlay custom operations
//!
//! Operations for linking trading matrix entries to product subscriptions.
//! Key insight: Products ADD attributes to matrix entries, they don't define them.

use anyhow::Result;
use async_trait::async_trait;

use super::CustomOperation;
use crate::dsl_v2::ast::VerbCall;
use crate::dsl_v2::executor::{ExecutionContext, ExecutionResult};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ============================================================================
// EFFECTIVE MATRIX OPERATION
// ============================================================================

/// Get effective matrix with all product overlays
///
/// Rationale: Queries v_cbu_matrix_effective view to get trading matrix entries
/// with all applicable product overlays aggregated.
pub struct MatrixEffectiveOp;

#[async_trait]
impl CustomOperation for MatrixEffectiveOp {
    fn domain(&self) -> &'static str {
        "matrix-overlay"
    }
    fn verb(&self) -> &'static str {
        "effective-matrix"
    }
    fn rationale(&self) -> &'static str {
        "Complex query aggregating trading matrix with product overlays"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        // Optional filters
        let instrument_class: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instrument-class")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        let market: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "market")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Query the effective matrix view
        let rows: Vec<(
            Uuid,              // universe_id
            String,            // cbu_name
            String,            // instrument_class
            Option<String>,    // market
            Option<String>,    // counterparty_name
            serde_json::Value, // product_overlays (jsonb)
            i64,               // overlay_count
        )> = sqlx::query_as(
            r#"SELECT
                universe_id,
                cbu_name,
                instrument_class,
                market,
                counterparty_name,
                product_overlays,
                overlay_count
               FROM "ob-poc".v_cbu_matrix_effective
               WHERE cbu_id = $1
                 AND ($2::text IS NULL OR instrument_class = $2)
                 AND ($3::text IS NULL OR market = $3)
               ORDER BY instrument_class, market, counterparty_name"#,
        )
        .bind(cbu_id)
        .bind(&instrument_class)
        .bind(&market)
        .fetch_all(pool)
        .await?;

        let result: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "universe_id": r.0,
                    "cbu_name": r.1,
                    "instrument_class": r.2,
                    "market": r.3,
                    "counterparty_name": r.4,
                    "product_overlays": r.5,
                    "overlay_count": r.6
                })
            })
            .collect();

        Ok(ExecutionResult::RecordSet(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::RecordSet(vec![]))
    }
}

// ============================================================================
// UNIFIED GAPS OPERATION
// ============================================================================

/// Get all gaps from both lifecycle and service domains
///
/// Rationale: Queries v_cbu_unified_gaps view to show missing resources
/// from both the trading matrix (lifecycle) and product (service) domains.
pub struct MatrixUnifiedGapsOp;

#[async_trait]
impl CustomOperation for MatrixUnifiedGapsOp {
    fn domain(&self) -> &'static str {
        "matrix-overlay"
    }
    fn verb(&self) -> &'static str {
        "unified-gaps"
    }
    fn rationale(&self) -> &'static str {
        "Queries unified gap view across lifecycle and service domains"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use uuid::Uuid;

        let cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let domain_filter: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "domain")
            .and_then(|a| a.value.as_string())
            .map(|s| s.to_string());

        // Query the unified gaps view
        let rows: Vec<(
            String,         // gap_source (LIFECYCLE or SERVICE)
            Option<String>, // instrument_class
            Option<String>, // market
            Option<String>, // counterparty_name
            Option<String>, // product_code
            String,         // operation_code
            String,         // operation_name
            String,         // missing_resource_code
            String,         // missing_resource_name
            Option<String>, // provisioning_verb
            bool,           // is_required
        )> = sqlx::query_as(
            r#"SELECT
                gap_source,
                instrument_class,
                market,
                counterparty_name,
                product_code,
                operation_code,
                operation_name,
                missing_resource_code,
                missing_resource_name,
                provisioning_verb,
                is_required
               FROM "ob-poc".v_cbu_unified_gaps
               WHERE cbu_id = $1
                 AND ($2::text IS NULL OR $2 = 'ALL' OR gap_source = $2)
               ORDER BY gap_source, operation_code, missing_resource_code"#,
        )
        .bind(cbu_id)
        .bind(&domain_filter)
        .fetch_all(pool)
        .await?;

        let result: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "source": r.0,
                    "instrument_class": r.1,
                    "market": r.2,
                    "counterparty_name": r.3,
                    "product_code": r.4,
                    "operation_code": r.5,
                    "operation_name": r.6,
                    "missing_resource_code": r.7,
                    "missing_resource_name": r.8,
                    "provisioning_verb": r.9,
                    "is_required": r.10
                })
            })
            .collect();

        // Summary counts
        let lifecycle_count = result.iter().filter(|r| r["source"] == "LIFECYCLE").count();
        let service_count = result.iter().filter(|r| r["source"] == "SERVICE").count();

        Ok(ExecutionResult::Record(serde_json::json!({
            "cbu_id": cbu_id,
            "total_gaps": result.len(),
            "lifecycle_gaps": lifecycle_count,
            "service_gaps": service_count,
            "gaps": result
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "total_gaps": 0,
            "lifecycle_gaps": 0,
            "service_gaps": 0,
            "gaps": []
        })))
    }
}

// ============================================================================
// COMPARE PRODUCTS OPERATION
// ============================================================================

/// Compare what different products add to a matrix entry
///
/// Rationale: Shows the delta between products - what's unique to each,
/// what overlaps, helping with product selection.
pub struct MatrixCompareProductsOp;

#[async_trait]
impl CustomOperation for MatrixCompareProductsOp {
    fn domain(&self) -> &'static str {
        "matrix-overlay"
    }
    fn verb(&self) -> &'static str {
        "compare-products"
    }
    fn rationale(&self) -> &'static str {
        "Complex comparison query showing product overlap and unique features"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use std::collections::{HashMap, HashSet};
        use uuid::Uuid;

        // cbu_id extracted for future CBU-specific comparison (e.g., overlays)
        let _cbu_id: Uuid = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "cbu-id")
            .and_then(|a| {
                if let Some(name) = a.value.as_symbol() {
                    ctx.resolve(name)
                } else {
                    a.value.as_uuid()
                }
            })
            .ok_or_else(|| anyhow::anyhow!("Missing cbu-id argument"))?;

        let instrument_class = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "instrument-class")
            .and_then(|a| a.value.as_string())
            .ok_or_else(|| anyhow::anyhow!("Missing instrument-class argument"))?;

        let products: Vec<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "products")
            .and_then(|a| {
                a.value.as_list().map(|list| {
                    list.iter()
                        .filter_map(|v| v.as_string().map(|s| s.to_string()))
                        .collect()
                })
            })
            .ok_or_else(|| anyhow::anyhow!("Missing products argument (must be a list)"))?;

        // Get base lifecycles for this instrument class
        let base_lifecycles: Vec<(String, String)> = sqlx::query_as(
            r#"SELECT l.code, l.name
               FROM "ob-poc".lifecycles l
               JOIN "ob-poc".instrument_lifecycles il ON il.lifecycle_id = l.lifecycle_id
               JOIN custody.instrument_classes ic ON ic.class_id = il.instrument_class_id
               WHERE ic.code = $1 AND il.is_mandatory = true AND l.is_active = true"#,
        )
        .bind(instrument_class)
        .fetch_all(pool)
        .await?;

        // Get services for each product
        let mut by_product: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_services: HashSet<String> = HashSet::new();

        for product_code in &products {
            let services: Vec<(String,)> = sqlx::query_as(
                r#"SELECT s.service_code
                   FROM "ob-poc".services s
                   JOIN "ob-poc".product_services ps ON ps.service_id = s.service_id
                   JOIN "ob-poc".products p ON p.product_id = ps.product_id
                   WHERE p.product_code = $1 AND s.is_active = true"#,
            )
            .bind(product_code)
            .fetch_all(pool)
            .await?;

            let service_codes: Vec<String> = services.iter().map(|s| s.0.clone()).collect();
            for s in &service_codes {
                all_services.insert(s.clone());
            }
            by_product.insert(product_code.clone(), service_codes);
        }

        // Find overlap (services in ALL products)
        let overlap: Vec<String> = all_services
            .iter()
            .filter(|s| {
                products
                    .iter()
                    .all(|p| by_product.get(p).is_some_and(|v| v.contains(s)))
            })
            .cloned()
            .collect();

        // Find unique to each product
        let mut unique_to_each: HashMap<String, Vec<String>> = HashMap::new();
        for product_code in &products {
            let services = by_product.get(product_code).cloned().unwrap_or_default();
            let unique: Vec<String> = services
                .iter()
                .filter(|s| {
                    products
                        .iter()
                        .filter(|p| *p != product_code)
                        .all(|p| by_product.get(p).is_none_or(|v| !v.contains(s)))
                })
                .cloned()
                .collect();
            unique_to_each.insert(product_code.clone(), unique);
        }

        Ok(ExecutionResult::Record(serde_json::json!({
            "instrument_class": instrument_class,
            "base_lifecycles": base_lifecycles.iter().map(|(c, n)| {
                serde_json::json!({"code": c, "name": n})
            }).collect::<Vec<_>>(),
            "by_product": by_product,
            "overlap": overlap,
            "unique_to_each": unique_to_each
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({
            "instrument_class": "",
            "base_lifecycles": [],
            "by_product": {},
            "overlap": [],
            "unique_to_each": {}
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_effective_op_metadata() {
        let op = MatrixEffectiveOp;
        assert_eq!(op.domain(), "matrix-overlay");
        assert_eq!(op.verb(), "effective-matrix");
    }

    #[test]
    fn test_matrix_unified_gaps_op_metadata() {
        let op = MatrixUnifiedGapsOp;
        assert_eq!(op.domain(), "matrix-overlay");
        assert_eq!(op.verb(), "unified-gaps");
    }

    #[test]
    fn test_matrix_compare_products_op_metadata() {
        let op = MatrixCompareProductsOp;
        assert_eq!(op.domain(), "matrix-overlay");
        assert_eq!(op.verb(), "compare-products");
    }
}
