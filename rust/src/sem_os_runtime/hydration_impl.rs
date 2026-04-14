use sem_os_postgres::constellation_hydration as pg_hydration;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::sem_os_runtime::constellation_runtime::{
    compute_action_surface, compute_summary, Cardinality, ConstellationError, ConstellationResult,
    ConstellationSlotContext, ConstellationSummary, HydratedConstellation, JoinDef, RawGraphEdge,
    RawHydrationData, RawOverlayRow, RawSlotRow, ResolvedSlot, SlotType, ValidatedConstellationMap,
};
use crate::sem_os_runtime::reducer_runtime::{
    build_eval_scope, collect_slot_runtime_artifacts, ReducerOverlayData, RuntimeEvalScope,
};

fn slot_rows_from_postgres(rows: Vec<pg_hydration::HydrationSlotRow>) -> Vec<RawSlotRow> {
    rows.into_iter()
        .map(|row| RawSlotRow {
            entity_id: row.entity_id,
            record_id: row.record_id,
            filter_value: row.filter_value,
            created_at: row.created_at,
        })
        .collect()
}

fn entity_details_from_postgres(
    details: Vec<pg_hydration::HydrationEntityDetail>,
) -> Vec<(Uuid, serde_json::Value)> {
    details
        .into_iter()
        .map(|detail| (detail.entity_id, detail.payload))
        .collect()
}

fn graph_edges_from_postgres(edges: Vec<pg_hydration::HydrationGraphEdge>) -> Vec<RawGraphEdge> {
    edges
        .into_iter()
        .map(|edge| RawGraphEdge {
            from_entity_id: edge.from_entity_id,
            to_entity_id: edge.to_entity_id,
            percentage: edge.percentage,
            ownership_type: edge.ownership_type,
            depth: edge.depth,
        })
        .collect()
}

/// Hydrate, normalize, and enrich a constellation from the database.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_os_runtime::constellation_runtime::{hydrate_constellation, load_builtin_constellation_map};
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

    let mut raw = RawHydrationData {
        root: Some(cbu_id),
        ..Default::default()
    };
    let scope = build_eval_scope(&mut tx, cbu_id, case_id)
        .await
        .map_err(|err| ConstellationError::Execution(format!("build_eval_scope failed: {err}")))?;

    for slot in &map.slots_ordered {
        hydrate_slot_tx(&mut tx, cbu_id, case_id, map, slot, &scope, &mut raw)
            .await
            .map_err(|err| {
                ConstellationError::Execution(format!("slot '{}' failed: {err}", slot.name))
            })?;
    }

    let normalized =
        crate::sem_os_runtime::normalize_impl::normalize_slots(map, cbu_id, case_id, raw);
    Ok(compute_action_surface(map, normalized))
}

async fn hydrate_slot_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    map: &ValidatedConstellationMap,
    slot: &ResolvedSlot,
    scope: &RuntimeEvalScope,
    raw: &mut RawHydrationData,
) -> ConstellationResult<()> {
    let rows = load_slot_rows_tx(tx, cbu_id, case_id, slot, raw)
        .await
        .map_err(|err| {
            ConstellationError::Execution(format!(
                "load_slot_rows failed for '{}': {err}",
                slot.name
            ))
        })?;
    raw.slot_rows.insert(slot.name.clone(), rows.clone());
    populate_entity_details_tx(tx, raw, &rows)
        .await
        .map_err(|err| {
            ConstellationError::Execution(format!(
                "populate_entity_details failed for '{}': {err}",
                slot.name
            ))
        })?;

    if let Some(entity_id) = pick_deterministic_row(&rows).and_then(|row| row.entity_id) {
        let Some(machine_name) = slot.def.state_machine.as_ref() else {
            if let Err(err) =
                hydrate_slot_overlays_tx(tx, cbu_id, case_id, raw, slot, entity_id).await
            {
                tracing::warn!(
                    slot = %slot.name,
                    %entity_id,
                    error = %err,
                    "Overlay hydration failed for non-state-machine slot — continuing"
                );
            }
            return Ok(());
        };
        let machine = map.state_machines.get(machine_name).ok_or_else(|| {
            ConstellationError::Execution(format!(
                "missing loaded state machine '{}'",
                machine_name
            ))
        })?;
        let artifacts = collect_slot_runtime_artifacts(
            tx,
            cbu_id,
            case_id,
            entity_id,
            &slot.name,
            Some(machine),
            Some(scope),
        )
        .await
        .map_err(|err| {
            ConstellationError::Execution(format!(
                "collect_slot_runtime_artifacts failed for '{}' entity {}: {err}",
                slot.name, entity_id
            ))
        })?;
        let derived = artifacts
            .reducer_result
            .ok_or_else(|| ConstellationError::Execution("missing reducer result".into()))?;
        raw.warnings
            .insert(slot.name.clone(), derived.consistency_warnings.clone());
        raw.reducer_results.insert(slot.name.clone(), derived);
        if let Err(err) = hydrate_slot_overlays_tx(tx, cbu_id, case_id, raw, slot, entity_id).await
        {
            tracing::warn!(
                slot = %slot.name,
                %entity_id,
                error = %err,
                "Overlay hydration writeback failed after reducer evaluation — continuing"
            );
        }
    }

    Ok(())
}

async fn load_slot_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    slot: &ResolvedSlot,
    raw: &mut RawHydrationData,
) -> ConstellationResult<Vec<RawSlotRow>> {
    if slot.def.slot_type == SlotType::EntityGraph {
        let (rows, edges) = query_entity_graph_rows_tx(tx, slot, raw).await?;
        raw.graph_edges.insert(slot.name.clone(), edges);
        Ok(rows)
    } else {
        query_slot_rows_tx(tx, cbu_id, case_id, slot, raw).await
    }
}

async fn hydrate_slot_overlays_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    raw: &mut RawHydrationData,
    slot: &ResolvedSlot,
    entity_id: Uuid,
) -> ConstellationResult<()> {
    let artifacts =
        collect_slot_runtime_artifacts(tx, cbu_id, case_id, entity_id, &slot.name, None, None)
            .await
            .map_err(ConstellationError::Other)?;
    let overlays = artifacts.overlays;
    raw.overlay_rows.insert(
        slot.name.clone(),
        raw_overlay_rows_from_slot_data(entity_id, overlays),
    );
    Ok(())
}

/// Compute just the high-level summary for a constellation.
///
/// # Examples
/// ```rust,no_run
/// # async fn demo(pool: &sqlx::PgPool) -> anyhow::Result<()> {
/// use uuid::Uuid;
/// use ob_poc::sem_os_runtime::constellation_runtime::{hydrate_constellation_summary, load_builtin_constellation_map};
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
/// use ob_poc::sem_os_runtime::constellation_runtime::{discover_state_machine_slot_contexts, load_builtin_constellation_map};
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
        let rows = if slot.def.slot_type == SlotType::EntityGraph {
            let (rows, edges) = query_entity_graph_rows(pool, slot, &raw).await?;
            raw.graph_edges.insert(slot.name.clone(), edges);
            rows
        } else {
            query_slot_rows(pool, cbu_id, case_id, slot, &raw).await?
        };
        raw.slot_rows.insert(slot.name.clone(), rows.clone());

        if slot.def.state_machine.as_deref() == Some(state_machine_name) {
            match slot.def.slot_type {
                SlotType::Entity => {
                    contexts.extend(rows.iter().filter_map(|row| {
                        row.entity_id.map(|entity_id| ConstellationSlotContext {
                            slot_path: slot.name.clone(),
                            entity_id,
                            slot_type: String::from("entity"),
                            cardinality: format!("{:?}", slot.def.cardinality).to_lowercase(),
                        })
                    }));
                }
                SlotType::EntityGraph => {
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
    slot: &ResolvedSlot,
    raw: &RawHydrationData,
) -> ConstellationResult<Vec<RawSlotRow>> {
    match slot.def.slot_type {
        SlotType::Cbu => {
            if slot.def.cardinality == Cardinality::Root {
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
        SlotType::Entity => {
            if let Some(join) = &slot.def.join {
                if join.via == "cbu_entity_roles" {
                    let rows = pg_hydration::query_role_entity_rows(
                        pool,
                        cbu_id,
                        join.filter_value.as_deref().unwrap_or_default(),
                    )
                    .await
                    .map(slot_rows_from_postgres)
                    .map_err(ConstellationError::Other)?;
                    return Ok(select_occurrence(rows, slot.def.occurrence));
                }
                // Unrecognised via — log so silent empties are visible.
                // This is a known limitation: only cbu_entity_roles has a
                // dedicated hydration query. Other entity slots need their
                // own query paths or a generic join implementation.
                tracing::debug!(
                    slot = %slot.name,
                    via = %join.via,
                    "Entity slot has unhandled join.via — returning empty (hydration not implemented for this table)"
                );
            }
            Ok(Vec::new())
        }
        SlotType::Case => query_case_rows(pool, cbu_id, case_id).await,
        SlotType::Tollgate => {
            let active_case_id = resolve_case_id(case_id, raw);
            query_tollgate_rows(pool, active_case_id).await
        }
        SlotType::Mandate => query_mandate_rows(pool, cbu_id).await,
        SlotType::EntityGraph => query_entity_graph_rows(pool, slot, raw)
            .await
            .map(|(rows, _)| rows),
    }
}

async fn query_slot_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
    slot: &ResolvedSlot,
    raw: &RawHydrationData,
) -> ConstellationResult<Vec<RawSlotRow>> {
    match slot.def.slot_type {
        SlotType::Cbu => {
            if slot.def.cardinality == Cardinality::Root {
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
        SlotType::Entity => {
            if let Some(join) = &slot.def.join {
                if join.via == "cbu_entity_roles" {
                    let rows = pg_hydration::query_role_entity_rows(
                        &mut **tx,
                        cbu_id,
                        join.filter_value.as_deref().unwrap_or_default(),
                    )
                    .await
                    .map(slot_rows_from_postgres)
                    .map_err(ConstellationError::Other)?;
                    return Ok(select_occurrence(rows, slot.def.occurrence));
                }
                tracing::debug!(
                    slot = %slot.name,
                    via = %join.via,
                    "Entity slot has unhandled join.via — returning empty (hydration not implemented for this table)"
                );
            }
            Ok(Vec::new())
        }
        SlotType::Case => query_case_rows_tx(tx, cbu_id, case_id).await,
        SlotType::Tollgate => {
            let active_case_id = resolve_case_id(case_id, raw);
            query_tollgate_rows_tx(tx, active_case_id).await
        }
        SlotType::Mandate => query_mandate_rows_tx(tx, cbu_id).await,
        SlotType::EntityGraph => query_entity_graph_rows_tx(tx, slot, raw)
            .await
            .map(|(rows, _)| rows),
    }
}

async fn query_child_cbu_rows(
    pool: &PgPool,
    parent_cbu_id: Uuid,
    join: &JoinDef,
) -> ConstellationResult<Vec<RawSlotRow>> {
    pg_hydration::query_child_cbu_rows(
        pool,
        parent_cbu_id,
        join.filter_column.as_deref(),
        join.filter_value.as_deref(),
    )
    .await
    .map(slot_rows_from_postgres)
    .map_err(ConstellationError::Other)
}

async fn query_child_cbu_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    parent_cbu_id: Uuid,
    join: &JoinDef,
) -> ConstellationResult<Vec<RawSlotRow>> {
    pg_hydration::query_child_cbu_rows(
        &mut **tx,
        parent_cbu_id,
        join.filter_column.as_deref(),
        join.filter_value.as_deref(),
    )
    .await
    .map(slot_rows_from_postgres)
    .map_err(ConstellationError::Other)
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
    pg_hydration::query_case_rows(pool, cbu_id, case_id)
        .await
        .map(slot_rows_from_postgres)
        .map_err(ConstellationError::Other)
}

async fn query_case_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
    case_id: Option<Uuid>,
) -> ConstellationResult<Vec<RawSlotRow>> {
    pg_hydration::query_case_rows(&mut **tx, cbu_id, case_id)
        .await
        .map(slot_rows_from_postgres)
        .map_err(ConstellationError::Other)
}

async fn query_tollgate_rows(
    pool: &PgPool,
    case_id: Option<Uuid>,
) -> ConstellationResult<Vec<RawSlotRow>> {
    pg_hydration::query_tollgate_rows(pool, case_id)
        .await
        .map(slot_rows_from_postgres)
        .map_err(ConstellationError::Other)
}

async fn query_tollgate_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    case_id: Option<Uuid>,
) -> ConstellationResult<Vec<RawSlotRow>> {
    pg_hydration::query_tollgate_rows(&mut **tx, case_id)
        .await
        .map(slot_rows_from_postgres)
        .map_err(ConstellationError::Other)
}

async fn query_mandate_rows(pool: &PgPool, cbu_id: Uuid) -> ConstellationResult<Vec<RawSlotRow>> {
    pg_hydration::query_mandate_rows(pool, cbu_id)
        .await
        .map(slot_rows_from_postgres)
        .map_err(ConstellationError::Other)
}

async fn query_mandate_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    cbu_id: Uuid,
) -> ConstellationResult<Vec<RawSlotRow>> {
    pg_hydration::query_mandate_rows(&mut **tx, cbu_id)
        .await
        .map(slot_rows_from_postgres)
        .map_err(ConstellationError::Other)
}

async fn query_entity_graph_rows(
    pool: &PgPool,
    slot: &ResolvedSlot,
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

    pg_hydration::query_entity_graph_rows(pool, &seeds, slot.def.max_depth.unwrap_or(3) as i32)
        .await
        .map(|(rows, edges)| {
            (
                slot_rows_from_postgres(rows),
                graph_edges_from_postgres(edges),
            )
        })
        .map_err(ConstellationError::Other)
}

async fn query_entity_graph_rows_tx(
    tx: &mut Transaction<'_, Postgres>,
    slot: &ResolvedSlot,
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

    pg_hydration::query_entity_graph_rows_tx(tx, &seeds, slot.def.max_depth.unwrap_or(5) as i32)
        .await
        .map(|(rows, edges)| {
            (
                slot_rows_from_postgres(rows),
                graph_edges_from_postgres(edges),
            )
        })
        .map_err(ConstellationError::Other)
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

    for (entity_id, payload) in entity_details_from_postgres(
        pg_hydration::query_entity_details(tx, &entity_ids)
            .await
            .map_err(ConstellationError::Other)?,
    ) {
        raw.entity_details.insert(entity_id, payload);
    }
    Ok(())
}

fn raw_overlay_rows_from_slot_data(
    entity_id: Uuid,
    overlays: ReducerOverlayData,
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
