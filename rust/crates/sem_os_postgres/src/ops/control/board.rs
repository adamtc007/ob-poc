//! Board-controller lifecycle verbs (6).
//!
//! - `show-board-controller` — compute (or retrieve override)
//!   board controller with confidence + derivation rule.
//! - `recompute-board-controller` — compute + upsert into
//!   `board_controller_cache`.
//! - `set-board-controller` — insert manual override row.
//! - `clear-board-controller-override` — cleared_at = NOW(),
//!   re-compute to confirm.
//! - `import-psc-register` / `import-gleif-control` — stubs
//!   that count existing imported rows pending API integration.
//!
//! Recompute + clear share the show computation via
//! [`compute_show_board_controller`].

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_bool_opt, json_extract_string, json_extract_string_opt, json_extract_uuid,
    json_extract_uuid_opt,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use crate::ops::SemOsVerbOp;

/// Shared body for `show-board-controller`. Called directly by
/// `ShowBoardController::execute`, and indirectly by
/// `RecomputeBoardController` + `ClearBoardControllerOverride` to
/// refresh the cache / echo the computed value after clearing an
/// override.
pub(super) async fn compute_show_board_controller(
    cbu_id: Uuid,
    scope: &mut dyn TransactionScope,
) -> Result<Value> {
    let cbu_info: Option<(String,)> = sqlx::query_as(
        r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1 AND deleted_at IS NULL"#,
    )
    .bind(cbu_id)
    .fetch_optional(scope.executor())
    .await?;
    let cbu_name = cbu_info.ok_or_else(|| anyhow!("CBU not found: {}", cbu_id))?.0;

    // Manual override first.
    #[derive(sqlx::FromRow)]
    struct OverrideRow {
        controller_entity_id: Uuid,
        justification: Option<String>,
        set_at: chrono::DateTime<chrono::Utc>,
    }

    let manual_override: Option<OverrideRow> = sqlx::query_as(
        r#"
        SELECT controller_entity_id, justification, set_at
        FROM "ob-poc".board_controller_overrides
        WHERE cbu_id = $1 AND cleared_at IS NULL
        ORDER BY set_at DESC
        LIMIT 1
        "#,
    )
    .bind(cbu_id)
    .fetch_optional(scope.executor())
    .await
    .unwrap_or(None);

    if let Some(o) = manual_override {
        let info: Option<(String, bool)> = sqlx::query_as(
            r#"
            SELECT e.name,
                   EXISTS(SELECT 1 FROM "ob-poc".entity_proper_persons pp WHERE pp.entity_id = e.entity_id)
            FROM "ob-poc".entities e
            WHERE e.entity_id = $1 AND e.deleted_at IS NULL
            "#,
        )
        .bind(o.controller_entity_id)
        .fetch_optional(scope.executor())
        .await?;

        let (name, is_natural) = info.unwrap_or(("Unknown".into(), false));
        let controller_type = if is_natural { "NATURAL_PERSON" } else { "LEGAL_ENTITY" };

        return Ok(json!({
            "cbu_id": cbu_id,
            "cbu_name": cbu_name,
            "board_controller_entity_id": o.controller_entity_id,
            "board_controller_name": name,
            "board_controller_type": controller_type,
            "confidence": "HIGH",
            "derivation_rule": "MANUAL_OVERRIDE",
            "derivation_explanation": format!("Manually set: {}", o.justification.unwrap_or_default()),
            "data_gaps": [],
            "evidence_sources": ["MANUAL"],
            "is_override": true,
            "computed_at": o.set_at.to_rfc3339(),
        }));
    }

    // Rule 1: majority appointer
    #[derive(sqlx::FromRow)]
    struct AppointerRow {
        appointer_id: Uuid,
        appointer_name: String,
        appointments: i64,
        total_board: i64,
    }

    let appointers: Vec<AppointerRow> = sqlx::query_as(
        r#"
        WITH cbu_entities AS (
            SELECT DISTINCT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
        ),
        board_analysis AS (
            SELECT
                bc.appointed_by_entity_id as appointer_id,
                e.name as appointer_name,
                COUNT(*) as appointments,
                (SELECT COUNT(*) FROM "ob-poc".board_compositions bc2
                 WHERE bc2.entity_id IN (SELECT entity_id FROM cbu_entities)
                 AND (bc2.ended_at IS NULL OR bc2.ended_at > CURRENT_DATE)) as total_board
            FROM "ob-poc".board_compositions bc
            JOIN "ob-poc".entities e ON bc.appointed_by_entity_id = e.entity_id
            WHERE bc.entity_id IN (SELECT entity_id FROM cbu_entities)
              AND bc.appointed_by_entity_id IS NOT NULL
              AND e.deleted_at IS NULL
              AND (bc.ended_at IS NULL OR bc.ended_at > CURRENT_DATE)
            GROUP BY bc.appointed_by_entity_id, e.name
        )
        SELECT appointer_id, appointer_name, appointments, total_board
        FROM board_analysis
        WHERE total_board > 0
        ORDER BY appointments DESC
        "#,
    )
    .bind(cbu_id)
    .fetch_all(scope.executor())
    .await
    .unwrap_or_default();

    let mut data_gaps: Vec<String> = Vec::new();
    let mut evidence_sources: Vec<&str> = Vec::new();

    if appointers.is_empty() {
        data_gaps.push("No board composition data found".into());
    }

    if let Some(top) = appointers.first() {
        if top.total_board > 0 {
            let ratio = top.appointments as f64 / top.total_board as f64;
            if ratio > 0.5 {
                let is_natural: bool = sqlx::query_scalar(
                    r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entity_proper_persons WHERE entity_id = $1)"#,
                )
                .bind(top.appointer_id)
                .fetch_one(scope.executor())
                .await?;
                let controller_type =
                    if is_natural { "NATURAL_PERSON" } else { "LEGAL_ENTITY" };
                evidence_sources.push("COMPUTED");

                return Ok(json!({
                    "cbu_id": cbu_id,
                    "cbu_name": cbu_name,
                    "board_controller_entity_id": top.appointer_id,
                    "board_controller_name": top.appointer_name,
                    "board_controller_type": controller_type,
                    "confidence": "HIGH",
                    "derivation_rule": "MAJORITY_APPOINTER",
                    "derivation_explanation": format!(
                        "{} appoints {} of {} board members ({}%)",
                        top.appointer_name, top.appointments, top.total_board,
                        (ratio * 100.0).round()
                    ),
                    "data_gaps": data_gaps,
                    "evidence_sources": evidence_sources,
                    "is_override": false,
                    "computed_at": chrono::Utc::now().to_rfc3339(),
                }));
            }
        }
    }

    // Rule 2: majority owner (>50%)
    #[derive(sqlx::FromRow)]
    struct OwnerRow {
        owner_id: Uuid,
        owner_name: String,
        percentage: rust_decimal::Decimal,
    }

    let majority_owner: Option<OwnerRow> = sqlx::query_as(
        r#"
        WITH cbu_entities AS (
            SELECT DISTINCT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
        )
        SELECT
            er.from_entity_id as owner_id,
            e.name as owner_name,
            er.percentage
        FROM "ob-poc".entity_relationships er
        JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
        WHERE er.to_entity_id IN (SELECT entity_id FROM cbu_entities)
          AND e.deleted_at IS NULL
          AND er.relationship_type IN ('ownership', 'control')
          AND er.percentage > 50
          AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
        ORDER BY er.percentage DESC
        LIMIT 1
        "#,
    )
    .bind(cbu_id)
    .fetch_optional(scope.executor())
    .await?;

    if let Some(o) = majority_owner {
        let is_natural: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entity_proper_persons WHERE entity_id = $1)"#,
        )
        .bind(o.owner_id)
        .fetch_one(scope.executor())
        .await?;
        let controller_type = if is_natural { "NATURAL_PERSON" } else { "LEGAL_ENTITY" };
        evidence_sources.push("COMPUTED");

        return Ok(json!({
            "cbu_id": cbu_id,
            "cbu_name": cbu_name,
            "board_controller_entity_id": o.owner_id,
            "board_controller_name": o.owner_name,
            "board_controller_type": controller_type,
            "confidence": "MEDIUM",
            "derivation_rule": "MAJORITY_OWNER",
            "derivation_explanation": format!(
                "{} owns {}% (>50% ownership implies board control)",
                o.owner_name, o.percentage
            ),
            "data_gaps": data_gaps,
            "evidence_sources": evidence_sources,
            "is_override": false,
            "computed_at": chrono::Utc::now().to_rfc3339(),
        }));
    }

    // Rule 3: GLEIF ultimate parent
    let gleif_parent: Option<(Uuid, String)> = sqlx::query_as(
        r#"
        WITH cbu_entities AS (
            SELECT DISTINCT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
        )
        SELECT er.from_entity_id, e.name
        FROM "ob-poc".entity_relationships er
        JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
        WHERE er.to_entity_id IN (SELECT entity_id FROM cbu_entities)
          AND e.deleted_at IS NULL
          AND er.source = 'GLEIF'
          AND er.relationship_type = 'control'
          AND er.control_type = 'ULTIMATE_ACCOUNTING_CONSOLIDATION'
          AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
        LIMIT 1
        "#,
    )
    .bind(cbu_id)
    .fetch_optional(scope.executor())
    .await
    .unwrap_or(None);

    if let Some((parent_id, parent_name)) = gleif_parent {
        let is_natural: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entity_proper_persons WHERE entity_id = $1)"#,
        )
        .bind(parent_id)
        .fetch_one(scope.executor())
        .await?;
        let controller_type = if is_natural { "NATURAL_PERSON" } else { "LEGAL_ENTITY" };
        evidence_sources.push("GLEIF");

        return Ok(json!({
            "cbu_id": cbu_id,
            "cbu_name": cbu_name,
            "board_controller_entity_id": parent_id,
            "board_controller_name": parent_name,
            "board_controller_type": controller_type,
            "confidence": "MEDIUM",
            "derivation_rule": "GLEIF_ULTIMATE_PARENT",
            "derivation_explanation": format!(
                "{} is GLEIF ultimate accounting consolidation parent",
                parent_name
            ),
            "data_gaps": data_gaps,
            "evidence_sources": evidence_sources,
            "is_override": false,
            "computed_at": chrono::Utc::now().to_rfc3339(),
        }));
    }

    data_gaps.push("No majority appointer found".into());
    data_gaps.push("No majority owner found".into());
    data_gaps.push("No GLEIF ultimate parent found".into());

    Ok(json!({
        "cbu_id": cbu_id,
        "cbu_name": cbu_name,
        "board_controller_entity_id": null,
        "board_controller_name": null,
        "board_controller_type": "UNKNOWN",
        "confidence": "LOW",
        "derivation_rule": "NONE",
        "derivation_explanation": "Unable to determine board controller from available data",
        "data_gaps": data_gaps,
        "evidence_sources": evidence_sources,
        "is_override": false,
        "computed_at": chrono::Utc::now().to_rfc3339(),
    }))
}

// ── control.show-board-controller ─────────────────────────────────────────────

pub struct ShowBoardController;

#[async_trait]
impl SemOsVerbOp for ShowBoardController {
    fn fqn(&self) -> &str {
        "control.show-board-controller"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let result = compute_show_board_controller(cbu_id, scope).await?;
        Ok(VerbExecutionOutcome::Record(result))
    }
}

// ── control.recompute-board-controller ────────────────────────────────────────

pub struct RecomputeBoardController;

#[async_trait]
impl SemOsVerbOp for RecomputeBoardController {
    fn fqn(&self) -> &str {
        "control.recompute-board-controller"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        let previous: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT controller_entity_id, controller_name
            FROM "ob-poc".board_controller_cache
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(scope.executor())
        .await
        .unwrap_or(None);

        let computed = compute_show_board_controller(cbu_id, scope).await?;

        let new_controller_id = computed
            .get("board_controller_entity_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());
        let new_controller_name = computed
            .get("board_controller_name")
            .and_then(|v| v.as_str())
            .map(String::from);
        let confidence = computed
            .get("confidence")
            .and_then(|v| v.as_str())
            .unwrap_or("LOW");
        let derivation_rule = computed
            .get("derivation_rule")
            .and_then(|v| v.as_str())
            .unwrap_or("NONE");

        let changed = match (&previous, &new_controller_id) {
            (Some((prev_id, _)), Some(new_id)) => prev_id != new_id,
            (None, Some(_)) | (Some(_), None) => true,
            (None, None) => false,
        };

        if let Some(controller_id) = new_controller_id {
            let _ = sqlx::query(
                r#"
                INSERT INTO "ob-poc".board_controller_cache
                    (cbu_id, controller_entity_id, controller_name, confidence, derivation_rule, computed_at)
                VALUES ($1, $2, $3, $4, $5, NOW())
                ON CONFLICT (cbu_id) DO UPDATE SET
                    controller_entity_id = EXCLUDED.controller_entity_id,
                    controller_name = EXCLUDED.controller_name,
                    confidence = EXCLUDED.confidence,
                    derivation_rule = EXCLUDED.derivation_rule,
                    computed_at = EXCLUDED.computed_at
                "#,
            )
            .bind(cbu_id)
            .bind(controller_id)
            .bind(&new_controller_name)
            .bind(confidence)
            .bind(derivation_rule)
            .execute(scope.executor())
            .await;
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "board_controller_entity_id": new_controller_id,
            "board_controller_name": new_controller_name,
            "confidence": confidence,
            "derivation_rule": derivation_rule,
            "previous_controller_entity_id": previous.as_ref().map(|(id, _)| id),
            "previous_controller_name": previous.as_ref().map(|(_, name)| name),
            "changed": changed,
            "recomputed_at": chrono::Utc::now().to_rfc3339(),
        })))
    }
}

// ── control.set-board-controller ──────────────────────────────────────────────

pub struct SetBoardController;

#[async_trait]
impl SemOsVerbOp for SetBoardController {
    fn fqn(&self) -> &str {
        "control.set-board-controller"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let controller_entity_id = json_extract_uuid(args, ctx, "controller-entity-id")?;
        let justification = json_extract_string(args, "justification")?;
        let evidence_doc_id = json_extract_uuid_opt(args, ctx, "evidence-doc-id");

        let entity_exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(
                SELECT 1 FROM "ob-poc".entities
                WHERE entity_id = $1 AND deleted_at IS NULL
            )"#,
        )
        .bind(controller_entity_id)
        .fetch_one(scope.executor())
        .await?;
        if !entity_exists {
            return Err(anyhow!("Entity not found: {}", controller_entity_id));
        }

        let _ = sqlx::query(
            r#"
            UPDATE "ob-poc".board_controller_overrides
            SET cleared_at = NOW()
            WHERE cbu_id = $1 AND cleared_at IS NULL
            "#,
        )
        .bind(cbu_id)
        .execute(scope.executor())
        .await;

        let override_id = Uuid::new_v4();
        let _ = sqlx::query(
            r#"
            INSERT INTO "ob-poc".board_controller_overrides
                (override_id, cbu_id, controller_entity_id, justification, evidence_doc_id, set_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            "#,
        )
        .bind(override_id)
        .bind(cbu_id)
        .bind(controller_entity_id)
        .bind(&justification)
        .bind(evidence_doc_id)
        .execute(scope.executor())
        .await;

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "board_controller_entity_id": controller_entity_id,
            "override_id": override_id,
            "justification": justification,
            "set_at": chrono::Utc::now().to_rfc3339(),
        })))
    }
}

// ── control.clear-board-controller-override ───────────────────────────────────

pub struct ClearBoardControllerOverride;

#[async_trait]
impl SemOsVerbOp for ClearBoardControllerOverride {
    fn fqn(&self) -> &str {
        "control.clear-board-controller-override"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;

        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".board_controller_overrides
            SET cleared_at = NOW()
            WHERE cbu_id = $1 AND cleared_at IS NULL
            "#,
        )
        .bind(cbu_id)
        .execute(scope.executor())
        .await;

        let override_cleared = result.map(|r| r.rows_affected() > 0).unwrap_or(false);

        let computed = compute_show_board_controller(cbu_id, scope).await.unwrap_or(json!({}));
        let computed_controller_id = computed
            .get("board_controller_entity_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "override_cleared": override_cleared,
            "now_using_computed": true,
            "computed_controller_entity_id": computed_controller_id,
        })))
    }
}

// ── control.import-psc-register ───────────────────────────────────────────────

pub struct ImportPscRegister;

#[async_trait]
impl SemOsVerbOp for ImportPscRegister {
    fn fqn(&self) -> &str {
        "control.import-psc-register"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let company_number = json_extract_string(args, "company-number")?;
        let source = json_extract_string_opt(args, "source")
            .unwrap_or_else(|| "COMPANIES_HOUSE".to_string());

        let existing: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM "ob-poc".entity_relationships er
            WHERE er.source = 'PSC_REGISTER'
              AND er.to_entity_id IN (
                  SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
              )
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(scope.executor())
        .await
        .unwrap_or(None);

        let pscs_imported = existing.map(|(c,)| c).unwrap_or(0);

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "company_number": company_number,
            "source": source,
            "pscs_imported": pscs_imported,
            "board_controller_updated": false,
            "message": "PSC import requires Companies House API integration",
            "imported_at": chrono::Utc::now().to_rfc3339(),
        })))
    }
}

// ── control.import-gleif-control ──────────────────────────────────────────────

pub struct ImportGleifControl;

#[async_trait]
impl SemOsVerbOp for ImportGleifControl {
    fn fqn(&self) -> &str {
        "control.import-gleif-control"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let lei = json_extract_string(args, "lei")?;
        let include_ultimate =
            json_extract_bool_opt(args, "include-ultimate-parent").unwrap_or(true);

        #[derive(sqlx::FromRow)]
        struct GleifDataRow {
            relationship_count: i64,
            has_direct_parent: bool,
            has_ultimate_parent: bool,
        }

        let existing: Option<GleifDataRow> = sqlx::query_as(
            r#"
            SELECT
                COUNT(*) as relationship_count,
                EXISTS(
                    SELECT 1 FROM "ob-poc".entity_relationships er
                    WHERE er.source = 'GLEIF'
                      AND er.control_type = 'DIRECT_CONSOLIDATION'
                      AND er.to_entity_id IN (SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1)
                ) as has_direct_parent,
                EXISTS(
                    SELECT 1 FROM "ob-poc".entity_relationships er
                    WHERE er.source = 'GLEIF'
                      AND er.control_type = 'ULTIMATE_ACCOUNTING_CONSOLIDATION'
                      AND er.to_entity_id IN (SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1)
                ) as has_ultimate_parent
            FROM "ob-poc".entity_relationships er
            WHERE er.source = 'GLEIF'
              AND er.to_entity_id IN (SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1)
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(scope.executor())
        .await
        .unwrap_or(None);

        let (rel_count, has_direct, has_ultimate) = existing
            .map(|r| (r.relationship_count, r.has_direct_parent, r.has_ultimate_parent))
            .unwrap_or((0, false, false));

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "lei": lei,
            "include_ultimate_parent": include_ultimate,
            "direct_parent_imported": has_direct,
            "ultimate_parent_imported": has_ultimate,
            "control_relationships_created": rel_count,
            "board_controller_updated": false,
            "message": "GLEIF import uses existing gleif.* verbs for data retrieval",
            "imported_at": chrono::Utc::now().to_rfc3339(),
        })))
    }
}
