use std::collections::BTreeMap;

use super::map_def::{Cardinality, SlotType};
use super::validate::ValidatedConstellationMap;

/// Batch hydration plan grouped by slot depth.
#[derive(Debug, Clone)]
pub struct HydrationQueryPlan {
    pub levels: Vec<QueryLevel>,
}

/// Queries to execute for a specific depth.
#[derive(Debug, Clone)]
pub struct QueryLevel {
    pub depth: usize,
    pub queries: Vec<SlotQuery>,
}

/// Single compiled query for a slot.
#[derive(Debug, Clone)]
pub struct SlotQuery {
    pub slot_name: String,
    pub query_type: QueryType,
    pub sql: String,
}

/// Query shape used to hydrate a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryType {
    FetchOne,
    JoinLookup,
    BatchFetch,
    RecursiveCte { max_depth: usize },
    OverlayBatch,
}

/// Compile a coarse query plan for a validated map.
///
/// # Examples
/// ```rust
/// use ob_poc::sem_reg::constellation::{compile_query_plan, load_builtin_constellation_map};
///
/// let map = load_builtin_constellation_map("struct.lux.ucits.sicav").unwrap();
/// let plan = compile_query_plan(&map);
/// assert!(!plan.levels.is_empty());
/// ```
pub fn compile_query_plan(map: &ValidatedConstellationMap) -> HydrationQueryPlan {
    let mut levels = BTreeMap::<usize, Vec<SlotQuery>>::new();
    let role_batch = compile_role_batch_query(map);

    for slot in &map.slots_ordered {
        let sql = if let Some(sql) = role_batch.get(&slot.name) {
            sql.clone()
        } else {
            compile_slot_sql(slot)
        };
        levels.entry(slot.depth).or_default().push(SlotQuery {
            slot_name: slot.name.clone(),
            query_type: query_type_for_slot(slot),
            sql,
        });
        if !slot.def.overlays.is_empty() || !slot.def.edge_overlays.is_empty() {
            levels.entry(slot.depth).or_default().push(SlotQuery {
                slot_name: format!("{}.overlays", slot.name),
                query_type: QueryType::OverlayBatch,
                sql: compile_overlay_sql(slot),
            });
        }
    }

    HydrationQueryPlan {
        levels: levels
            .into_iter()
            .map(|(depth, queries)| QueryLevel { depth, queries })
            .collect(),
    }
}

fn compile_slot_sql(slot: &super::validate::ResolvedSlot) -> String {
    match slot.def.cardinality {
        Cardinality::Root => format!(
            "SELECT {pk} FROM {table} WHERE {pk} = $1",
            pk = slot.def.pk.as_deref().unwrap_or("id"),
            table = slot.def.table.as_deref().unwrap_or("unknown")
        ),
        Cardinality::Recursive => format!(
            "WITH RECURSIVE slot_graph AS (...) SELECT * FROM {table} /* max_depth={depth} */",
            table = slot
                .def
                .join
                .as_ref()
                .map(|join| join.via.as_str())
                .unwrap_or("unknown"),
            depth = slot.def.max_depth.unwrap_or(0)
        ),
        Cardinality::Mandatory | Cardinality::Optional => format!(
            "SELECT * FROM {table} /* via {via} */",
            table = slot
                .def
                .table
                .as_deref()
                .or_else(|| slot.def.join.as_ref().map(|join| join.via.as_str()))
                .unwrap_or("unknown"),
            via = slot
                .def
                .join
                .as_ref()
                .map(|join| join.via.as_str())
                .unwrap_or("none")
        ),
    }
}

fn query_type_for_slot(slot: &super::validate::ResolvedSlot) -> QueryType {
    match slot.def.cardinality {
        Cardinality::Root => QueryType::FetchOne,
        Cardinality::Recursive => QueryType::RecursiveCte {
            max_depth: slot.def.max_depth.unwrap_or(1),
        },
        Cardinality::Mandatory | Cardinality::Optional => {
            if slot.def.slot_type == SlotType::Entity && slot.def.join.is_some() {
                QueryType::BatchFetch
            } else {
                QueryType::JoinLookup
            }
        }
    }
}

fn compile_overlay_sql(slot: &super::validate::ResolvedSlot) -> String {
    let mut sources = slot.def.overlays.clone();
    if !slot.def.edge_overlays.is_empty() {
        sources.extend(slot.def.edge_overlays.iter().map(|source| format!("edge:{source}")));
    }
    format!(
        "/* overlay batch for slot {} */ SELECT {}",
        slot.name,
        sources.join(", ")
    )
}

fn compile_role_batch_query(map: &ValidatedConstellationMap) -> BTreeMap<String, String> {
    let role_slots = map
        .slots_ordered
        .iter()
        .filter(|slot| {
            slot.def.slot_type == SlotType::Entity
                && slot
                    .def
                    .join
                    .as_ref()
                    .map(|join| join.via == "cbu_entity_roles")
                    .unwrap_or(false)
        })
        .collect::<Vec<_>>();

    if role_slots.len() < 2 {
        return BTreeMap::new();
    }

    let filters = role_slots
        .iter()
        .filter_map(|slot| {
            slot.def.join.as_ref().and_then(|join| {
                join.filter_value
                    .as_ref()
                    .map(|value| format!("'{}' AS {}", value, slot.name.replace('.', "_")))
            })
        })
        .collect::<Vec<_>>()
        .join(", ");

    role_slots
        .into_iter()
        .map(|slot| {
            (
                slot.name.clone(),
                format!(
                    "SELECT cer.cbu_id, cer.entity_id, r.name AS role_name, {} \
                     FROM \"ob-poc\".cbu_entity_roles cer \
                     JOIN \"ob-poc\".roles r ON r.role_id = cer.role_id \
                     WHERE cer.cbu_id = $1",
                    filters
                ),
            )
        })
        .collect()
}
