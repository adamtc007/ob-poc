use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::action_surface::compute_action_surface;
use super::error::{ConstellationError, ConstellationResult};
use super::hydrated::HydratedConstellation;
use super::normalize::normalize_slots;
use super::query_plan::compile_query_plan;
use super::summary::{compute_summary, ConstellationSummary};
use super::validate::ValidatedConstellationMap;
use crate::state_reducer::{
    build_eval_scope_tx, fetch_slot_overlays_tx, get_active_override_tx, reduce_slot,
    SlotReduceResult,
};

fn is_missing_relation_error(error: &sqlx::Error, relation_name: &str) -> bool {
    let relation = format!("\"ob-poc\".{}", relation_name);
    matches!(error, sqlx::Error::Database(db_error)
        if db_error.code().as_deref() == Some("42P01")
            && (db_error.message().contains(&relation)
                || db_error.message().contains(relation_name)))
}

/// Raw hydration bundle collected before normalization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawHydrationData {
    pub root: Option<Uuid>,
    pub slot_rows: HashMap<String, Vec<RawSlotRow>>,
    pub overlay_rows: HashMap<String, Vec<RawOverlayRow>>,
    pub overrides: HashMap<String, serde_json::Value>,
    pub entity_details: HashMap<Uuid, serde_json::Value>,
    pub graph_edges: HashMap<String, Vec<RawGraphEdge>>,
    pub warnings: HashMap<String, Vec<String>>,
    pub reducer_results: HashMap<String, SlotReduceResult>,
}

/// Raw slot row before singular/graph normalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSlotRow {
    pub entity_id: Option<Uuid>,
    pub record_id: Option<Uuid>,
    pub filter_value: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Raw overlay row before reducer binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawOverlayRow {
    pub entity_id: Option<Uuid>,
    pub source_name: String,
    pub fields: serde_json::Value,
}

/// Raw graph edge for recursive slots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawGraphEdge {
    pub from_entity_id: Uuid,
    pub to_entity_id: Uuid,
    pub percentage: Option<f64>,
    pub ownership_type: Option<String>,
    pub depth: usize,
}

/// Reducer-relevant slot context discovered from a constellation map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstellationSlotContext {
    pub slot_path: String,
    pub entity_id: Uuid,
    pub slot_type: String,
    pub cardinality: String,
}

/// Hydrate, normalize, and enrich a constellation from the database.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::constellation::{hydrate_constellation, load_builtin_constellation_map};
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let _ = hydrate_constellation(pool, Uuid::new_v4(), None, &map).await?;
/// # Ok(())
/// # }
/// ```
pub async fn hydrate_constellation(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map: &ValidatedConstellationMap,
) -> ConstellationResult<HydratedConstellation> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|err| ConstellationError::Other(err.into()))?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await
        .map_err(|err| ConstellationError::Other(err.into()))?;

    let _plan = compile_query_plan(map);
    let mut raw = RawHydrationData {
        root: Some(cbu_id),
        ..Default::default()
    };
    let scope = build_eval_scope_tx(&mut tx, cbu_id, case_id)
        .await
        .map_err(ConstellationError::Other)?;

    for slot in &map.slots_ordered {
        let rows = if slot.def.slot_type == super::map_def::SlotType::EntityGraph {
            let (rows, edges) = query_entity_graph_rows_tx(&mut tx, slot, &raw).await?;
            raw.graph_edges.insert(slot.name.clone(), edges);
            rows
        } else {
            query_slot_rows_tx(&mut tx, cbu_id, case_id, slot, &raw).await?
        };
        raw.slot_rows.insert(slot.name.clone(), rows.clone());
        populate_entity_details_tx(&mut tx, &mut raw, &rows).await?;

        if let Some(entity_id) = pick_deterministic_row(&rows).and_then(|row| row.entity_id) {
            if let Some(machine_name) = slot.def.state_machine.as_ref() {
                let machine = map.state_machines.get(machine_name).ok_or_else(|| {
                    ConstellationError::Execution(format!(
                        "missing loaded state machine '{}'",
                        machine_name
                    ))
                })?;
                let mut overlays = fetch_slot_overlays_tx(&mut tx, cbu_id, entity_id, case_id)
                    .await
                    .map_err(ConstellationError::Other)?;
                overlays.scope = scope.as_scope_data();
                let override_entry = get_active_override_tx(&mut tx, cbu_id, case_id, &slot.name)
                    .await
                    .map_err(ConstellationError::Other)?;
                let derived = reduce_slot(machine, &slot.name, &overlays, override_entry)
                    .map_err(|err| ConstellationError::Execution(err.to_string()))?;
                raw.warnings
                    .insert(slot.name.clone(), derived.consistency_warnings.clone());
                raw.reducer_results.insert(slot.name.clone(), derived);
            }

            let overlays = fetch_slot_overlays_tx(&mut tx, cbu_id, entity_id, case_id)
                .await
                .map_err(ConstellationError::Other)?;
            raw.overlay_rows.insert(
                slot.name.clone(),
                raw_overlay_rows_from_slot_data(entity_id, overlays),
            );
        }
    }

    let normalized = normalize_slots(map, cbu_id, case_id, raw);
    Ok(compute_action_surface(map, normalized))
}

/// Compute just the high-level summary for a constellation.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::constellation::{hydrate_constellation_summary, load_builtin_constellation_map};
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let _ = hydrate_constellation_summary(pool, Uuid::new_v4(), None, &map).await?;
/// # Ok(())
/// # }
/// ```
pub async fn hydrate_constellation_summary(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map: &ValidatedConstellationMap,
) -> ConstellationResult<ConstellationSummary> {
    let hydrated = hydrate_constellation(pool, cbu_id, case_id, map).await?;
    Ok(compute_summary(&hydrated))
}

/// Discover slot contexts in a constellation that use the given reducer machine.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::constellation::{discover_state_machine_slot_contexts, load_builtin_constellation_map};
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let _ = discover_state_machine_slot_contexts(
///     pool,
///     Uuid::new_v4(),
///     None,
///     &map,
///     "entity_kyc_lifecycle",
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn discover_state_machine_slot_contexts(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map: &ValidatedConstellationMap,
    state_machine_name: &str,
) -> ConstellationResult<Vec<ConstellationSlotContext>> {
    let mut raw = RawHydrationData::default();
    let mut contexts = Vec::new();

    for slot in &map.slots_ordered {
        let rows = if slot.def.slot_type == super::map_def::SlotType::EntityGraph {
            let (rows, edges) = query_entity_graph_rows(pool, slot, &raw).await?;
            raw.graph_edges.insert(slot.name.clone(), edges);
            rows
        } else {
            query_slot_rows(pool, cbu_id, case_id, slot, &raw).await?
        };
        raw.slot_rows.insert(slot.name.clone(), rows.clone());

        if slot.def.state_machine.as_deref() == Some(state_machine_name) {
            match slot.def.slot_type {
                super::map_def::SlotType::Entity => {
                    contexts.extend(rows.iter().filter_map(|row| {
                        row.entity_id.map(|entity_id| ConstellationSlotContext {
                            slot_path: slot.name.clone(),
                            entity_id,
                            slot_type: String::from("entity"),
                            cardinality: format!("{:?}", slot.def.cardinality).to_lowercase(),
                        })
                    }));
                }
                super::map_def::SlotType::EntityGraph => {
                    let unique = rows
                        .iter()
                        .filter_map(|row| row.entity_id)
                        .collect::<std::collections::HashSet<_>>();
                    contexts.extend(
                        unique
                            .into_iter()
                            .map(|entity_id| ConstellationSlotContext {
                                slot_path: format!("{}.{}", slot.name, entity_id),
                                entity_id,
                                slot_type: String::from("entity_graph"),
                                cardinality: String::from("recursive"),
                            }),
                    );
                }
                _ => {}
            }
        }
    }

    Ok(contexts)
}

async fn query_slot_rows(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    slot: &super::validate::ResolvedSlot,
    raw: &RawHydrationData,
) -> ConstellationResult<Vec<RawSlotRow>> {
    match slot.def.slot_type {
        super::map_def::SlotType::Cbu => {
            if slot.def.cardinality == super::map_def::Cardinality::Root {
                return Ok(vec![RawSlotRow {
                    entity_id: Some(cbu_id),
                    record_id: Some(cbu_id),
                    filter_value: None,
                    created_at: None,
                }]);
            }
            if let Some(join) = &slot.def.join {
                if join.via == "cbu_structure_links" {
                    let rows = query_child_cbu_rows(pool, cbu_id, join).await?;
                    return Ok(select_occurrence(rows, slot.def.occurrence));
                }
            }
            Ok(Vec::new())
        }
        super::map_def::SlotType::Entity => {
            if let Some(join) = &slot.def.join {
                if join.via == "cbu_entity_roles" {
                    let filter_value = join.filter_value.clone().unwrap_or_default();
                    let rows = sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
                        r#"
                        SELECT cer.entity_id, cer.created_at
                        FROM "ob-poc".cbu_entity_roles cer
                        JOIN "ob-poc".roles r ON r.role_id = cer.role_id
                        WHERE cer.cbu_id = $1
                          AND REPLACE(LOWER(r.name), '_', '-') = LOWER($2)
                        ORDER BY cer.created_at DESC NULLS LAST
                        "#,
                    )
                    .bind(cbu_id)
                    .bind(filter_value)
                    .fetch_all(pool)
                    .await
                    .map_err(|err| ConstellationError::Other(err.into()))?;
                    let rows = rows
                        .into_iter()
                        .map(|(entity_id, created_at)| RawSlotRow {
                            entity_id: Some(entity_id),
                            record_id: Some(entity_id),
                            filter_value: join.filter_value.clone(),
                            created_at,
                        })
                        .collect::<Vec<_>>();
                    return Ok(select_occurrence(rows, slot.def.occurrence));
                }
            }
            Ok(Vec::new())
        }
        super::map_def::SlotType::Case => query_case_rows(pool, cbu_id, case_id).await,
        super::map_def::SlotType::Tollgate => {
            let active_case_id = resolve_case_id(case_id, raw);
            query_tollgate_rows(pool, active_case_id).await
        }
        super::map_def::SlotType::Mandate => query_mandate_rows(pool, cbu_id).await,
        super::map_def::SlotType::EntityGraph => query_entity_graph_rows(pool, slot, raw)
            .await
            .map(|(rows, _)| rows),
    }
}

async fn query_slot_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    slot: &super::validate::ResolvedSlot,
    raw: &RawHydrationData,
) -> ConstellationResult<Vec<RawSlotRow>> {
    match slot.def.slot_type {
        super::map_def::SlotType::Cbu => {
            if slot.def.cardinality == super::map_def::Cardinality::Root {
                return Ok(vec![RawSlotRow {
                    entity_id: Some(cbu_id),
                    record_id: Some(cbu_id),
                    filter_value: None,
                    created_at: None,
                }]);
            }
            if let Some(join) = &slot.def.join {
                if join.via == "cbu_structure_links" {
                    let rows = query_child_cbu_rows_tx(tx, cbu_id, join).await?;
                    return Ok(select_occurrence(rows, slot.def.occurrence));
                }
            }
            Ok(Vec::new())
        }
        super::map_def::SlotType::Entity => {
            if let Some(join) = &slot.def.join {
                if join.via == "cbu_entity_roles" {
                    let filter_value = join.filter_value.clone().unwrap_or_default();
                    let rows = sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
                        r#"
                        SELECT cer.entity_id, cer.created_at
                        FROM "ob-poc".cbu_entity_roles cer
                        JOIN "ob-poc".roles r ON r.role_id = cer.role_id
                        WHERE cer.cbu_id = $1
                          AND REPLACE(LOWER(r.name), '_', '-') = LOWER($2)
                        ORDER BY cer.created_at DESC NULLS LAST
                        "#,
                    )
                    .bind(cbu_id)
                    .bind(filter_value)
                    .fetch_all(&mut **tx)
                    .await
                    .map_err(|err| ConstellationError::Other(err.into()))?;
                    let rows = rows
                        .into_iter()
                        .map(|(entity_id, created_at)| RawSlotRow {
                            entity_id: Some(entity_id),
                            record_id: Some(entity_id),
                            filter_value: join.filter_value.clone(),
                            created_at,
                        })
                        .collect::<Vec<_>>();
                    return Ok(select_occurrence(rows, slot.def.occurrence));
                }
            }
            Ok(Vec::new())
        }
        super::map_def::SlotType::Case => query_case_rows_tx(tx, cbu_id, case_id).await,
        super::map_def::SlotType::Tollgate => {
            let active_case_id = resolve_case_id(case_id, raw);
            query_tollgate_rows_tx(tx, active_case_id).await
        }
        super::map_def::SlotType::Mandate => query_mandate_rows_tx(tx, cbu_id).await,
        super::map_def::SlotType::EntityGraph => query_entity_graph_rows_tx(tx, slot, raw)
            .await
            .map(|(rows, _)| rows),
    }
}

async fn query_child_cbu_rows(
    pool: &PgPool,
    parent_cbu_id: Uuid,
    join: &super::map_def::JoinDef,
) -> ConstellationResult<Vec<RawSlotRow>> {
    let filter_column = join.filter_column.as_deref();
    let filter_value = join.filter_value.as_deref();
    let rows = match (filter_column, filter_value) {
        (Some("relationship_selector"), Some(selector)) => {
            sqlx::query_as::<_, (Uuid, String, Option<DateTime<Utc>>)>(
                r#"
                SELECT child_cbu_id, relationship_selector, created_at
                FROM "ob-poc".cbu_structure_links
                WHERE parent_cbu_id = $1
                  AND relationship_selector = $2
                  AND status = 'ACTIVE'
                  AND (effective_from IS NULL OR effective_from <= CURRENT_DATE)
                  AND (effective_to IS NULL OR effective_to >= CURRENT_DATE)
                ORDER BY created_at DESC NULLS LAST, updated_at DESC
                "#,
            )
            .bind(parent_cbu_id)
            .bind(selector)
            .fetch_all(pool)
            .await
            .map_err(|err| ConstellationError::Other(err.into()))?
        }
        (Some("relationship_type"), Some(relationship_type)) => {
            sqlx::query_as::<_, (Uuid, String, Option<DateTime<Utc>>)>(
                r#"
                SELECT child_cbu_id, relationship_selector, created_at
                FROM "ob-poc".cbu_structure_links
                WHERE parent_cbu_id = $1
                  AND relationship_type = $2
                  AND status = 'ACTIVE'
                  AND (effective_from IS NULL OR effective_from <= CURRENT_DATE)
                  AND (effective_to IS NULL OR effective_to >= CURRENT_DATE)
                ORDER BY created_at DESC NULLS LAST, updated_at DESC
                "#,
            )
            .bind(parent_cbu_id)
            .bind(relationship_type.replace('-', "_").to_ascii_uppercase())
            .fetch_all(pool)
            .await
            .map_err(|err| ConstellationError::Other(err.into()))?
        }
        _ => sqlx::query_as::<_, (Uuid, String, Option<DateTime<Utc>>)>(
            r#"
                SELECT child_cbu_id, relationship_selector, created_at
                FROM "ob-poc".cbu_structure_links
                WHERE parent_cbu_id = $1
                  AND status = 'ACTIVE'
                  AND (effective_from IS NULL OR effective_from <= CURRENT_DATE)
                  AND (effective_to IS NULL OR effective_to >= CURRENT_DATE)
                ORDER BY created_at DESC NULLS LAST, updated_at DESC
                "#,
        )
        .bind(parent_cbu_id)
        .fetch_all(pool)
        .await
        .map_err(|err| ConstellationError::Other(err.into()))?,
    };

    Ok(rows
        .into_iter()
        .map(|(child_cbu_id, selector, created_at)| RawSlotRow {
            entity_id: Some(child_cbu_id),
            record_id: Some(child_cbu_id),
            filter_value: Some(selector),
            created_at,
        })
        .collect())
}

async fn query_child_cbu_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    parent_cbu_id: Uuid,
    join: &super::map_def::JoinDef,
) -> ConstellationResult<Vec<RawSlotRow>> {
    let filter_column = join.filter_column.as_deref();
    let filter_value = join.filter_value.as_deref();
    let rows = match (filter_column, filter_value) {
        (Some("relationship_selector"), Some(selector)) => {
            sqlx::query_as::<_, (Uuid, String, Option<DateTime<Utc>>)>(
                r#"
                SELECT child_cbu_id, relationship_selector, created_at
                FROM "ob-poc".cbu_structure_links
                WHERE parent_cbu_id = $1
                  AND relationship_selector = $2
                  AND status = 'ACTIVE'
                  AND (effective_from IS NULL OR effective_from <= CURRENT_DATE)
                  AND (effective_to IS NULL OR effective_to >= CURRENT_DATE)
                ORDER BY created_at DESC NULLS LAST, updated_at DESC
                "#,
            )
            .bind(parent_cbu_id)
            .bind(selector)
            .fetch_all(&mut **tx)
            .await
            .map_err(|err| ConstellationError::Other(err.into()))?
        }
        (Some("relationship_type"), Some(relationship_type)) => {
            sqlx::query_as::<_, (Uuid, String, Option<DateTime<Utc>>)>(
                r#"
                SELECT child_cbu_id, relationship_selector, created_at
                FROM "ob-poc".cbu_structure_links
                WHERE parent_cbu_id = $1
                  AND relationship_type = $2
                  AND status = 'ACTIVE'
                  AND (effective_from IS NULL OR effective_from <= CURRENT_DATE)
                  AND (effective_to IS NULL OR effective_to >= CURRENT_DATE)
                ORDER BY created_at DESC NULLS LAST, updated_at DESC
                "#,
            )
            .bind(parent_cbu_id)
            .bind(relationship_type.replace('-', "_").to_ascii_uppercase())
            .fetch_all(&mut **tx)
            .await
            .map_err(|err| ConstellationError::Other(err.into()))?
        }
        _ => sqlx::query_as::<_, (Uuid, String, Option<DateTime<Utc>>)>(
            r#"
                SELECT child_cbu_id, relationship_selector, created_at
                FROM "ob-poc".cbu_structure_links
                WHERE parent_cbu_id = $1
                  AND status = 'ACTIVE'
                  AND (effective_from IS NULL OR effective_from <= CURRENT_DATE)
                  AND (effective_to IS NULL OR effective_to >= CURRENT_DATE)
                ORDER BY created_at DESC NULLS LAST, updated_at DESC
                "#,
        )
        .bind(parent_cbu_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|err| ConstellationError::Other(err.into()))?,
    };

    Ok(rows
        .into_iter()
        .map(|(child_cbu_id, selector, created_at)| RawSlotRow {
            entity_id: Some(child_cbu_id),
            record_id: Some(child_cbu_id),
            filter_value: Some(selector),
            created_at,
        })
        .collect())
}

fn resolve_case_id(case_id: Option<Uuid>, raw: &RawHydrationData) -> Option<Uuid> {
    case_id.or_else(|| {
        raw.slot_rows
            .get("case")
            .and_then(|rows| rows.first())
            .and_then(|row| row.record_id)
    })
}

async fn query_case_rows(
    pool: &PgPool,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
) -> ConstellationResult<Vec<RawSlotRow>> {
    let rows = if let Some(case_id) = case_id {
        sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
            r#"
            SELECT case_id, opened_at
            FROM "ob-poc".cases
            WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_all(pool)
        .await
        .or_else(|err| {
            if is_missing_relation_error(&err, "cases") {
                Ok(Vec::new())
            } else {
                Err(err)
            }
        })
        .map_err(|err| ConstellationError::Other(err.into()))?
    } else {
        sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
            r#"
            SELECT case_id, opened_at
            FROM "ob-poc".cases
            WHERE cbu_id = $1
            ORDER BY opened_at DESC NULLS LAST
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_all(pool)
        .await
        .or_else(|err| {
            if is_missing_relation_error(&err, "cases") {
                Ok(Vec::new())
            } else {
                Err(err)
            }
        })
        .map_err(|err| ConstellationError::Other(err.into()))?
    };

    Ok(rows
        .into_iter()
        .map(|(record_id, created_at)| RawSlotRow {
            entity_id: None,
            record_id: Some(record_id),
            filter_value: None,
            created_at,
        })
        .collect())
}

async fn query_case_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
) -> ConstellationResult<Vec<RawSlotRow>> {
    let rows = if let Some(case_id) = case_id {
        sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
            r#"
            SELECT case_id, opened_at
            FROM "ob-poc".cases
            WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_all(&mut **tx)
        .await
        .or_else(|err| {
            if is_missing_relation_error(&err, "cases") {
                Ok(Vec::new())
            } else {
                Err(err)
            }
        })
        .map_err(|err| ConstellationError::Other(err.into()))?
    } else {
        sqlx::query_as::<_, (Uuid, Option<DateTime<Utc>>)>(
            r#"
            SELECT case_id, opened_at
            FROM "ob-poc".cases
            WHERE cbu_id = $1
            ORDER BY opened_at DESC NULLS LAST
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&mut **tx)
        .await
        .or_else(|err| {
            if is_missing_relation_error(&err, "cases") {
                Ok(Vec::new())
            } else {
                Err(err)
            }
        })
        .map_err(|err| ConstellationError::Other(err.into()))?
    };

    Ok(rows
        .into_iter()
        .map(|(record_id, created_at)| RawSlotRow {
            entity_id: None,
            record_id: Some(record_id),
            filter_value: None,
            created_at,
        })
        .collect())
}

async fn query_tollgate_rows(
    pool: &PgPool,
    case_id: Option<Uuid>,
) -> ConstellationResult<Vec<RawSlotRow>> {
    let Some(case_id) = case_id else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query_as::<_, (Uuid, bool, Option<DateTime<Utc>>)>(
        r#"
        SELECT DISTINCT ON (tollgate_id)
            evaluation_id, passed, evaluated_at
        FROM "ob-poc".tollgate_evaluations
        WHERE case_id = $1
        ORDER BY tollgate_id, evaluated_at DESC NULLS LAST
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "tollgate_evaluations") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .map_err(|err| ConstellationError::Other(err.into()))?;

    Ok(rows
        .into_iter()
        .map(|(record_id, passed, created_at)| RawSlotRow {
            entity_id: None,
            record_id: Some(record_id),
            filter_value: Some(if passed {
                String::from("passed")
            } else {
                String::from("failed")
            }),
            created_at,
        })
        .collect())
}

async fn query_tollgate_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    case_id: Option<Uuid>,
) -> ConstellationResult<Vec<RawSlotRow>> {
    let Some(case_id) = case_id else {
        return Ok(Vec::new());
    };

    let rows = sqlx::query_as::<_, (Uuid, bool, Option<DateTime<Utc>>)>(
        r#"
        SELECT DISTINCT ON (tollgate_id)
            evaluation_id, passed, evaluated_at
        FROM "ob-poc".tollgate_evaluations
        WHERE case_id = $1
        ORDER BY tollgate_id, evaluated_at DESC NULLS LAST
        "#,
    )
    .bind(case_id)
    .fetch_all(&mut **tx)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "tollgate_evaluations") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .map_err(|err| ConstellationError::Other(err.into()))?;

    Ok(rows
        .into_iter()
        .map(|(record_id, passed, created_at)| RawSlotRow {
            entity_id: None,
            record_id: Some(record_id),
            filter_value: Some(if passed {
                String::from("passed")
            } else {
                String::from("failed")
            }),
            created_at,
        })
        .collect())
}

async fn query_mandate_rows(pool: &PgPool, cbu_id: Uuid) -> ConstellationResult<Vec<RawSlotRow>> {
    let rows = sqlx::query_as::<_, (Uuid, String, i32, Option<DateTime<Utc>>)>(
        r#"
        SELECT profile_id, status, version, created_at
        FROM "ob-poc".cbu_trading_profiles
        WHERE cbu_id = $1
        ORDER BY
            CASE WHEN status = 'ACTIVE' THEN 0 ELSE 1 END,
            version DESC,
            created_at DESC NULLS LAST
        "#,
    )
    .bind(cbu_id)
    .fetch_all(pool)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "cbu_trading_profiles") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .map_err(|err| ConstellationError::Other(err.into()))?;

    Ok(rows
        .into_iter()
        .map(|(record_id, status, version, created_at)| RawSlotRow {
            entity_id: None,
            record_id: Some(record_id),
            filter_value: Some(format!("{status}:v{version}")),
            created_at,
        })
        .collect())
}

async fn query_mandate_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
) -> ConstellationResult<Vec<RawSlotRow>> {
    let rows = sqlx::query_as::<_, (Uuid, String, i32, Option<DateTime<Utc>>)>(
        r#"
        SELECT profile_id, status, version, created_at
        FROM "ob-poc".cbu_trading_profiles
        WHERE cbu_id = $1
        ORDER BY
            CASE WHEN status = 'ACTIVE' THEN 0 ELSE 1 END,
            version DESC,
            created_at DESC NULLS LAST
        "#,
    )
    .bind(cbu_id)
    .fetch_all(&mut **tx)
    .await
    .or_else(|err| {
        if is_missing_relation_error(&err, "cbu_trading_profiles") {
            Ok(Vec::new())
        } else {
            Err(err)
        }
    })
    .map_err(|err| ConstellationError::Other(err.into()))?;

    Ok(rows
        .into_iter()
        .map(|(record_id, status, version, created_at)| RawSlotRow {
            entity_id: None,
            record_id: Some(record_id),
            filter_value: Some(format!("{status}:v{version}")),
            created_at,
        })
        .collect())
}

async fn query_entity_graph_rows(
    pool: &PgPool,
    slot: &super::validate::ResolvedSlot,
    raw: &RawHydrationData,
) -> ConstellationResult<(Vec<RawSlotRow>, Vec<RawGraphEdge>)> {
    let seeds = slot
        .def
        .depends_on
        .iter()
        .filter_map(|dep| raw.slot_rows.get(dep.slot_name()))
        .flat_map(|rows| rows.iter())
        .filter_map(|row| row.entity_id)
        .collect::<Vec<_>>();

    if seeds.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let max_depth = slot.def.max_depth.unwrap_or(3) as i32;
    let rows = sqlx::query_as::<_, (Uuid, Uuid, Option<f64>, Option<String>, i32)>(
        r#"
        WITH RECURSIVE ownership_chain AS (
            SELECT
                er.from_entity_id,
                er.to_entity_id,
                er.percentage::float8,
                COALESCE(er.ownership_type, er.control_type, er.relationship_type) AS edge_type,
                1 AS depth
            FROM "ob-poc".entity_relationships_current er
            WHERE er.to_entity_id = ANY($1)
              AND er.relationship_type IN ('ownership', 'control', 'management')

            UNION ALL

            SELECT
                er.from_entity_id,
                er.to_entity_id,
                er.percentage::float8,
                COALESCE(er.ownership_type, er.control_type, er.relationship_type) AS edge_type,
                oc.depth + 1
            FROM ownership_chain oc
            JOIN "ob-poc".entity_relationships_current er
              ON er.to_entity_id = oc.from_entity_id
            WHERE oc.depth < $2
              AND er.relationship_type IN ('ownership', 'control', 'management')
        )
        SELECT from_entity_id, to_entity_id, percentage, edge_type, depth
        FROM ownership_chain
        "#,
    )
    .bind(&seeds)
    .bind(max_depth)
    .fetch_all(pool)
    .await
    .map_err(|err| ConstellationError::Other(err.into()))?;

    let mut nodes = seeds
        .iter()
        .copied()
        .map(|entity_id| RawSlotRow {
            entity_id: Some(entity_id),
            record_id: Some(entity_id),
            filter_value: None,
            created_at: None,
        })
        .collect::<Vec<_>>();
    let edges = rows
        .into_iter()
        .map(
            |(from_entity_id, to_entity_id, percentage, ownership_type, depth)| {
                nodes.push(RawSlotRow {
                    entity_id: Some(from_entity_id),
                    record_id: Some(from_entity_id),
                    filter_value: ownership_type.clone(),
                    created_at: None,
                });
                nodes.push(RawSlotRow {
                    entity_id: Some(to_entity_id),
                    record_id: Some(to_entity_id),
                    filter_value: ownership_type.clone(),
                    created_at: None,
                });
                RawGraphEdge {
                    from_entity_id,
                    to_entity_id,
                    percentage,
                    ownership_type,
                    depth: depth as usize,
                }
            },
        )
        .collect::<Vec<_>>();

    Ok((nodes, edges))
}

async fn query_entity_graph_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    slot: &super::validate::ResolvedSlot,
    raw: &RawHydrationData,
) -> ConstellationResult<(Vec<RawSlotRow>, Vec<RawGraphEdge>)> {
    let seeds = slot
        .def
        .depends_on
        .iter()
        .filter_map(|dep| raw.slot_rows.get(dep.slot_name()))
        .flat_map(|rows| rows.iter())
        .filter_map(|row| row.entity_id)
        .collect::<Vec<_>>();

    if seeds.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let max_depth = slot.def.max_depth.unwrap_or(5) as i32;
    let rows = sqlx::query_as::<_, (Uuid,)>(
        r#"
        WITH RECURSIVE ownership_tree AS (
            SELECT DISTINCT er.to_entity_id AS entity_id, 1 AS depth
            FROM "ob-poc".entity_relationships er
            WHERE er.from_entity_id = ANY($1)
              AND er.relationship_type = 'OWNERSHIP'
            UNION
            SELECT er.to_entity_id AS entity_id, ot.depth + 1
            FROM "ob-poc".entity_relationships er
            JOIN ownership_tree ot ON er.from_entity_id = ot.entity_id
            WHERE er.relationship_type = 'OWNERSHIP'
              AND ot.depth < $2
        )
        SELECT DISTINCT entity_id
        FROM ownership_tree
        "#,
    )
    .bind(&seeds)
    .bind(max_depth)
    .fetch_all(&mut **tx)
    .await
    .map_err(|err| ConstellationError::Other(err.into()))?;

    let edges = sqlx::query_as::<_, (Uuid, Uuid, Option<f64>, Option<String>, i32)>(
        r#"
        WITH RECURSIVE ownership_edges AS (
            SELECT
                er.from_entity_id,
                er.to_entity_id,
                er.percentage_owned,
                er.relationship_subtype,
                1 AS depth
            FROM "ob-poc".entity_relationships er
            WHERE er.from_entity_id = ANY($1)
              AND er.relationship_type = 'OWNERSHIP'
            UNION
            SELECT
                er.from_entity_id,
                er.to_entity_id,
                er.percentage_owned,
                er.relationship_subtype,
                oe.depth + 1
            FROM "ob-poc".entity_relationships er
            JOIN ownership_edges oe ON er.from_entity_id = oe.to_entity_id
            WHERE er.relationship_type = 'OWNERSHIP'
              AND oe.depth < $2
        )
        SELECT from_entity_id, to_entity_id, percentage_owned, relationship_subtype, depth
        FROM ownership_edges
        "#,
    )
    .bind(&seeds)
    .bind(max_depth)
    .fetch_all(&mut **tx)
    .await
    .map_err(|err| ConstellationError::Other(err.into()))?;

    Ok((
        rows.into_iter()
            .map(|(entity_id,)| RawSlotRow {
                entity_id: Some(entity_id),
                record_id: Some(entity_id),
                filter_value: None,
                created_at: None,
            })
            .collect(),
        edges
            .into_iter()
            .map(
                |(from_entity_id, to_entity_id, percentage, ownership_type, depth)| RawGraphEdge {
                    from_entity_id,
                    to_entity_id,
                    percentage,
                    ownership_type,
                    depth: depth as usize,
                },
            )
            .collect(),
    ))
}

async fn populate_entity_details_tx(
    tx: &mut Transaction<'_, Postgres>,
    raw: &mut RawHydrationData,
    rows: &[RawSlotRow],
) -> ConstellationResult<()> {
    let entity_ids = rows
        .iter()
        .filter_map(|row| row.entity_id)
        .filter(|entity_id| !raw.entity_details.contains_key(entity_id))
        .collect::<Vec<_>>();

    for entity_id in entity_ids {
        let detail = sqlx::query_as::<_, (String, Option<String>)>(
            r#"
            SELECT
                e.name,
                COALESCE(et.type_code, e.bods_entity_type, et.name) AS entity_type
            FROM "ob-poc".entities e
            LEFT JOIN "ob-poc".entity_types et
              ON et.entity_type_id = e.entity_type_id
            WHERE e.entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|err| ConstellationError::Other(err.into()))?;
        if let Some((name, entity_type)) = detail {
            raw.entity_details.insert(
                entity_id,
                serde_json::json!({
                    "name": name,
                    "entity_type": entity_type,
                }),
            );
            continue;
        }

        let cbu_detail = sqlx::query_as::<_, (String, Option<String>)>(
            r#"
            SELECT name, jurisdiction
            FROM "ob-poc".cbus
            WHERE cbu_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|err| ConstellationError::Other(err.into()))?;
        if let Some((name, jurisdiction)) = cbu_detail {
            raw.entity_details.insert(
                entity_id,
                serde_json::json!({
                    "name": name,
                    "entity_type": "cbu",
                    "jurisdiction": jurisdiction,
                }),
            );
        }
    }
    Ok(())
}

fn raw_overlay_rows_from_slot_data(
    entity_id: Uuid,
    overlays: crate::state_reducer::SlotOverlayData,
) -> Vec<RawOverlayRow> {
    overlays
        .sources
        .into_iter()
        .flat_map(|(source_name, rows)| {
            rows.into_iter().map(move |row| RawOverlayRow {
                entity_id: Some(entity_id),
                source_name: source_name.clone(),
                fields: serde_json::to_value(row.fields).unwrap_or_else(|_| serde_json::json!({})),
            })
        })
        .collect()
}

fn pick_deterministic_row(rows: &[RawSlotRow]) -> Option<&RawSlotRow> {
    rows.iter().max_by(|lhs, rhs| {
        lhs.created_at
            .cmp(&rhs.created_at)
            .then(lhs.filter_value.cmp(&rhs.filter_value))
            .then(lhs.entity_id.cmp(&rhs.entity_id))
            .then(lhs.record_id.cmp(&rhs.record_id))
    })
}

fn select_occurrence(rows: Vec<RawSlotRow>, occurrence: Option<usize>) -> Vec<RawSlotRow> {
    let Some(occurrence) = occurrence else {
        return rows;
    };
    if occurrence == 0 {
        return Vec::new();
    }
    rows.into_iter().nth(occurrence - 1).into_iter().collect()
}
