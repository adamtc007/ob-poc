//! PostgresChildEntityResolver — production [`ChildEntityResolver`].
//!
//! The CascadePlanner needs to know which child entities belong to a
//! given parent. The `parent_slot.join` block in the DAG declares this:
//!
//! ```yaml
//! parent_slot:
//!   workspace: cbu
//!   slot: cbu
//!   join:
//!     via: cbu_entity_relationships
//!     parent_fk: parent_cbu_id
//!     child_fk: child_cbu_id
//! ```
//!
//! This resolver reads that declaration from the registry at lookup
//! time and runs:
//!
//! ```sql
//! SELECT child_fk FROM "ob-poc".{via} WHERE {parent_fk} = $1
//! ```
//!
//! returning the list of child entity_ids.
//!
//! Identifier hygiene: all values come from the YAML schema and are
//! validator-checked (alphanumeric + underscore). Safe to interpolate.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use dsl_core::config::DagRegistry;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use super::hierarchy_cascade::ChildEntityResolver;

/// PostgresChildEntityResolver — resolves child entities via the DAG-
/// declared `parent_slot.join` block.
///
/// Holds an Arc to the DagRegistry so it can introspect the child
/// slot's `parent_slot` declaration at lookup time.
#[derive(Clone)]
pub struct PostgresChildEntityResolver {
    registry: Arc<DagRegistry>,
}

impl PostgresChildEntityResolver {
    pub fn new(registry: Arc<DagRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ChildEntityResolver for PostgresChildEntityResolver {
    async fn list_children(
        &self,
        parent_workspace: &str,
        parent_slot: &str,
        parent_entity_id: Uuid,
        child_workspace: &str,
        child_slot: &str,
        pool: &PgPool,
    ) -> Result<Vec<Uuid>> {
        // Look up the child slot's parent_slot declaration.
        let parent_ref = self
            .registry
            .parent_slot_for(child_workspace, child_slot)
            .ok_or_else(|| {
                anyhow!(
                    "child slot {}.{} has no parent_slot declared in DAG",
                    child_workspace,
                    child_slot
                )
            })?;

        // Verify the parent_slot points back to (parent_workspace, parent_slot).
        let resolved_ws = parent_ref.workspace.as_deref().unwrap_or(child_workspace);
        if resolved_ws != parent_workspace || parent_ref.slot != parent_slot {
            return Err(anyhow!(
                "child slot {}.{} parent_slot points to {}.{}, expected {}.{}",
                child_workspace,
                child_slot,
                resolved_ws,
                parent_ref.slot,
                parent_workspace,
                parent_slot,
            ));
        }

        let join = parent_ref.join.as_ref().ok_or_else(|| {
            anyhow!(
                "child slot {}.{} parent_slot has no `join:` block — \
                 cannot resolve children without via/parent_fk/child_fk",
                child_workspace,
                child_slot
            )
        })?;
        let via = join.via.as_deref().ok_or_else(|| {
            anyhow!(
                "child slot {}.{} parent_slot.join missing via",
                child_workspace,
                child_slot
            )
        })?;
        let parent_fk = join.parent_fk.as_deref().ok_or_else(|| {
            anyhow!(
                "child slot {}.{} parent_slot.join missing parent_fk",
                child_workspace,
                child_slot
            )
        })?;
        let child_fk = join.child_fk.as_deref().ok_or_else(|| {
            anyhow!(
                "child slot {}.{} parent_slot.join missing child_fk",
                child_workspace,
                child_slot
            )
        })?;

        // Hygiene: identifiers must be alphanumeric + underscore.
        if !is_safe_ident(via) || !is_safe_ident(parent_fk) || !is_safe_ident(child_fk) {
            return Err(anyhow!(
                "child slot {}.{} parent_slot.join contains non-identifier chars",
                child_workspace,
                child_slot
            ));
        }

        let sql =
            format!(r#"SELECT {child_fk}::text AS v FROM "ob-poc".{via} WHERE {parent_fk} = $1"#,);
        let rows = sqlx::query(&sql)
            .bind(parent_entity_id)
            .fetch_all(pool)
            .await?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            use sqlx::Row;
            let s: Option<String> = r.try_get::<Option<String>, _>("v").unwrap_or(None);
            if let Some(s) = s {
                if let Ok(id) = Uuid::parse_str(&s) {
                    out.push(id);
                }
            }
        }
        Ok(out)
    }
}

fn is_safe_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::config::dag::{Dag, LoadedDag};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn ws_dag(yaml: &str) -> LoadedDag {
        let dag: Dag = serde_yaml::from_str(yaml).unwrap();
        LoadedDag {
            source_path: PathBuf::new(),
            dag,
        }
    }

    fn registry_from(workspaces: &[(&str, &str)]) -> Arc<DagRegistry> {
        let mut map = BTreeMap::new();
        for (name, yaml) in workspaces {
            map.insert(name.to_string(), ws_dag(yaml));
        }
        Arc::new(DagRegistry::from_loaded(map))
    }

    #[test]
    fn is_safe_ident_basic() {
        assert!(is_safe_ident("cbu_id"));
        assert!(is_safe_ident("parent_cbu_id"));
        assert!(!is_safe_ident("a; DROP TABLE"));
        assert!(!is_safe_ident(""));
        assert!(!is_safe_ident("'foo"));
    }

    #[test]
    fn resolver_construction() {
        // Construct the resolver with a registry containing a slot
        // that has parent_slot + join. We exercise the SQL-formation
        // path in integration tests with a real DB.
        let r = registry_from(&[(
            "cbu",
            r#"
workspace: cbu
dag_id: cbu_dag
slots:
  - id: cbu
    stateless: false
    parent_slot:
      workspace: cbu
      slot: cbu
      join:
        via: cbu_entity_relationships
        parent_fk: parent_cbu_id
        child_fk: child_cbu_id
    state_machine: { id: cl, states: [{ id: VALIDATED }] }
"#,
        )]);
        let _resolver = PostgresChildEntityResolver::new(r);
    }
}
