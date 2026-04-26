//! Matrix-overlay domain verbs (3 plugin verbs) — SemOS-side
//! YAML-first re-implementation of the plugin subset of
//! `rust/config/verbs/matrix-overlay.yaml`.
//!
//! - `matrix-overlay.effective-matrix` — projects
//!   `v_cbu_matrix_effective` for a CBU, filtered by instrument
//!   class / market.
//! - `matrix-overlay.unified-gaps` — projects
//!   `v_cbu_unified_gaps` with a LIFECYCLE/SERVICE source
//!   breakdown summary.
//! - `matrix-overlay.compare-products` — cross-references
//!   product service offerings vs a base lifecycle spine,
//!   emitting overlap + per-product unique sets.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_list, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

// ── matrix-overlay.effective-matrix ───────────────────────────────────────────

pub struct EffectiveMatrix;

#[async_trait]
impl SemOsVerbOp for EffectiveMatrix {
    fn fqn(&self) -> &str {
        "matrix-overlay.effective-matrix"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let instrument_class = json_extract_string_opt(args, "instrument-class");
        let market = json_extract_string_opt(args, "market");

        type Row = (
            Uuid,
            String,
            String,
            Option<String>,
            Option<String>,
            Value,
            i64,
        );

        let rows: Vec<Row> = sqlx::query_as(
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
        .fetch_all(scope.executor())
        .await?;

        let results: Vec<Value> = rows
            .into_iter()
            .map(|r| {
                json!({
                    "universe_id": r.0,
                    "cbu_name": r.1,
                    "instrument_class": r.2,
                    "market": r.3,
                    "counterparty_name": r.4,
                    "product_overlays": r.5,
                    "overlay_count": r.6,
                })
            })
            .collect();

        Ok(VerbExecutionOutcome::RecordSet(results))
    }
}

// ── matrix-overlay.unified-gaps ───────────────────────────────────────────────

pub struct UnifiedGaps;

#[async_trait]
impl SemOsVerbOp for UnifiedGaps {
    fn fqn(&self) -> &str {
        "matrix-overlay.unified-gaps"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let domain_filter = json_extract_string_opt(args, "domain");

        type Row = (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
            String,
            String,
            Option<String>,
            bool,
        );

        let rows: Vec<Row> = sqlx::query_as(
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
        .fetch_all(scope.executor())
        .await?;

        let gaps: Vec<Value> = rows
            .into_iter()
            .map(|r| {
                json!({
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
                    "is_required": r.10,
                })
            })
            .collect();

        let lifecycle_count = gaps.iter().filter(|r| r["source"] == "LIFECYCLE").count();
        let service_count = gaps.iter().filter(|r| r["source"] == "SERVICE").count();

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "total_gaps": gaps.len(),
            "lifecycle_gaps": lifecycle_count,
            "service_gaps": service_count,
            "gaps": gaps,
        })))
    }
}

// ── matrix-overlay.compare-products ───────────────────────────────────────────

pub struct CompareProducts;

#[async_trait]
impl SemOsVerbOp for CompareProducts {
    fn fqn(&self) -> &str {
        "matrix-overlay.compare-products"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let _cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let instrument_class = json_extract_string(args, "instrument-class")?;
        let products = json_extract_string_list(args, "products")?;

        let base_lifecycles: Vec<(String, String)> = sqlx::query_as(
            r#"SELECT l.code, l.name
               FROM "ob-poc".lifecycles l
               JOIN "ob-poc".instrument_lifecycles il ON il.lifecycle_id = l.lifecycle_id
               JOIN "ob-poc".instrument_classes ic ON ic.class_id = il.instrument_class_id
               WHERE ic.code = $1 AND il.is_mandatory = true AND l.is_active = true"#,
        )
        .bind(&instrument_class)
        .fetch_all(scope.executor())
        .await?;

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
            .fetch_all(scope.executor())
            .await?;

            let service_codes: Vec<String> = services.iter().map(|s| s.0.clone()).collect();
            for s in &service_codes {
                all_services.insert(s.clone());
            }
            by_product.insert(product_code.clone(), service_codes);
        }

        let overlap: Vec<String> = all_services
            .iter()
            .filter(|s| {
                products
                    .iter()
                    .all(|p| by_product.get(p).is_some_and(|v| v.contains(s)))
            })
            .cloned()
            .collect();

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

        Ok(VerbExecutionOutcome::Record(json!({
            "instrument_class": instrument_class,
            "base_lifecycles": base_lifecycles.iter().map(|(c, n)| {
                json!({"code": c, "name": n})
            }).collect::<Vec<_>>(),
            "by_product": by_product,
            "overlap": overlap,
            "unique_to_each": unique_to_each,
        })))
    }
}
