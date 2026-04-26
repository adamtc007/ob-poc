//! Graph-level control analysis verbs (5).
//!
//! - `control.analyze` — multi-vector controller analysis (ownership,
//!   voting, board, trust, partnership, executive).
//! - `control.build-graph` — recursive control graph for a CBU.
//! - `control.identify-ubos` — natural-person UBOs via ownership
//!   chain + non-ownership vectors (board / trust / GP).
//! - `control.trace-chain` — path-finding between two entities
//!   via control edges.
//! - `control.reconcile-ownership` — share-capital vs
//!   entity_relationships percentage reconciliation.

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::json_extract_uuid;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use crate::ops::SemOsVerbOp;

// ── control.analyze ───────────────────────────────────────────────────────────

pub struct ControlAnalyze;

#[async_trait]
impl SemOsVerbOp for ControlAnalyze {
    fn fqn(&self) -> &str {
        "control.analyze"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let include_indirect = args
            .get("include-indirect")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let control_threshold = args
            .get("control-threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(25.0);

        let entity_info: Option<(Uuid, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT e.entity_id, e.name, et.type_code
            FROM "ob-poc".entities e
            JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
            WHERE e.entity_id = $1
              AND e.deleted_at IS NULL
            "#,
        )
        .bind(entity_id)
        .fetch_optional(scope.executor())
        .await?;

        let (_, entity_name, entity_type) =
            entity_info.ok_or_else(|| anyhow!("Entity not found: {}", entity_id))?;

        let mut control_vectors: Vec<Value> = Vec::new();
        let mut controllers: HashMap<String, Vec<String>> = HashMap::new();

        // 1. Ownership control
        let ownership: Vec<(Uuid, String, Option<rust_decimal::Decimal>, Option<String>)> =
            sqlx::query_as(
                r#"
                SELECT er.from_entity_id, e.name, er.percentage, er.ownership_type
                FROM "ob-poc".entity_relationships er
                JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
                WHERE er.to_entity_id = $1
                  AND e.deleted_at IS NULL
                  AND er.relationship_type = 'ownership'
                  AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                  AND er.percentage >= $2
                ORDER BY er.percentage DESC
                "#,
            )
            .bind(entity_id)
            .bind(control_threshold as f32)
            .fetch_all(scope.executor())
            .await?;

        for (from_id, owner_name, pct, own_type) in ownership {
            let owner_id = from_id.to_string();
            let pct_f: f64 = pct
                .map(|p| p.to_string().parse().unwrap_or(0.0))
                .unwrap_or(0.0);
            control_vectors.push(json!({
                "vector_type": "ownership",
                "holder_id": owner_id,
                "holder_name": owner_name,
                "percentage": pct_f,
                "ownership_type": own_type,
                "strength": pct_f / 100.0,
            }));
            controllers
                .entry(owner_id)
                .or_default()
                .push("ownership".into());
        }

        // 2. Voting rights
        let voting: Vec<(Uuid, String, Option<rust_decimal::Decimal>)> = sqlx::query_as(
            r#"
            SELECT er.from_entity_id, e.name, er.percentage
            FROM "ob-poc".entity_relationships er
            JOIN "ob-poc".entities e ON er.from_entity_id = e.entity_id
            WHERE er.to_entity_id = $1
              AND e.deleted_at IS NULL
              AND er.relationship_type = 'control'
              AND er.control_type = 'voting_rights'
              AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
              AND er.percentage >= $2
            ORDER BY er.percentage DESC
            "#,
        )
        .bind(entity_id)
        .bind(control_threshold as f32)
        .fetch_all(scope.executor())
        .await?;

        for (from_id, name, pct) in voting {
            let holder_id = from_id.to_string();
            let pct_f: f64 = pct
                .map(|p| p.to_string().parse().unwrap_or(0.0))
                .unwrap_or(0.0);
            control_vectors.push(json!({
                "vector_type": "voting_rights",
                "holder_id": holder_id,
                "holder_name": name,
                "percentage": pct_f,
                "strength": pct_f / 100.0,
            }));
            controllers
                .entry(holder_id)
                .or_default()
                .push("voting_rights".into());
        }

        // 3. Board control (majority appointer)
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
                    (SELECT COUNT(*) FROM "ob-poc".board_compositions
                     WHERE entity_id = $1 AND (ended_at IS NULL OR ended_at > CURRENT_DATE)) as total_board
                FROM "ob-poc".board_compositions bc
                LEFT JOIN "ob-poc".entities e ON bc.appointed_by_entity_id = e.entity_id
                WHERE bc.entity_id = $1
                  AND (bc.ended_at IS NULL OR bc.ended_at > CURRENT_DATE)
                  AND bc.appointed_by_entity_id IS NOT NULL
                  AND (e.entity_id IS NULL OR e.deleted_at IS NULL)
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
        .fetch_all(scope.executor())
        .await?;

        for row in board_control {
            if let (Some(appointer), true) = (row.appointer_id, row.has_majority) {
                let holder_id = appointer.to_string();
                control_vectors.push(json!({
                    "vector_type": "board_appointment",
                    "holder_id": holder_id,
                    "holder_name": row.appointer_name,
                    "appointments": row.appointments,
                    "total_board": row.total_board,
                    "has_majority": true,
                    "strength": 0.9,
                }));
                controllers
                    .entry(holder_id)
                    .or_default()
                    .push("board_appointment".into());
            }
        }

        // 4. Trust control (if discretionary trust)
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
                SELECT tp.holder_entity_id, e.name, tp.provision_type, tp.has_discretion
                FROM "ob-poc".trust_provisions tp
                JOIN "ob-poc".entities e ON tp.holder_entity_id = e.entity_id
                WHERE tp.trust_entity_id = $1
                  AND e.deleted_at IS NULL
                  AND (tp.effective_to IS NULL OR tp.effective_to > CURRENT_DATE)
                  AND (tp.provision_type IN ('TRUSTEE_DISCRETIONARY', 'PROTECTOR', 'APPOINTOR_POWER', 'TRUSTEE_REMOVAL')
                       OR tp.has_discretion = true)
                "#,
            )
            .bind(entity_id)
            .fetch_all(scope.executor())
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
                    "strength": strength,
                }));
                controllers
                    .entry(holder_id)
                    .or_default()
                    .push("trust_role".into());
            }
        }

        // 5. Partnership GP control
        if entity_type.as_deref() == Some("partnership_limited") {
            let gp_control: Vec<(Uuid, String)> = sqlx::query_as(
                r#"
                SELECT pc.partner_entity_id, e.name
                FROM "ob-poc".partnership_capital pc
                JOIN "ob-poc".entities e ON pc.partner_entity_id = e.entity_id
                WHERE pc.partnership_entity_id = $1
                  AND e.deleted_at IS NULL
                  AND pc.partner_type = 'GP'
                  AND pc.is_active = true
                "#,
            )
            .bind(entity_id)
            .fetch_all(scope.executor())
            .await?;

            for (partner_id, name) in gp_control {
                let holder_id = partner_id.to_string();
                control_vectors.push(json!({
                    "vector_type": "general_partner",
                    "holder_id": holder_id,
                    "holder_name": name,
                    "strength": 0.95,
                }));
                controllers
                    .entry(holder_id)
                    .or_default()
                    .push("general_partner".into());
            }
        }

        // 6. Executive control
        let exec_control: Vec<(Uuid, String, String)> = sqlx::query_as(
            r#"
            SELECT cer.entity_id, e.name, r.name
            FROM "ob-poc".cbu_entity_roles cer
            JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
            JOIN "ob-poc".roles r ON cer.role_id = r.role_id
            WHERE cer.target_entity_id = $1
              AND e.deleted_at IS NULL
              AND r.name IN ('CEO', 'MANAGING_DIRECTOR', 'EXECUTIVE_DIRECTOR', 'CFO')
              AND (cer.effective_to IS NULL OR cer.effective_to > CURRENT_DATE)
            "#,
        )
        .bind(entity_id)
        .fetch_all(scope.executor())
        .await?;

        for (controller_id, controller_name, role_name) in exec_control {
            let holder_id = controller_id.to_string();
            control_vectors.push(json!({
                "vector_type": "executive_control",
                "holder_id": holder_id,
                "holder_name": controller_name,
                "position": role_name,
                "strength": 0.6,
            }));
            controllers
                .entry(holder_id)
                .or_default()
                .push("executive_control".into());
        }

        // Build controller summary with natural-person flag + aggregate score
        let mut controller_list: Vec<Value> = Vec::new();
        for (controller_id, vectors) in &controllers {
            let is_natural: bool = sqlx::query_scalar(
                r#"SELECT EXISTS(
                    SELECT 1 FROM "ob-poc".entity_proper_persons
                    WHERE entity_id = $1
                )"#,
            )
            .bind(Uuid::parse_str(controller_id).unwrap_or(Uuid::nil()))
            .fetch_one(scope.executor())
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
                "is_natural_person": is_natural,
            }));
        }

        let control_type = match controller_list.len() {
            0 => "unknown",
            1 => "single",
            2..=3 => "joint",
            _ => "diffuse",
        };

        let mut indirect_ubos: Vec<Value> = Vec::new();
        if include_indirect {
            for controller in &controller_list {
                if controller.get("is_natural_person") == Some(&json!(false)) {
                    if let Some(cid) = controller.get("controller_id").and_then(|c| c.as_str()) {
                        indirect_ubos.push(json!({
                            "intermediate_entity_id": cid,
                            "needs_analysis": true,
                        }));
                    }
                }
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "entity_id": entity_id,
            "entity_name": entity_name,
            "entity_type": entity_type,
            "control_vectors": control_vectors,
            "controllers": controller_list,
            "control_type": control_type,
            "is_controlled": !controllers.is_empty(),
            "indirect_analysis_needed": indirect_ubos,
            "analysis_timestamp": chrono::Utc::now().to_rfc3339(),
        })))
    }
}

// ── control.build-graph ───────────────────────────────────────────────────────

pub struct ControlBuildGraph;

#[async_trait]
impl SemOsVerbOp for ControlBuildGraph {
    fn fqn(&self) -> &str {
        "control.build-graph"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let max_depth = args.get("max-depth").and_then(|v| v.as_i64()).unwrap_or(10) as i32;

        let cbu_entities: Vec<(Uuid,)> = sqlx::query_as(
            r#"SELECT DISTINCT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;

        let mut nodes: Vec<Value> = Vec::new();
        let mut edges: Vec<Value> = Vec::new();
        let mut visited: HashSet<String> = HashSet::new();

        for (entity_id,) in &cbu_entities {
            let entity_id_str = entity_id.to_string();
            if visited.contains(&entity_id_str) {
                continue;
            }

            type GraphRow = (
                Uuid,
                String,
                Option<String>,
                Option<Uuid>,
                Option<String>,
                Option<rust_decimal::Decimal>,
                i32,
            );

            let graph_data: Vec<GraphRow> = sqlx::query_as(
                r#"
                WITH RECURSIVE control_graph AS (
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
                      AND e.deleted_at IS NULL

                    UNION ALL

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
                      AND e.deleted_at IS NULL
                      AND (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
                      AND er.relationship_type IN ('ownership', 'control', 'trust_role')
                )
                SELECT DISTINCT ON (entity_id)
                    entity_id, name, entity_type,
                    controller_id, relationship_type, percentage, depth
                FROM control_graph
                ORDER BY entity_id, depth
                "#,
            )
            .bind(entity_id)
            .bind(max_depth)
            .fetch_all(scope.executor())
            .await?;

            for (row_entity_id, name, entity_type, controller_id, rel_type, pct, depth) in
                graph_data
            {
                let node_id = row_entity_id.to_string();
                if !visited.contains(&node_id) {
                    visited.insert(node_id.clone());
                    nodes.push(json!({
                        "id": node_id,
                        "name": name,
                        "entity_type": entity_type,
                        "depth": depth,
                    }));
                }

                if let Some(ctrl_id) = controller_id {
                    edges.push(json!({
                        "from": ctrl_id.to_string(),
                        "to": row_entity_id.to_string(),
                        "relationship_type": rel_type,
                        "percentage": pct.map(|p| p.to_string()),
                    }));
                }
            }
        }

        let entity_ids: Vec<Uuid> = cbu_entities.iter().map(|(id,)| *id).collect();

        let board_edges: Vec<(Option<Uuid>, Uuid)> = sqlx::query_as(
            r#"
            SELECT DISTINCT bc.appointed_by_entity_id, bc.entity_id
            FROM "ob-poc".board_compositions bc
            WHERE bc.entity_id = ANY($1)
              AND bc.appointed_by_entity_id IS NOT NULL
              AND (bc.ended_at IS NULL OR bc.ended_at > CURRENT_DATE)
            "#,
        )
        .bind(&entity_ids)
        .fetch_all(scope.executor())
        .await?;

        for (appointer, entity_id) in board_edges {
            if let Some(from) = appointer {
                edges.push(json!({
                    "from": from.to_string(),
                    "to": entity_id.to_string(),
                    "relationship_type": "board_appointment",
                }));
            }
        }

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "nodes": nodes.clone(),
            "edges": edges.clone(),
            "node_count": nodes.len(),
            "edge_count": edges.len(),
            "max_depth_reached": max_depth,
            "built_at": chrono::Utc::now().to_rfc3339(),
        })))
    }
}

// ── control.identify-ubos ─────────────────────────────────────────────────────

pub struct ControlIdentifyUbos;

#[async_trait]
impl SemOsVerbOp for ControlIdentifyUbos {
    fn fqn(&self) -> &str {
        "control.identify-ubos"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let cbu_id = json_extract_uuid(args, ctx, "cbu-id")?;
        let ownership_threshold = args
            .get("ownership-threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(25.0);

        type OwnershipUboRow = (
            Uuid,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<rust_decimal::Decimal>,
        );

        let ownership_ubos: Vec<OwnershipUboRow> = sqlx::query_as(
            r#"
            WITH RECURSIVE ownership_chain AS (
                SELECT
                    cer.entity_id as target_entity_id,
                    cer.entity_id as current_entity_id,
                    100.0::numeric as effective_percentage,
                    ARRAY[cer.entity_id] as chain,
                    0 as depth
                FROM "ob-poc".cbu_entity_roles cer
                WHERE cer.cbu_id = $1

                UNION ALL

                SELECT
                    oc.target_entity_id,
                    er.from_entity_id as current_entity_id,
                    (oc.effective_percentage * COALESCE(er.percentage, 100) / 100)::numeric,
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
                  AND e.deleted_at IS NULL
                ORDER BY oc.current_entity_id, oc.effective_percentage DESC
            )
            SELECT person_entity_id, person_name, first_name, last_name, nationality, effective_percentage
            FROM natural_person_ubos
            "#,
        )
        .bind(cbu_id)
        .bind(ownership_threshold as f32)
        .fetch_all(scope.executor())
        .await?;

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
            SELECT DISTINCT
                pp.entity_id as person_entity_id,
                e.name as person_name,
                pp.first_name,
                pp.last_name,
                pp.nationality,
                'board_control'::text as control_vector,
                NULL::numeric as effective_percentage
            FROM "ob-poc".board_compositions bc
            JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = bc.entity_id AND cer.cbu_id = $1
            JOIN "ob-poc".entities e ON bc.appointed_by_entity_id = e.entity_id
            JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE bc.appointed_by_entity_id IS NOT NULL
              AND e.deleted_at IS NULL
              AND (bc.ended_at IS NULL OR bc.ended_at > CURRENT_DATE)

            UNION

            SELECT DISTINCT
                pp.entity_id,
                e.name,
                pp.first_name,
                pp.last_name,
                pp.nationality,
                'trust_' || tp.provision_type,
                NULL::numeric
            FROM "ob-poc".trust_provisions tp
            JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = tp.trust_entity_id AND cer.cbu_id = $1
            JOIN "ob-poc".entities e ON tp.holder_entity_id = e.entity_id
            JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE tp.provision_type IN ('TRUSTEE_DISCRETIONARY', 'PROTECTOR', 'APPOINTOR_POWER')
              AND e.deleted_at IS NULL
              AND (tp.effective_to IS NULL OR tp.effective_to > CURRENT_DATE)

            UNION

            SELECT DISTINCT
                pp.entity_id,
                e.name,
                pp.first_name,
                pp.last_name,
                pp.nationality,
                'general_partner'::text,
                pc.profit_share_pct
            FROM "ob-poc".partnership_capital pc
            JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = pc.partnership_entity_id AND cer.cbu_id = $1
            JOIN "ob-poc".entities e ON pc.partner_entity_id = e.entity_id
            JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE pc.partner_type = 'GP'
              AND e.deleted_at IS NULL
              AND pc.is_active = true
            "#,
        )
        .bind(cbu_id)
        .fetch_all(scope.executor())
        .await?;

        let mut all_ubos: HashMap<String, Value> = HashMap::new();

        for (person_id, person_name, first, last, nationality, effective_pct) in ownership_ubos {
            let pid = person_id.to_string();
            let entry = all_ubos.entry(pid.clone()).or_insert_with(|| {
                json!({
                    "person_entity_id": pid,
                    "person_name": person_name,
                    "first_name": first,
                    "last_name": last,
                    "nationality": nationality,
                    "control_vectors": [],
                    "max_ownership_percentage": 0.0,
                })
            });

            if let Some(obj) = entry.as_object_mut() {
                if let Some(vectors) = obj
                    .get_mut("control_vectors")
                    .and_then(|v| v.as_array_mut())
                {
                    vectors.push(json!({
                        "type": "ownership",
                        "percentage": effective_pct.as_ref().map(|p| p.to_string()),
                    }));
                }
                if let Some(pct) = effective_pct {
                    let current_max = obj
                        .get("max_ownership_percentage")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let pct_f: f64 = pct.to_string().parse().unwrap_or(0.0);
                    if pct_f > current_max {
                        obj.insert("max_ownership_percentage".into(), json!(pct_f));
                    }
                }
            }
        }

        for row in control_ubos {
            let pid = row.person_entity_id.to_string();
            let entry = all_ubos.entry(pid.clone()).or_insert_with(|| {
                json!({
                    "person_entity_id": pid,
                    "person_name": row.person_name,
                    "first_name": row.first_name,
                    "last_name": row.last_name,
                    "nationality": row.nationality,
                    "control_vectors": [],
                    "max_ownership_percentage": 0.0,
                })
            });
            if let Some(obj) = entry.as_object_mut() {
                if let Some(vectors) = obj
                    .get_mut("control_vectors")
                    .and_then(|v| v.as_array_mut())
                {
                    vectors.push(json!({
                        "type": row.control_vector,
                        "percentage": row.effective_percentage.map(|p| p.to_string()),
                    }));
                }
            }
        }

        let ubo_list: Vec<Value> = all_ubos.into_values().collect();

        Ok(VerbExecutionOutcome::Record(json!({
            "cbu_id": cbu_id,
            "ownership_threshold": ownership_threshold,
            "ubos": ubo_list.clone(),
            "ubo_count": ubo_list.len(),
            "identified_at": chrono::Utc::now().to_rfc3339(),
        })))
    }
}
