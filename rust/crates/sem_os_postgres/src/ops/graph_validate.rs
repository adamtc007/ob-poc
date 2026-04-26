//! Graph validation (1 plugin verb) — YAML-first re-implementation of
//! `graph.validate` from `rust/config/verbs/graph.yaml`.
//!
//! Ownership/control graph validation per KYC/UBO architecture spec
//! section 6.4:
//!
//! - Cycle detection (Tarjan-style SCC via iterative DFS)
//! - Missing percentages on ownership edges
//! - Per-target supply > 100% aggregation
//! - Terminus integrity (chains must terminate at natural persons or
//!   regulated entities)
//! - Source conflicts (conflicting ownership % from distinct sources)
//! - Orphan entities (isolated nodes)
//!
//! Loads `entity_relationships` (scoped to the case when `case-id` is
//! supplied), runs checks in-memory, persists anomalies into
//! `research_anomalies` anchored to a freshly-created
//! `research_actions` row.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::json_extract_uuid_opt;
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphAnomaly {
    pub anomaly_type: String,
    pub entity_ids: Vec<Uuid>,
    pub detail: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphValidateResult {
    pub anomalies_found: i32,
    pub anomalies: Vec<GraphAnomaly>,
    pub edges_analysed: i32,
    pub entities_analysed: i32,
    pub anomalies_persisted: i32,
}

#[derive(Debug, Clone)]
struct Edge {
    relationship_id: Uuid,
    from_entity_id: Uuid,
    to_entity_id: Uuid,
    relationship_type: String,
    percentage: Option<f64>,
    source: Option<String>,
}

type EdgeRow = (
    Uuid,
    Uuid,
    Uuid,
    String,
    Option<rust_decimal::Decimal>,
    Option<String>,
);

async fn load_edges(scope: &mut dyn TransactionScope, case_id: Option<Uuid>) -> Result<Vec<Edge>> {
    let rows: Vec<EdgeRow> = if let Some(cid) = case_id {
        sqlx::query_as(
            r#"
            SELECT
                er.relationship_id,
                er.from_entity_id,
                er.to_entity_id,
                er.relationship_type,
                er.percentage,
                er.source
            FROM "ob-poc".entity_relationships er
            WHERE (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
              AND (
                  er.from_entity_id IN (SELECT entity_id FROM "ob-poc".entity_workstreams WHERE case_id = $1)
                  OR er.to_entity_id IN (SELECT entity_id FROM "ob-poc".entity_workstreams WHERE case_id = $1)
              )
            ORDER BY er.to_entity_id, er.from_entity_id
            "#,
        )
        .bind(cid)
        .fetch_all(scope.executor())
        .await?
    } else {
        sqlx::query_as(
            r#"
            SELECT
                er.relationship_id,
                er.from_entity_id,
                er.to_entity_id,
                er.relationship_type,
                er.percentage,
                er.source
            FROM "ob-poc".entity_relationships er
            WHERE (er.effective_to IS NULL OR er.effective_to > CURRENT_DATE)
            ORDER BY er.to_entity_id, er.from_entity_id
            "#,
        )
        .fetch_all(scope.executor())
        .await?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                relationship_id,
                from_entity_id,
                to_entity_id,
                relationship_type,
                percentage,
                source,
            )| Edge {
                relationship_id,
                from_entity_id,
                to_entity_id,
                relationship_type,
                percentage: percentage.map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)),
                source,
            },
        )
        .collect())
}

fn detect_cycles(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
    let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    let mut all_nodes: HashSet<Uuid> = HashSet::new();
    for edge in edges {
        adj.entry(edge.from_entity_id)
            .or_default()
            .push(edge.to_entity_id);
        all_nodes.insert(edge.from_entity_id);
        all_nodes.insert(edge.to_entity_id);
    }

    let mut index_counter: u32 = 0;
    let mut stack: Vec<Uuid> = Vec::new();
    let mut on_stack: HashSet<Uuid> = HashSet::new();
    let mut indices: HashMap<Uuid, u32> = HashMap::new();
    let mut lowlinks: HashMap<Uuid, u32> = HashMap::new();
    let mut sccs: Vec<Vec<Uuid>> = Vec::new();

    for start_node in &all_nodes {
        if indices.contains_key(start_node) {
            continue;
        }
        let mut dfs_stack: Vec<(Uuid, usize)> = vec![(*start_node, 0)];
        indices.insert(*start_node, index_counter);
        lowlinks.insert(*start_node, index_counter);
        index_counter += 1;
        stack.push(*start_node);
        on_stack.insert(*start_node);

        while let Some((node, ni)) = dfs_stack.last_mut() {
            let node = *node;
            let neighbors = adj.get(&node).cloned().unwrap_or_default();
            if *ni < neighbors.len() {
                let neighbor = neighbors[*ni];
                *ni += 1;
                if let std::collections::hash_map::Entry::Vacant(e) = indices.entry(neighbor) {
                    e.insert(index_counter);
                    lowlinks.insert(neighbor, index_counter);
                    index_counter += 1;
                    stack.push(neighbor);
                    on_stack.insert(neighbor);
                    dfs_stack.push((neighbor, 0));
                } else if on_stack.contains(&neighbor) {
                    let neighbor_idx = indices[&neighbor];
                    let current_low = lowlinks[&node];
                    if neighbor_idx < current_low {
                        lowlinks.insert(node, neighbor_idx);
                    }
                }
            } else {
                dfs_stack.pop();
                if let Some((parent, _)) = dfs_stack.last() {
                    let parent = *parent;
                    let node_low = lowlinks[&node];
                    let parent_low = lowlinks[&parent];
                    if node_low < parent_low {
                        lowlinks.insert(parent, node_low);
                    }
                }
                if lowlinks[&node] == indices[&node] {
                    let mut scc: Vec<Uuid> = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack.remove(&w);
                        scc.push(w);
                        if w == node {
                            break;
                        }
                    }
                    if scc.len() > 1 {
                        sccs.push(scc);
                    }
                }
            }
        }
    }

    for scc in sccs {
        let ids_str: Vec<String> = scc.iter().map(|id| id.to_string()).collect();
        anomalies.push(GraphAnomaly {
            anomaly_type: "CYCLE".to_string(),
            entity_ids: scc,
            detail: format!(
                "Ownership/control cycle detected among {} entities: {}",
                ids_str.len(),
                ids_str.join(" -> ")
            ),
            severity: "ERROR".to_string(),
        });
    }
}

fn check_missing_percentages(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
    for edge in edges {
        if edge.relationship_type == "ownership" && edge.percentage.is_none() {
            anomalies.push(GraphAnomaly {
                anomaly_type: "MISSING_PERCENTAGE".to_string(),
                entity_ids: vec![edge.from_entity_id, edge.to_entity_id],
                detail: format!(
                    "Ownership edge from {} to {} (rel {}) has no percentage",
                    edge.from_entity_id, edge.to_entity_id, edge.relationship_id
                ),
                severity: "WARNING".to_string(),
            });
        }
    }
}

fn check_supply_exceeds_100(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
    let mut supply_map: HashMap<Uuid, f64> = HashMap::new();
    let mut holders_map: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for edge in edges {
        if edge.relationship_type == "ownership" {
            if let Some(pct) = edge.percentage {
                *supply_map.entry(edge.to_entity_id).or_insert(0.0) += pct;
                holders_map
                    .entry(edge.to_entity_id)
                    .or_default()
                    .push(edge.from_entity_id);
            }
        }
    }
    for (to_entity_id, total) in &supply_map {
        if *total > 100.0 {
            let mut entity_ids = vec![*to_entity_id];
            if let Some(holders) = holders_map.get(to_entity_id) {
                entity_ids.extend(holders);
            }
            anomalies.push(GraphAnomaly {
                anomaly_type: "SUPPLY_EXCEEDS_100".to_string(),
                entity_ids,
                detail: format!(
                    "Entity {} has total inbound ownership of {:.2}% (exceeds 100%)",
                    to_entity_id, total
                ),
                severity: "ERROR".to_string(),
            });
        }
    }
}

async fn check_terminus_integrity(
    edges: &[Edge],
    _all_entity_ids: &HashSet<Uuid>,
    scope: &mut dyn TransactionScope,
    anomalies: &mut Vec<GraphAnomaly>,
) -> Result<()> {
    let mut has_inbound: HashSet<Uuid> = HashSet::new();
    let mut has_outbound: HashSet<Uuid> = HashSet::new();
    for edge in edges {
        if edge.relationship_type == "ownership" || edge.relationship_type == "control" {
            has_inbound.insert(edge.to_entity_id);
            has_outbound.insert(edge.from_entity_id);
        }
    }
    let root_entities: Vec<Uuid> = has_outbound
        .iter()
        .filter(|id| !has_inbound.contains(id))
        .copied()
        .collect();
    if root_entities.is_empty() {
        return Ok(());
    }

    let natural_rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT pp.entity_id
           FROM "ob-poc".entity_proper_persons pp
           WHERE pp.entity_id = ANY($1)"#,
    )
    .bind(&root_entities)
    .fetch_all(scope.executor())
    .await?;
    let natural_persons: HashSet<Uuid> = natural_rows.into_iter().map(|(id,)| id).collect();

    let regulated_rows: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT er.entity_id
        FROM "ob-poc".entity_relationships er2
        JOIN "ob-poc".entities er ON er.entity_id = er2.from_entity_id
        WHERE er.entity_id = ANY($1)
          AND er.deleted_at IS NULL
          AND er2.is_regulated = true
        UNION
        SELECT e.entity_id
        FROM "ob-poc".entities e
        JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
        WHERE e.entity_id = ANY($1)
          AND e.deleted_at IS NULL
          AND et.type_code IN ('government', 'public_authority', 'listed_company')
        "#,
    )
    .bind(&root_entities)
    .fetch_all(scope.executor())
    .await?;
    let regulated_entities: HashSet<Uuid> = regulated_rows.into_iter().map(|(id,)| id).collect();

    for root_id in &root_entities {
        if !natural_persons.contains(root_id) && !regulated_entities.contains(root_id) {
            anomalies.push(GraphAnomaly {
                anomaly_type: "TERMINUS_NOT_NATURAL_PERSON".to_string(),
                entity_ids: vec![*root_id],
                detail: format!(
                    "Entity {} is a root of an ownership chain but is not a natural person or \
                     known regulated terminator. Ownership chains should resolve to individuals.",
                    root_id
                ),
                severity: "WARNING".to_string(),
            });
        }
    }
    Ok(())
}

fn check_source_conflicts(edges: &[Edge], anomalies: &mut Vec<GraphAnomaly>) {
    let mut edge_groups: HashMap<(Uuid, Uuid, String), Vec<&Edge>> = HashMap::new();
    for edge in edges {
        let key = (
            edge.from_entity_id,
            edge.to_entity_id,
            edge.relationship_type.clone(),
        );
        edge_groups.entry(key).or_default().push(edge);
    }
    for ((from_id, to_id, rel_type), group) in &edge_groups {
        if group.len() < 2 {
            continue;
        }
        let mut source_pcts: HashMap<String, f64> = HashMap::new();
        for edge in group {
            let src = edge.source.clone().unwrap_or_else(|| "unknown".to_string());
            if let Some(pct) = edge.percentage {
                if let Some(existing_pct) = source_pcts.get(&src) {
                    if (*existing_pct - pct).abs() > 0.01 {
                        anomalies.push(GraphAnomaly {
                            anomaly_type: "SOURCE_CONFLICT".to_string(),
                            entity_ids: vec![*from_id, *to_id],
                            detail: format!(
                                "Source '{}' reports conflicting {} percentages for {} -> {}: \
                                 {:.2}% vs {:.2}%",
                                src, rel_type, from_id, to_id, existing_pct, pct
                            ),
                            severity: "ERROR".to_string(),
                        });
                    }
                } else {
                    source_pcts.insert(src, pct);
                }
            }
        }
        let sources: Vec<(&String, &f64)> = source_pcts.iter().collect();
        for i in 0..sources.len() {
            for j in (i + 1)..sources.len() {
                let (src_a, pct_a) = sources[i];
                let (src_b, pct_b) = sources[j];
                if (pct_a - pct_b).abs() > 0.01 {
                    anomalies.push(GraphAnomaly {
                        anomaly_type: "SOURCE_CONFLICT".to_string(),
                        entity_ids: vec![*from_id, *to_id],
                        detail: format!(
                            "Conflicting {} percentages for {} -> {}: source '{}' reports {:.2}% \
                             vs source '{}' reports {:.2}%",
                            rel_type, from_id, to_id, src_a, pct_a, src_b, pct_b
                        ),
                        severity: "WARNING".to_string(),
                    });
                }
            }
        }
    }
}

fn check_orphan_entities(
    edges: &[Edge],
    all_entity_ids: &HashSet<Uuid>,
    anomalies: &mut Vec<GraphAnomaly>,
) {
    let mut connected: HashSet<Uuid> = HashSet::new();
    for edge in edges {
        connected.insert(edge.from_entity_id);
        connected.insert(edge.to_entity_id);
    }
    for entity_id in all_entity_ids {
        if !connected.contains(entity_id) {
            anomalies.push(GraphAnomaly {
                anomaly_type: "ORPHAN_ENTITY".to_string(),
                entity_ids: vec![*entity_id],
                detail: format!(
                    "Entity {} has no ownership or control relationships (orphan in graph)",
                    entity_id
                ),
                severity: "WARNING".to_string(),
            });
        }
    }
}

async fn persist_anomalies(
    scope: &mut dyn TransactionScope,
    anomalies: &[GraphAnomaly],
    case_id: Option<Uuid>,
    all_entity_ids: &HashSet<Uuid>,
) -> Result<i32> {
    if anomalies.is_empty() {
        return Ok(0);
    }

    let representative_entity_id = if let Some(cid) = case_id {
        let row: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT entity_id FROM "ob-poc".entity_workstreams WHERE case_id = $1 LIMIT 1"#,
        )
        .bind(cid)
        .fetch_optional(scope.executor())
        .await?;
        row.map(|(id,)| id)
    } else {
        all_entity_ids.iter().next().copied()
    };

    let target_entity_id = representative_entity_id
        .ok_or_else(|| anyhow!("No entities found to anchor research action"))?;

    let action_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO "ob-poc".research_actions (
            target_entity_id,
            action_type,
            source_provider,
            source_key,
            source_key_type,
            verb_domain,
            verb_name,
            verb_args,
            success,
            entities_created,
            entities_updated,
            relationships_created
        ) VALUES (
            $1,
            'GRAPH_VALIDATION',
            'internal',
            COALESCE($2::text, 'all'),
            'graph_validate',
            'graph',
            'validate',
            '{}'::jsonb,
            true, 0, 0, 0
        )
        RETURNING action_id
        "#,
    )
    .bind(target_entity_id)
    .bind(case_id.map(|id| id.to_string()))
    .fetch_one(scope.executor())
    .await?;

    let mut persisted = 0i32;
    for anomaly in anomalies {
        let entity_id = anomaly
            .entity_ids
            .first()
            .copied()
            .unwrap_or(target_entity_id);

        let rule_code = match anomaly.anomaly_type.as_str() {
            "CYCLE" => "GRAPH_CYCLE_DETECTED",
            "MISSING_PERCENTAGE" => "GRAPH_MISSING_PCT",
            "SUPPLY_EXCEEDS_100" => "GRAPH_SUPPLY_GT_100",
            "TERMINUS_NOT_NATURAL_PERSON" => "GRAPH_TERMINUS_NOT_NP",
            "SOURCE_CONFLICT" => "GRAPH_SOURCE_CONFLICT",
            "ORPHAN_ENTITY" => "GRAPH_ORPHAN_ENTITY",
            other => other,
        };

        let db_severity = match anomaly.severity.as_str() {
            "ERROR" => "ERROR",
            "WARNING" => "WARNING",
            _ => "INFO",
        };

        let expected_value: Option<String> = match anomaly.anomaly_type.as_str() {
            "SUPPLY_EXCEEDS_100" => Some("<=100%".to_string()),
            "MISSING_PERCENTAGE" => Some("numeric percentage".to_string()),
            "TERMINUS_NOT_NATURAL_PERSON" => Some("natural person or regulated entity".to_string()),
            "CYCLE" => Some("acyclic graph".to_string()),
            _ => None,
        };
        let actual_value: Option<String> = Some(anomaly.detail.clone());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".research_anomalies (
                action_id, entity_id, rule_code, severity,
                description, expected_value, actual_value, status
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, 'OPEN')
            "#,
        )
        .bind(action_id)
        .bind(entity_id)
        .bind(rule_code)
        .bind(db_severity)
        .bind(&anomaly.detail)
        .bind(&expected_value)
        .bind(&actual_value)
        .execute(scope.executor())
        .await?;
        persisted += 1;
    }

    tracing::info!(
        "graph.validate: persisted {} anomalies under action_id {}",
        persisted,
        action_id
    );
    Ok(persisted)
}

pub struct Validate;

#[async_trait]
impl SemOsVerbOp for Validate {
    fn fqn(&self) -> &str {
        "graph.validate"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid_opt(args, ctx, "case-id");

        let edges = load_edges(scope, case_id).await?;
        let mut all_entity_ids: HashSet<Uuid> = HashSet::new();
        for edge in &edges {
            all_entity_ids.insert(edge.from_entity_id);
            all_entity_ids.insert(edge.to_entity_id);
        }
        let entities_analysed = all_entity_ids.len() as i32;
        let edges_analysed = edges.len() as i32;

        let mut anomalies: Vec<GraphAnomaly> = Vec::new();
        detect_cycles(&edges, &mut anomalies);
        check_missing_percentages(&edges, &mut anomalies);
        check_supply_exceeds_100(&edges, &mut anomalies);
        check_source_conflicts(&edges, &mut anomalies);
        check_orphan_entities(&edges, &all_entity_ids, &mut anomalies);
        check_terminus_integrity(&edges, &all_entity_ids, scope, &mut anomalies).await?;

        let anomalies_found = anomalies.len() as i32;
        let anomalies_persisted =
            persist_anomalies(scope, &anomalies, case_id, &all_entity_ids).await?;

        let result = GraphValidateResult {
            anomalies_found,
            anomalies,
            edges_analysed,
            entities_analysed,
            anomalies_persisted,
        };
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}
