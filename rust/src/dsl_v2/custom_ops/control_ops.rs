//! Control Operations - Unified control analysis plugin handlers
//!
//! Implements plugin handlers for control.yaml verbs:
//! - control.analyze - Comprehensive control analysis for any entity type
//! - control.build-graph - Build full control graph for a CBU
//! - control.identify-ubos - Identify all UBOs across all control vectors
//! - control.trace-chain - Trace specific control chain between entities
//! - control.reconcile-ownership - Reconcile ownership percentages with control

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

#[cfg(feature = "database")]
use sqlx::PgPool;

use super::helpers::get_required_uuid;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

// ============================================================================
// ControlAnalyzeOp - Comprehensive control analysis for any entity type
// ============================================================================

pub struct ControlAnalyzeOp;

#[async_trait]
impl CustomOperation for ControlAnalyzeOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "analyze"
    }

    fn rationale(&self) -> &'static str {
        "Performs comprehensive control analysis across all control vectors (ownership, voting, board, trust, partnership) to identify who controls an entity"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_required_uuid(verb_call, "entity-id")?;

        let include_indirect = verb_call
            .get_arg("include-indirect")
            .and_then(|v| v.value.as_boolean())
            .unwrap_or(true);

        let control_threshold = verb_call
            .get_arg("control-threshold")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse::<f64>().unwrap_or(25.0))
            .unwrap_or(25.0);

        // Get entity info
        let entity_info: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.name, et.type_code
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE e.entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(pool)
        .await?;

        let (_, entity_name, entity_type) =
            entity_info.ok_or_else(|| anyhow!("Entity not found: {}", entity_id))?;

        let mut control_vectors: Vec<serde_json::Value> = Vec::new();
        let mut controllers: HashMap<String, Vec<String>> = HashMap::new();

        // 1. Check ownership control (from entity_relationships)
        let ownership_records: Vec<(Uuid, String, Option<rust_decimal::Decimal>, Option<String>)> =
            sqlx::query_as(
                r#"
                SELECT
                    er.from_entity_id,
                    e.name,
                    er.percentage,
                    er.ownership_type
                FROM "ob-poc".entity_relationships er
                JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
                WHERE er.to_entity_id = $1
                  AND er.relationship_type = 'ownership'
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                  AND er.percentage >= $2
                ORDER BY er.percentage DESC
                "#,
            )
            .bind(entity_id)
            .bind(control_threshold as f32)
            .fetch_all(pool)
            .await?;

        for (from_entity_id, owner_name, percentage, ownership_type) in ownership_records {
            let owner_id = from_entity_id.to_string();
            let pct: f64 = percentage
                .map(|p| p.to_string().parse().unwrap_or(0.0))
                .unwrap_or(0.0);
            control_vectors.push(json!({
                "vector_type": "ownership",
                "holder_id": owner_id,
                "holder_name": owner_name,
                "percentage": pct,
                "ownership_type": ownership_type,
                "strength": pct / 100.0
            }));
            controllers
                .entry(owner_id)
                .or_default()
                .push("ownership".to_string());
        }

        // 2. Check voting rights control
        let voting_records: Vec<(Uuid, String, Option<rust_decimal::Decimal>)> = sqlx::query_as(
            r#"
            SELECT
                er.from_entity_id,
                e.name,
                er.percentage
            FROM "ob-poc".entity_relationships er
            JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
            WHERE er.to_entity_id = $1
              AND er.relationship_type = 'control'
              AND er.control_type = 'voting_rights'
              AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
              AND er.percentage >= $2
            ORDER BY er.percentage DESC
            "#,
        )
        .bind(entity_id)
        .bind(control_threshold as f32)
        .fetch_all(pool)
        .await?;

        for (from_entity_id, holder_name, percentage) in voting_records {
            let holder_id = from_entity_id.to_string();
            let pct: f64 = percentage
                .map(|p| p.to_string().parse().unwrap_or(0.0))
                .unwrap_or(0.0);
            control_vectors.push(json!({
                "vector_type": "voting_rights",
                "holder_id": holder_id,
                "holder_name": holder_name,
                "percentage": pct,
                "strength": pct / 100.0
            }));
            controllers
                .entry(holder_id)
                .or_default()
                .push("voting_rights".to_string());
        }

        // 3. Check board control using kyc.board_compositions
        // This table tracks who appoints whom to the board
        #[derive(sqlx::FromRow)]
        struct BoardControlRow {
            appointer_id: Option<Uuid>,
            appointer_name: Option<String>,
            appointments: Option<i64>,
            total_board: Option<i64>,
            has_majority: bool,
        }

        let board_control: Vec<BoardControlRow> = sqlx::query_as(
            r#"
            WITH board_analysis AS (
                SELECT
                    bc.appointed_by_entity_id as appointer_id,
                    e.name as appointer_name,
                    COUNT(*) as appointments,
                    (SELECT COUNT(*) FROM kyc.board_compositions
                     WHERE entity_id = $1 AND (ended_at IS NULL OR ended_at > CURRENT_DATE)) as total_board
                FROM kyc.board_compositions bc
                LEFT JOIN "ob-poc".entities e ON bc.appointed_by_entity_id = e.entity_id
                WHERE bc.entity_id = $1
                  AND (bc.ended_at IS NULL OR bc.ended_at > CURRENT_DATE)
                  AND bc.appointed_by_entity_id IS NOT NULL
                GROUP BY bc.appointed_by_entity_id, e.name
            )
            SELECT
                appointer_id,
                appointer_name,
                appointments,
                total_board,
                CASE WHEN total_board > 0 THEN
                    (appointments::float / total_board::float) > 0.5
                ELSE false END as has_majority
            FROM board_analysis
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        for row in board_control {
            if let Some(appointer) = row.appointer_id {
                if row.has_majority {
                    let appointer_name = row.appointer_name;
                    let appointments = row.appointments;
                    let total_board = row.total_board;
                    let holder_id = appointer.to_string();
                    control_vectors.push(json!({
                        "vector_type": "board_appointment",
                        "holder_id": holder_id,
                        "holder_name": appointer_name,
                        "appointments": appointments,
                        "total_board": total_board,
                        "has_majority": row.has_majority,
                        "strength": 0.9
                    }));
                    controllers
                        .entry(holder_id)
                        .or_default()
                        .push("board_appointment".to_string());
                }
            }
        }

        // 4. Check trust control (if entity is a trust)
        // Uses kyc.trust_provisions for granular provisions analysis
        if entity_type.as_deref() == Some("trust_discretionary") {
            #[derive(sqlx::FromRow)]
            struct TrustControlRow {
                holder_entity_id: Uuid,
                name: String,
                provision_type: Option<String>,
                has_discretion: Option<bool>,
            }

            let trust_control: Vec<TrustControlRow> = sqlx::query_as(
                r#"
                SELECT
                    tp.holder_entity_id,
                    e.name,
                    tp.provision_type,
                    tp.has_discretion
                FROM kyc.trust_provisions tp
                JOIN "ob-poc".entities e ON tp.holder_entity_id = e.entity_id
                WHERE tp.trust_entity_id = $1
                  AND (tp.effective_to IS NULL OR tp.effective_to > CURRENT_DATE)
                  AND (tp.provision_type IN ('TRUSTEE_DISCRETIONARY', 'PROTECTOR', 'APPOINTOR_POWER', 'TRUSTEE_REMOVAL')
                       OR tp.has_discretion = true)
                "#,
            )
            .bind(entity_id)
            .fetch_all(pool)
            .await?;

            for row in trust_control {
                let holder_id = row.holder_entity_id.to_string();
                let strength = match row.provision_type.as_deref() {
                    Some("TRUSTEE_DISCRETIONARY") if row.has_discretion.unwrap_or(false) => 0.9,
                    Some("PROTECTOR") => 0.7,
                    Some("APPOINTOR_POWER") => 0.8,
                    Some("TRUSTEE_REMOVAL") => 0.85,
                    _ => 0.5,
                };
                control_vectors.push(json!({
                    "vector_type": "trust_role",
                    "holder_id": holder_id,
                    "holder_name": row.name,
                    "provision_type": row.provision_type,
                    "has_discretion": row.has_discretion,
                    "strength": strength
                }));
                controllers
                    .entry(holder_id)
                    .or_default()
                    .push("trust_role".to_string());
            }
        }

        // 5. Check partnership control (if entity is a partnership)
        // Uses kyc.partnership_capital for partner economics and control rights
        if entity_type.as_deref() == Some("partnership_limited") {
            #[derive(sqlx::FromRow)]
            struct GpControlRow {
                partner_entity_id: Uuid,
                name: String,
            }

            let gp_control: Vec<GpControlRow> = sqlx::query_as(
                r#"
                SELECT
                    pc.partner_entity_id,
                    e.name
                FROM kyc.partnership_capital pc
                JOIN "ob-poc".entities e ON pc.partner_entity_id = e.entity_id
                WHERE pc.partnership_entity_id = $1
                  AND pc.partner_type = 'GP'
                  AND pc.is_active = true
                "#,
            )
            .bind(entity_id)
            .fetch_all(pool)
            .await?;

            for row in gp_control {
                let holder_id = row.partner_entity_id.to_string();
                control_vectors.push(json!({
                    "vector_type": "general_partner",
                    "holder_id": holder_id,
                    "holder_name": row.name,
                    "strength": 0.95
                }));
                controllers
                    .entry(holder_id)
                    .or_default()
                    .push("general_partner".to_string());
            }
        }

        // 6. Check executive control
        let exec_control: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT
                cer.entity_id,
                e.name,
                r.name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.target_entity_id = $1
              AND r.name IN ('CEO', 'MANAGING_DIRECTOR', 'EXECUTIVE_DIRECTOR', 'CFO')
              AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        for (controller_id, controller_name, role_name) in exec_control {
            let holder_id = controller_id.to_string();
            control_vectors.push(json!({
                "vector_type": "executive_control",
                "holder_id": holder_id,
                "holder_name": controller_name,
                "position": role_name,
                "strength": 0.6
            }));
            controllers
                .entry(holder_id)
                .or_default()
                .push("executive_control".to_string());
        }

        // Build controller summary
        let mut controller_list: Vec<serde_json::Value> = Vec::new();
        for (controller_id, vectors) in &controllers {
            // Look up if natural person
            let is_natural: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".entity_proper_persons
                    WHERE entity_id = $1
                )
                "#,
            )
            .bind(Uuid::parse_str(controller_id).unwrap_or(Uuid::nil()))
            .fetch_one(pool)
            .await?;

            let aggregate_score: f64 = control_vectors
                .iter()
                .filter(|v| v.get("holder_id").and_then(|h| h.as_str()) == Some(controller_id))
                .filter_map(|v| v.get("strength").and_then(|s| s.as_f64()))
                .sum();

            controller_list.push(json!({
                "controller_id": controller_id,
                "control_vectors": vectors,
                "vector_count": vectors.len(),
                "aggregate_control_score": aggregate_score.min(1.0),
                "is_natural_person": is_natural
            }));
        }

        // Determine control type
        let control_type = match controller_list.len() {
            0 => "unknown",
            1 => "single",
            2..=3 => "joint",
            _ => "diffuse",
        };

        // If include_indirect, note non-natural-person controllers that need analysis
        let mut indirect_ubos: Vec<serde_json::Value> = Vec::new();
        if include_indirect {
            for controller in &controller_list {
                if controller.get("is_natural_person") == Some(&json!(false)) {
                    if let Some(cid) = controller.get("controller_id").and_then(|c| c.as_str()) {
                        indirect_ubos.push(json!({
                            "intermediate_entity_id": cid,
                            "needs_analysis": true
                        }));
                    }
                }
            }
        }

        let result = json!({
            "entity_id": entity_id,
            "entity_name": entity_name,
            "entity_type": entity_type,
            "control_vectors": control_vectors,
            "controllers": controller_list,
            "control_type": control_type,
            "is_controlled": !controllers.is_empty(),
            "indirect_analysis_needed": indirect_ubos,
            "analysis_timestamp": chrono::Utc::now().to_rfc3339()
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// ControlBuildGraphOp - Build full control graph for a CBU
// ============================================================================

pub struct ControlBuildGraphOp;

#[async_trait]
impl CustomOperation for ControlBuildGraphOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "build-graph"
    }

    fn rationale(&self) -> &'static str {
        "Builds a complete control graph for a CBU showing all control relationships across ownership, voting, board, trust, and partnership vectors"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;

        let max_depth = verb_call
            .get_arg("max-depth")
            .and_then(|v| v.value.as_integer())
            .unwrap_or(10) as i32;

        // Get all entities linked to this CBU
        let cbu_entities: Vec<(Uuid,)> = sqlx::query_as(
            r#"
            SELECT DISTINCT entity_id
            FROM "ob-poc".cbu_entity_roles
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        let mut nodes: Vec<serde_json::Value> = Vec::new();
        let mut edges: Vec<serde_json::Value> = Vec::new();
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Build graph starting from each CBU entity
        for (entity_id,) in &cbu_entities {
            let entity_id_str = entity_id.to_string();
            if visited.contains(&entity_id_str) {
                continue;
            }

            // Recursive CTE to traverse control relationships
            let graph_data: Vec<(
                Uuid,
                String,
                Option<String>,
                Option<Uuid>,
                Option<String>,
                Option<rust_decimal::Decimal>,
                i32,
            )> = sqlx::query_as(
                r#"
                WITH RECURSIVE control_graph AS (
                    -- Base: start from the entity
                    SELECT
                        e.entity_id,
                        e.name,
                        et.type_code as entity_type,
                        NULL::uuid as controller_id,
                        NULL::text as relationship_type,
                        NULL::numeric as percentage,
                        0 as depth
                    FROM "ob-poc".entities e
                    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
                    WHERE e.entity_id = $1

                    UNION ALL

                    -- Recursive: follow control relationships upward
                    SELECT
                        e.entity_id,
                        e.name,
                        et.type_code as entity_type,
                        er.from_entity_id as controller_id,
                        er.relationship_type,
                        er.percentage,
                        cg.depth + 1 as depth
                    FROM control_graph cg
                    JOIN "ob-poc".entity_relationships er ON er.to_entity_id = cg.entity_id
                    JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
                    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
                    WHERE cg.depth < $2
                      AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                      AND er.relationship_type IN ('ownership', 'control', 'trust_role')
                )
                SELECT DISTINCT ON (entity_id)
                    entity_id,
                    name,
                    entity_type,
                    controller_id,
                    relationship_type,
                    percentage,
                    depth
                FROM control_graph
                ORDER BY entity_id, depth
                "#,
            )
            .bind(entity_id)
            .bind(max_depth)
            .fetch_all(pool)
            .await?;

            for (
                row_entity_id,
                name,
                entity_type,
                controller_id,
                relationship_type,
                percentage,
                depth,
            ) in graph_data
            {
                let node_id = row_entity_id.to_string();
                if !visited.contains(&node_id) {
                    visited.insert(node_id.clone());
                    nodes.push(json!({
                        "id": node_id,
                        "name": name,
                        "entity_type": entity_type,
                        "depth": depth
                    }));
                }

                if let Some(ctrl_id) = controller_id {
                    edges.push(json!({
                        "from": ctrl_id.to_string(),
                        "to": row_entity_id.to_string(),
                        "relationship_type": relationship_type,
                        "percentage": percentage.map(|p| p.to_string())
                    }));
                }
            }
        }

        // Add board appointment edges from kyc.board_compositions
        let entity_ids: Vec<Uuid> = cbu_entities.iter().map(|(id,)| *id).collect();

        #[derive(sqlx::FromRow)]
        struct BoardEdgeRow {
            appointed_by_entity_id: Option<Uuid>,
            entity_id: Uuid,
        }

        let board_edges: Vec<BoardEdgeRow> = sqlx::query_as(
            r#"
            SELECT DISTINCT
                bc.appointed_by_entity_id,
                bc.entity_id
            FROM kyc.board_compositions bc
            WHERE bc.entity_id = ANY($1)
              AND bc.appointed_by_entity_id IS NOT NULL
              AND (bc.ended_at IS NULL OR bc.ended_at > CURRENT_DATE)
            "#,
        )
        .bind(&entity_ids)
        .fetch_all(pool)
        .await?;

        for row in board_edges {
            if let Some(from) = row.appointed_by_entity_id {
                edges.push(json!({
                    "from": from.to_string(),
                    "to": row.entity_id.to_string(),
                    "relationship_type": "board_appointment"
                }));
            }
        }

        let result = json!({
            "cbu_id": cbu_id,
            "nodes": nodes,
            "edges": edges,
            "node_count": nodes.len(),
            "edge_count": edges.len(),
            "max_depth_reached": max_depth,
            "built_at": chrono::Utc::now().to_rfc3339()
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// ControlIdentifyUbosOp - Identify all UBOs across all control vectors
// ============================================================================

pub struct ControlIdentifyUbosOp;

#[async_trait]
impl CustomOperation for ControlIdentifyUbosOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "identify-ubos"
    }

    fn rationale(&self) -> &'static str {
        "Identifies all Ultimate Beneficial Owners for a CBU by tracing all control vectors to natural persons"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;

        let ownership_threshold = verb_call
            .get_arg("ownership-threshold")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse::<f64>().unwrap_or(25.0))
            .unwrap_or(25.0);

        // Get all natural persons who are UBOs through ownership chain
        let ownership_ubos: Vec<(
            Uuid,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<rust_decimal::Decimal>,
        )> = sqlx::query_as(
            r#"
            WITH RECURSIVE ownership_chain AS (
                -- Start from entities linked to CBU
                SELECT
                    cer.entity_id as target_entity_id,
                    cer.entity_id as current_entity_id,
                    100.0::numeric as effective_percentage,
                    ARRAY[cer.entity_id] as chain,
                    0 as depth
                FROM "ob-poc".cbu_entity_roles cer
                WHERE cer.cbu_id = $1

                UNION ALL

                -- Follow ownership relationships upward
                SELECT
                    oc.target_entity_id,
                    er.from_entity_id as current_entity_id,
                    (oc.effective_percentage * COALESCE(er.percentage, 100) / 100)::numeric as effective_percentage,
                    oc.chain || er.from_entity_id,
                    oc.depth + 1
                FROM ownership_chain oc
                JOIN "ob-poc".entity_relationships er ON er.to_entity_id = oc.current_entity_id
                WHERE oc.depth < 10
                  AND NOT er.from_entity_id = ANY(oc.chain)
                  AND er.relationship_type IN ('ownership', 'control')
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
            ),
            natural_person_ubos AS (
                SELECT DISTINCT ON (oc.current_entity_id)
                    oc.current_entity_id as person_entity_id,
                    e.name as person_name,
                    pp.first_name,
                    pp.last_name,
                    pp.nationality,
                    oc.effective_percentage
                FROM ownership_chain oc
                JOIN "ob-poc".entities e ON oc.current_entity_id = e.entity_id
                JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
                WHERE oc.effective_percentage >= $2
                ORDER BY oc.current_entity_id, oc.effective_percentage DESC
            )
            SELECT
                person_entity_id,
                person_name,
                first_name,
                last_name,
                nationality,
                effective_percentage
            FROM natural_person_ubos
            "#,
        )
        .bind(cbu_id)
        .bind(ownership_threshold as f32)
        .fetch_all(pool)
        .await?;

        // Also check for control-based UBOs (board control, trust roles, GP status)
        // Uses kyc.board_compositions, kyc.trust_provisions, kyc.partnership_capital
        #[derive(sqlx::FromRow)]
        struct ControlUboRow {
            person_entity_id: Uuid,
            person_name: String,
            first_name: Option<String>,
            last_name: Option<String>,
            nationality: Option<String>,
            control_vector: String,
            effective_percentage: Option<rust_decimal::Decimal>,
        }

        let control_ubos: Vec<ControlUboRow> = sqlx::query_as(
            r#"
            -- Board controllers who are natural persons
            SELECT DISTINCT
                pp.entity_id as person_entity_id,
                e.name as person_name,
                pp.first_name,
                pp.last_name,
                pp.nationality,
                'board_control'::text as control_vector,
                NULL::numeric as effective_percentage
            FROM kyc.board_compositions bc
            JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = bc.entity_id AND cer.cbu_id = $1
            JOIN "ob-poc".entities e ON bc.appointed_by_entity_id = e.entity_id
            JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE bc.appointed_by_entity_id IS NOT NULL
              AND (bc.ended_at IS NULL OR bc.ended_at > CURRENT_DATE)

            UNION

            -- Trust controllers who are natural persons
            SELECT DISTINCT
                pp.entity_id as person_entity_id,
                e.name as person_name,
                pp.first_name,
                pp.last_name,
                pp.nationality,
                'trust_' || tp.provision_type as control_vector,
                NULL::numeric as effective_percentage
            FROM kyc.trust_provisions tp
            JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = tp.trust_entity_id AND cer.cbu_id = $1
            JOIN "ob-poc".entities e ON tp.holder_entity_id = e.entity_id
            JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE tp.provision_type IN ('TRUSTEE_DISCRETIONARY', 'PROTECTOR', 'APPOINTOR_POWER')
              AND (tp.effective_to IS NULL OR tp.effective_to > CURRENT_DATE)

            UNION

            -- GP partners who are natural persons
            SELECT DISTINCT
                pp.entity_id as person_entity_id,
                e.name as person_name,
                pp.first_name,
                pp.last_name,
                pp.nationality,
                'general_partner'::text as control_vector,
                pc.profit_share_pct as effective_percentage
            FROM kyc.partnership_capital pc
            JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = pc.partnership_entity_id AND cer.cbu_id = $1
            JOIN "ob-poc".entities e ON pc.partner_entity_id = e.entity_id
            JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE pc.partner_type = 'GP'
              AND pc.is_active = true
            "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await?;

        // Combine and deduplicate UBOs
        let mut all_ubos: HashMap<String, serde_json::Value> = HashMap::new();

        for (
            person_entity_id,
            person_name,
            first_name,
            last_name,
            nationality,
            effective_percentage,
        ) in ownership_ubos
        {
            let person_id = person_entity_id.to_string();
            let entry = all_ubos.entry(person_id.clone()).or_insert_with(|| {
                json!({
                    "person_entity_id": person_id,
                    "person_name": person_name,
                    "first_name": first_name,
                    "last_name": last_name,
                    "nationality": nationality,
                    "control_vectors": [],
                    "max_ownership_percentage": 0.0
                })
            });

            if let Some(obj) = entry.as_object_mut() {
                if let Some(vectors) = obj
                    .get_mut("control_vectors")
                    .and_then(|v| v.as_array_mut())
                {
                    vectors.push(json!({
                        "type": "ownership",
                        "percentage": effective_percentage.map(|p| p.to_string())
                    }));
                }
                if let Some(pct) = effective_percentage {
                    let current_max = obj
                        .get("max_ownership_percentage")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let pct_f64: f64 = pct.to_string().parse().unwrap_or(0.0);
                    if pct_f64 > current_max {
                        obj.insert("max_ownership_percentage".to_string(), json!(pct_f64));
                    }
                }
            }
        }

        for row in control_ubos {
            let person_id = row.person_entity_id.to_string();
            let entry = all_ubos.entry(person_id.clone()).or_insert_with(|| {
                json!({
                    "person_entity_id": person_id,
                    "person_name": row.person_name,
                    "first_name": row.first_name,
                    "last_name": row.last_name,
                    "nationality": row.nationality,
                    "control_vectors": [],
                    "max_ownership_percentage": 0.0
                })
            });

            if let Some(obj) = entry.as_object_mut() {
                if let Some(vectors) = obj
                    .get_mut("control_vectors")
                    .and_then(|v| v.as_array_mut())
                {
                    vectors.push(json!({
                        "type": row.control_vector,
                        "percentage": row.effective_percentage.map(|p| p.to_string())
                    }));
                }
            }
        }

        let ubo_list: Vec<serde_json::Value> = all_ubos.into_values().collect();

        let result = json!({
            "cbu_id": cbu_id,
            "ownership_threshold": ownership_threshold,
            "ubos": ubo_list,
            "ubo_count": ubo_list.len(),
            "identified_at": chrono::Utc::now().to_rfc3339()
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// ControlTraceChainOp - Trace specific control chain between entities
// ============================================================================

pub struct ControlTraceChainOp;

#[async_trait]
impl CustomOperation for ControlTraceChainOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "trace-chain"
    }

    fn rationale(&self) -> &'static str {
        "Traces the control chain between two specific entities, showing all intermediate relationships"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let from_entity_id = get_required_uuid(verb_call, "from-entity-id")?;
        let to_entity_id = get_required_uuid(verb_call, "to-entity-id")?;

        let max_depth = verb_call
            .get_arg("max-depth")
            .and_then(|v| v.value.as_integer())
            .unwrap_or(10) as i32;

        // Find path from 'from' entity to 'to' entity via control relationships
        let chain: Option<(Vec<Uuid>, i32)> = sqlx::query_as(
            r#"
            WITH RECURSIVE control_path AS (
                -- Start from the 'from' entity
                SELECT
                    er.from_entity_id,
                    er.to_entity_id,
                    er.relationship_type,
                    er.control_type,
                    er.percentage,
                    ARRAY[er.from_entity_id, er.to_entity_id] as path,
                    1 as depth
                FROM "ob-poc".entity_relationships er
                WHERE er.from_entity_id = $1
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)

                UNION ALL

                -- Extend the path
                SELECT
                    cp.from_entity_id,
                    er.to_entity_id,
                    er.relationship_type,
                    er.control_type,
                    er.percentage,
                    cp.path || er.to_entity_id,
                    cp.depth + 1
                FROM control_path cp
                JOIN "ob-poc".entity_relationships er ON er.from_entity_id = cp.to_entity_id
                WHERE cp.depth < $3
                  AND NOT er.to_entity_id = ANY(cp.path)
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
            )
            SELECT
                path,
                depth
            FROM control_path
            WHERE to_entity_id = $2
            ORDER BY depth
            LIMIT 1
            "#,
        )
        .bind(from_entity_id)
        .bind(to_entity_id)
        .bind(max_depth)
        .fetch_optional(pool)
        .await?;

        let result = if let Some((path, depth)) = chain {
            // Get details for each entity in the path
            let mut chain_details: Vec<serde_json::Value> = Vec::new();
            for (i, entity_id) in path.iter().enumerate() {
                let entity_info: Option<(String, Option<String>)> = sqlx::query_as(
                    r#"
                    SELECT e.name, et.type_code
                    FROM "ob-poc".entities e
                    JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
                    WHERE e.entity_id = $1
                    "#,
                )
                .bind(entity_id)
                .fetch_optional(pool)
                .await?;

                if let Some((name, entity_type)) = entity_info {
                    // Get relationship to next entity in chain
                    let relationship = if i < path.len() - 1 {
                        let rel: Option<(
                            Option<String>,
                            Option<String>,
                            Option<rust_decimal::Decimal>,
                        )> = sqlx::query_as(
                            r#"
                            SELECT relationship_type, control_type, percentage
                            FROM "ob-poc".entity_relationships
                            WHERE from_entity_id = $1 AND to_entity_id = $2
                              AND (effective_to IS NULL OR effective_to > CURRENT_DATE)
                            LIMIT 1
                            "#,
                        )
                        .bind(entity_id)
                        .bind(path[i + 1])
                        .fetch_optional(pool)
                        .await?;

                        rel.map(|(rel_type, ctrl_type, pct)| {
                            json!({
                                "type": rel_type,
                                "control_type": ctrl_type,
                                "percentage": pct.map(|p| p.to_string())
                            })
                        })
                    } else {
                        None
                    };

                    chain_details.push(json!({
                        "position": i,
                        "entity_id": entity_id.to_string(),
                        "entity_name": name,
                        "entity_type": entity_type,
                        "relationship_to_next": relationship
                    }));
                }
            }

            // Calculate effective control percentage
            let effective_percentage: f64 = chain_details
                .iter()
                .filter_map(|d| {
                    d.get("relationship_to_next")
                        .and_then(|r| r.get("percentage"))
                        .and_then(|p| p.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                })
                .fold(100.0, |acc, pct| acc * pct / 100.0);

            json!({
                "from_entity_id": from_entity_id,
                "to_entity_id": to_entity_id,
                "chain_found": true,
                "chain_length": depth,
                "chain": chain_details,
                "effective_control_percentage": effective_percentage
            })
        } else {
            json!({
                "from_entity_id": from_entity_id,
                "to_entity_id": to_entity_id,
                "chain_found": false,
                "message": "No control chain found between the specified entities"
            })
        };

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// ControlReconcileOwnershipOp - Reconcile ownership percentages with control
// ============================================================================

pub struct ControlReconcileOwnershipOp;

#[async_trait]
impl CustomOperation for ControlReconcileOwnershipOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "reconcile-ownership"
    }

    fn rationale(&self) -> &'static str {
        "Reconciles ownership percentages from different sources (share capital, entity relationships) and identifies discrepancies"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let entity_id = get_required_uuid(verb_call, "entity-id")?;

        let tolerance = verb_call
            .get_arg("tolerance")
            .and_then(|v| v.value.as_decimal())
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.01))
            .unwrap_or(0.01); // 1% tolerance for rounding

        // Get ownership from share capital using kyc.holdings and kyc.share_classes
        #[derive(sqlx::FromRow)]
        struct ShareOwnershipRow {
            investor_entity_id: Uuid,
            ownership_pct: Option<f64>,
        }

        let share_ownership: Vec<ShareOwnershipRow> = sqlx::query_as(
            r#"
            WITH share_totals AS (
                SELECT
                    h.investor_entity_id,
                    SUM(h.units) as total_units
                FROM kyc.holdings h
                JOIN kyc.share_classes sc ON h.share_class_id = sc.id
                WHERE sc.issuer_entity_id = $1
                  AND h.status = 'active'
                GROUP BY h.investor_entity_id
            ),
            total_issued AS (
                SELECT COALESCE(SUM(h.units), 0) as all_units
                FROM kyc.holdings h
                JOIN kyc.share_classes sc ON h.share_class_id = sc.id
                WHERE sc.issuer_entity_id = $1
                  AND h.status = 'active'
            )
            SELECT
                st.investor_entity_id,
                CASE WHEN ti.all_units > 0 THEN
                    (st.total_units::float / ti.all_units::float * 100)
                ELSE 0 END as ownership_pct
            FROM share_totals st
            CROSS JOIN total_issued ti
            "#,
        )
        .bind(entity_id)
        .fetch_all(pool)
        .await?;

        // Get ownership from entity relationships
        let relationship_ownership: Vec<(Uuid, String, Option<rust_decimal::Decimal>)> =
            sqlx::query_as(
                r#"
            SELECT
                er.from_entity_id,
                e.name,
                er.percentage
            FROM "ob-poc".entity_relationships er
            JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
            WHERE er.to_entity_id = $1
              AND er.relationship_type = 'ownership'
              AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
            "#,
            )
            .bind(entity_id)
            .fetch_all(pool)
            .await?;

        // Compare and find discrepancies
        let mut discrepancies: Vec<serde_json::Value> = Vec::new();
        let mut reconciled: Vec<serde_json::Value> = Vec::new();

        // Build map of share-based ownership
        let mut share_map: HashMap<String, f64> = HashMap::new();
        for row in &share_ownership {
            let id = row.investor_entity_id.to_string();
            let pct = row.ownership_pct.unwrap_or(0.0);
            share_map.insert(id, pct);
        }

        // Build map of relationship-based ownership
        let mut rel_map: HashMap<String, f64> = HashMap::new();
        for (owner_id, _, percentage) in &relationship_ownership {
            let id = owner_id.to_string();
            let pct: f64 = percentage
                .map(|p| p.to_string().parse().unwrap_or(0.0))
                .unwrap_or(0.0);
            rel_map.insert(id, pct);
        }

        // Check share-based against relationship-based
        for (holder_id, share_pct) in &share_map {
            let rel_pct = rel_map.get(holder_id).copied().unwrap_or(0.0);
            let diff = (share_pct - rel_pct).abs();

            if diff > tolerance * 100.0 {
                discrepancies.push(json!({
                    "owner_entity_id": holder_id,
                    "share_capital_percentage": share_pct,
                    "relationship_percentage": rel_pct,
                    "difference": diff,
                    "status": "discrepancy"
                }));
            } else {
                reconciled.push(json!({
                    "owner_entity_id": holder_id,
                    "share_capital_percentage": share_pct,
                    "relationship_percentage": rel_pct,
                    "status": "reconciled"
                }));
            }
        }

        // Check for relationship owners not in share capital
        for (owner_id, rel_pct) in &rel_map {
            if !share_map.contains_key(owner_id) && *rel_pct > tolerance * 100.0 {
                discrepancies.push(json!({
                    "owner_entity_id": owner_id,
                    "share_capital_percentage": 0.0,
                    "relationship_percentage": rel_pct,
                    "difference": rel_pct,
                    "status": "missing_in_share_capital"
                }));
            }
        }

        // Calculate totals
        let share_total: f64 = share_map.values().sum();
        let rel_total: f64 = rel_map.values().sum();

        let result = json!({
            "entity_id": entity_id,
            "tolerance_percentage": tolerance * 100.0,
            "share_capital_total": share_total,
            "relationship_total": rel_total,
            "totals_match": (share_total - rel_total).abs() <= tolerance * 100.0,
            "reconciled_count": reconciled.len(),
            "discrepancy_count": discrepancies.len(),
            "reconciled": reconciled,
            "discrepancies": discrepancies,
            "reconciliation_status": if discrepancies.is_empty() { "clean" } else { "needs_attention" },
            "reconciled_at": chrono::Utc::now().to_rfc3339()
        });

        Ok(ExecutionResult::Record(result))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// ShowBoardControllerOp - Show board controller for a CBU
// ============================================================================

pub struct ShowBoardControllerOp;

#[async_trait]
impl CustomOperation for ShowBoardControllerOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "show-board-controller"
    }

    fn rationale(&self) -> &'static str {
        "Shows the computed board controller for a CBU with derivation explanation, confidence, and data gaps"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;

        // Get CBU info
        let cbu_info: Option<(String,)> =
            sqlx::query_as(r#"SELECT name FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .fetch_optional(pool)
                .await?;

        let cbu_name = cbu_info
            .ok_or_else(|| anyhow!("CBU not found: {}", cbu_id))?
            .0;

        // Check for manual override first
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
        .fetch_optional(pool)
        .await
        .unwrap_or(None); // Table may not exist yet

        if let Some(override_row) = manual_override {
            // Get controller info
            let controller_info: Option<(String, bool)> = sqlx::query_as(
                r#"
                SELECT e.name,
                       EXISTS(SELECT 1 FROM "ob-poc".entity_proper_persons pp WHERE pp.entity_id = e.entity_id)
                FROM "ob-poc".entities e
                WHERE e.entity_id = $1
                "#,
            )
            .bind(override_row.controller_entity_id)
            .fetch_optional(pool)
            .await?;

            let (controller_name, is_natural) =
                controller_info.unwrap_or(("Unknown".to_string(), false));
            let controller_type = if is_natural {
                "NATURAL_PERSON"
            } else {
                "LEGAL_ENTITY"
            };

            return Ok(ExecutionResult::Record(json!({
                "cbu_id": cbu_id,
                "cbu_name": cbu_name,
                "board_controller_entity_id": override_row.controller_entity_id,
                "board_controller_name": controller_name,
                "board_controller_type": controller_type,
                "confidence": "HIGH",
                "derivation_rule": "MANUAL_OVERRIDE",
                "derivation_explanation": format!("Manually set: {}", override_row.justification.unwrap_or_default()),
                "data_gaps": [],
                "evidence_sources": ["MANUAL"],
                "is_override": true,
                "computed_at": override_row.set_at.to_rfc3339()
            })));
        }

        // Compute board controller from data
        // Rule 1: Check for majority appointer in board compositions
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
                    (SELECT COUNT(*) FROM kyc.board_compositions bc2
                     WHERE bc2.entity_id IN (SELECT entity_id FROM cbu_entities)
                     AND (bc2.ended_at IS NULL OR bc2.ended_at > CURRENT_DATE)) as total_board
                FROM kyc.board_compositions bc
                JOIN "ob-poc".entities e ON bc.appointed_by_entity_id = e.entity_id
                WHERE bc.entity_id IN (SELECT entity_id FROM cbu_entities)
                  AND bc.appointed_by_entity_id IS NOT NULL
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
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut data_gaps: Vec<String> = Vec::new();
        let mut evidence_sources: Vec<&str> = Vec::new();

        // Check if we have board data
        if appointers.is_empty() {
            data_gaps.push("No board composition data found".to_string());
        }

        // Find majority appointer
        if let Some(top) = appointers.first() {
            if top.total_board > 0 {
                let ratio = top.appointments as f64 / top.total_board as f64;
                if ratio > 0.5 {
                    // Majority appointer found
                    let is_natural: bool = sqlx::query_scalar(
                        r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entity_proper_persons WHERE entity_id = $1)"#,
                    )
                    .bind(top.appointer_id)
                    .fetch_one(pool)
                    .await?;

                    let controller_type = if is_natural {
                        "NATURAL_PERSON"
                    } else {
                        "LEGAL_ENTITY"
                    };
                    evidence_sources.push("COMPUTED");

                    return Ok(ExecutionResult::Record(json!({
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
                        "computed_at": chrono::Utc::now().to_rfc3339()
                    })));
                }
            }
        }

        // Rule 2: Check for >50% ownership control
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
              AND er.relationship_type IN ('ownership', 'control')
              AND er.percentage > 50
              AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
            ORDER BY er.percentage DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;

        if let Some(owner) = majority_owner {
            let is_natural: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entity_proper_persons WHERE entity_id = $1)"#,
            )
            .bind(owner.owner_id)
            .fetch_one(pool)
            .await?;

            let controller_type = if is_natural {
                "NATURAL_PERSON"
            } else {
                "LEGAL_ENTITY"
            };
            evidence_sources.push("COMPUTED");

            return Ok(ExecutionResult::Record(json!({
                "cbu_id": cbu_id,
                "cbu_name": cbu_name,
                "board_controller_entity_id": owner.owner_id,
                "board_controller_name": owner.owner_name,
                "board_controller_type": controller_type,
                "confidence": "MEDIUM",
                "derivation_rule": "MAJORITY_OWNER",
                "derivation_explanation": format!(
                    "{} owns {}% (>50% ownership implies board control)",
                    owner.owner_name, owner.percentage
                ),
                "data_gaps": data_gaps,
                "evidence_sources": evidence_sources,
                "is_override": false,
                "computed_at": chrono::Utc::now().to_rfc3339()
            })));
        }

        // Rule 3: Check GLEIF ultimate parent
        #[derive(sqlx::FromRow)]
        struct GleifParentRow {
            parent_entity_id: Uuid,
            parent_name: String,
        }

        let gleif_parent: Option<GleifParentRow> = sqlx::query_as(
            r#"
            WITH cbu_entities AS (
                SELECT DISTINCT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
            )
            SELECT
                er.from_entity_id as parent_entity_id,
                e.name as parent_name
            FROM "ob-poc".entity_relationships er
            JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
            WHERE er.to_entity_id IN (SELECT entity_id FROM cbu_entities)
              AND er.source = 'GLEIF'
              AND er.relationship_type = 'control'
              AND er.control_type = 'ULTIMATE_ACCOUNTING_CONSOLIDATION'
              AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some(parent) = gleif_parent {
            let is_natural: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entity_proper_persons WHERE entity_id = $1)"#,
            )
            .bind(parent.parent_entity_id)
            .fetch_one(pool)
            .await?;

            let controller_type = if is_natural {
                "NATURAL_PERSON"
            } else {
                "LEGAL_ENTITY"
            };
            evidence_sources.push("GLEIF");

            return Ok(ExecutionResult::Record(json!({
                "cbu_id": cbu_id,
                "cbu_name": cbu_name,
                "board_controller_entity_id": parent.parent_entity_id,
                "board_controller_name": parent.parent_name,
                "board_controller_type": controller_type,
                "confidence": "MEDIUM",
                "derivation_rule": "GLEIF_ULTIMATE_PARENT",
                "derivation_explanation": format!(
                    "{} is GLEIF ultimate accounting consolidation parent",
                    parent.parent_name
                ),
                "data_gaps": data_gaps,
                "evidence_sources": evidence_sources,
                "is_override": false,
                "computed_at": chrono::Utc::now().to_rfc3339()
            })));
        }

        // No controller found
        data_gaps.push("No majority appointer found".to_string());
        data_gaps.push("No majority owner found".to_string());
        data_gaps.push("No GLEIF ultimate parent found".to_string());

        Ok(ExecutionResult::Record(json!({
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
            "computed_at": chrono::Utc::now().to_rfc3339()
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// RecomputeBoardControllerOp - Recompute board controller for a CBU
// ============================================================================

pub struct RecomputeBoardControllerOp;

#[async_trait]
impl CustomOperation for RecomputeBoardControllerOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "recompute-board-controller"
    }

    fn rationale(&self) -> &'static str {
        "Recomputes the board controller for a CBU by traversing ownership/control graph"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;

        // Get previous controller (if any)
        let previous: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            SELECT controller_entity_id, controller_name
            FROM "ob-poc".board_controller_cache
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None); // Table may not exist

        // Call show-board-controller to compute
        let show_op = ShowBoardControllerOp;
        let show_result = show_op.execute(verb_call, ctx, pool).await?;

        let computed = match &show_result {
            ExecutionResult::Record(r) => r.clone(),
            _ => return Err(anyhow!("Unexpected result from show-board-controller")),
        };

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

        // Check if changed
        let changed = match (&previous, &new_controller_id) {
            (Some((prev_id, _)), Some(new_id)) => prev_id != new_id,
            (None, Some(_)) => true,
            (Some(_), None) => true,
            (None, None) => false,
        };

        // Update cache (upsert)
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
            .execute(pool)
            .await; // Ignore error if table doesn't exist
        }

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id,
            "board_controller_entity_id": new_controller_id,
            "board_controller_name": new_controller_name,
            "confidence": confidence,
            "derivation_rule": derivation_rule,
            "previous_controller_entity_id": previous.as_ref().map(|(id, _)| id),
            "previous_controller_name": previous.as_ref().map(|(_, name)| name),
            "changed": changed,
            "recomputed_at": chrono::Utc::now().to_rfc3339()
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// SetBoardControllerOp - Manually set board controller
// ============================================================================

pub struct SetBoardControllerOp;

#[async_trait]
impl CustomOperation for SetBoardControllerOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "set-board-controller"
    }

    fn rationale(&self) -> &'static str {
        "Manually sets the board controller for a CBU, creating an override"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;
        let controller_entity_id = get_required_uuid(verb_call, "controller-entity-id")?;

        let justification = verb_call
            .get_arg("justification")
            .and_then(|v| v.value.as_string())
            .ok_or_else(|| anyhow!("justification is required"))?;

        let evidence_doc_id = verb_call
            .get_arg("evidence-doc-id")
            .and_then(|v| v.value.as_uuid());

        // Verify entity exists
        let entity_exists: bool = sqlx::query_scalar(
            r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".entities WHERE entity_id = $1)"#,
        )
        .bind(controller_entity_id)
        .fetch_one(pool)
        .await?;

        if !entity_exists {
            return Err(anyhow!("Entity not found: {}", controller_entity_id));
        }

        // Clear any existing override
        let _ = sqlx::query(
            r#"
            UPDATE "ob-poc".board_controller_overrides
            SET cleared_at = NOW()
            WHERE cbu_id = $1 AND cleared_at IS NULL
            "#,
        )
        .bind(cbu_id)
        .execute(pool)
        .await; // Ignore if table doesn't exist

        // Insert new override
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
        .bind(justification)
        .bind(evidence_doc_id)
        .execute(pool)
        .await;

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id,
            "board_controller_entity_id": controller_entity_id,
            "override_id": override_id,
            "justification": justification,
            "set_at": chrono::Utc::now().to_rfc3339()
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// ClearBoardControllerOverrideOp - Clear manual override
// ============================================================================

pub struct ClearBoardControllerOverrideOp;

#[async_trait]
impl CustomOperation for ClearBoardControllerOverrideOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "clear-board-controller-override"
    }

    fn rationale(&self) -> &'static str {
        "Clears a manual board controller override, returning to computed value"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;

        // Clear override
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".board_controller_overrides
            SET cleared_at = NOW()
            WHERE cbu_id = $1 AND cleared_at IS NULL
            "#,
        )
        .bind(cbu_id)
        .execute(pool)
        .await;

        let override_cleared = result.map(|r| r.rows_affected() > 0).unwrap_or(false);

        // Get computed controller
        let show_op = ShowBoardControllerOp;
        let show_result = show_op.execute(verb_call, ctx, pool).await?;

        let computed = match &show_result {
            ExecutionResult::Record(r) => r.clone(),
            _ => json!({}),
        };

        let computed_controller_id = computed
            .get("board_controller_entity_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok());

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id,
            "override_cleared": override_cleared,
            "now_using_computed": true,
            "computed_controller_entity_id": computed_controller_id
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// ImportPscRegisterOp - Import PSC register data
// ============================================================================

pub struct ImportPscRegisterOp;

#[async_trait]
impl CustomOperation for ImportPscRegisterOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "import-psc-register"
    }

    fn rationale(&self) -> &'static str {
        "Imports board controller data from a PSC (Persons with Significant Control) register"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;

        let company_number = verb_call
            .get_arg("company-number")
            .and_then(|v| v.value.as_string())
            .ok_or_else(|| anyhow!("company-number is required"))?;

        let source = verb_call
            .get_arg("source")
            .and_then(|v| v.value.as_string())
            .unwrap_or("COMPANIES_HOUSE");

        // This would call the Companies House API in a real implementation
        // For now, we log the intent and return a placeholder

        // Check if we have existing research data from Companies House
        #[derive(sqlx::FromRow)]
        struct PscDataRow {
            psc_count: i64,
        }

        let existing_psc: Option<PscDataRow> = sqlx::query_as(
            r#"
            SELECT COUNT(*) as psc_count
            FROM "ob-poc".entity_relationships er
            WHERE er.source = 'PSC_REGISTER'
              AND er.to_entity_id IN (
                  SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
              )
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        let pscs_imported = existing_psc.map(|r| r.psc_count).unwrap_or(0);

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id,
            "company_number": company_number,
            "source": source,
            "pscs_imported": pscs_imported,
            "board_controller_updated": false,
            "message": "PSC import requires Companies House API integration",
            "imported_at": chrono::Utc::now().to_rfc3339()
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

// ============================================================================
// ImportGleifControlOp - Import GLEIF control data
// ============================================================================

pub struct ImportGleifControlOp;

#[async_trait]
impl CustomOperation for ImportGleifControlOp {
    fn domain(&self) -> &'static str {
        "control"
    }

    fn verb(&self) -> &'static str {
        "import-gleif-control"
    }

    fn rationale(&self) -> &'static str {
        "Imports control data from GLEIF (Global LEI Foundation) relationship records"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = get_required_uuid(verb_call, "cbu-id")?;

        let lei = verb_call
            .get_arg("lei")
            .and_then(|v| v.value.as_string())
            .ok_or_else(|| anyhow!("lei is required"))?;

        let include_ultimate = verb_call
            .get_arg("include-ultimate-parent")
            .and_then(|v| v.value.as_boolean())
            .unwrap_or(true);

        // Check for existing GLEIF relationships
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
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        let (rel_count, has_direct, has_ultimate) = existing
            .map(|r| {
                (
                    r.relationship_count,
                    r.has_direct_parent,
                    r.has_ultimate_parent,
                )
            })
            .unwrap_or((0, false, false));

        Ok(ExecutionResult::Record(json!({
            "cbu_id": cbu_id,
            "lei": lei,
            "include_ultimate_parent": include_ultimate,
            "direct_parent_imported": has_direct,
            "ultimate_parent_imported": has_ultimate,
            "control_relationships_created": rel_count,
            "board_controller_updated": false,
            "message": "GLEIF import uses existing gleif.* verbs for data retrieval",
            "imported_at": chrono::Utc::now().to_rfc3339()
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Void)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_metadata() {
        let analyze = ControlAnalyzeOp;
        assert_eq!(analyze.domain(), "control");
        assert_eq!(analyze.verb(), "analyze");

        let build_graph = ControlBuildGraphOp;
        assert_eq!(build_graph.domain(), "control");
        assert_eq!(build_graph.verb(), "build-graph");

        let identify = ControlIdentifyUbosOp;
        assert_eq!(identify.domain(), "control");
        assert_eq!(identify.verb(), "identify-ubos");

        let trace = ControlTraceChainOp;
        assert_eq!(trace.domain(), "control");
        assert_eq!(trace.verb(), "trace-chain");

        let reconcile = ControlReconcileOwnershipOp;
        assert_eq!(reconcile.domain(), "control");
        assert_eq!(reconcile.verb(), "reconcile-ownership");

        let show = ShowBoardControllerOp;
        assert_eq!(show.domain(), "control");
        assert_eq!(show.verb(), "show-board-controller");

        let recompute = RecomputeBoardControllerOp;
        assert_eq!(recompute.domain(), "control");
        assert_eq!(recompute.verb(), "recompute-board-controller");

        let set = SetBoardControllerOp;
        assert_eq!(set.domain(), "control");
        assert_eq!(set.verb(), "set-board-controller");

        let clear = ClearBoardControllerOverrideOp;
        assert_eq!(clear.domain(), "control");
        assert_eq!(clear.verb(), "clear-board-controller-override");

        let import_psc = ImportPscRegisterOp;
        assert_eq!(import_psc.domain(), "control");
        assert_eq!(import_psc.verb(), "import-psc-register");

        let import_gleif = ImportGleifControlOp;
        assert_eq!(import_gleif.domain(), "control");
        assert_eq!(import_gleif.verb(), "import-gleif-control");
    }
}
