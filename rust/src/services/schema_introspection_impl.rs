//! ob-poc impl of [`dsl_runtime::service_traits::SchemaIntrospectionAccess`].
//!
//! Bridges the trait to:
//!  - `crate::ontology::ontology()` for entity defs / lifecycles / taxonomy
//!  - `crate::dsl_v2::verb_registry::registry()` for runtime verb registry
//!  - `crate::sem_reg::store::SnapshotStore` for `EntityTypeDef` snapshots
//!
//! Each method projects its source data onto the JSON shape that
//! `sem_os_schema_ops` consumer ops re-emit directly via
//! `VerbExecutionOutcome::Record` — no intermediate types.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use sqlx::PgPool;

use dsl_runtime::service_traits::SchemaIntrospectionAccess;

use crate::dsl_v2::verb_registry::registry;
use crate::ontology::{ontology, SearchKeyDef};
use crate::sem_reg::entity_type_def::EntityTypeDefBody;
use crate::sem_reg::store::SnapshotStore;
use crate::sem_reg::types::ObjectType;

pub struct ObPocSchemaIntrospectionAccess;

impl ObPocSchemaIntrospectionAccess {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ObPocSchemaIntrospectionAccess {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchemaIntrospectionAccess for ObPocSchemaIntrospectionAccess {
    fn resolve_entity_alias(&self, name: &str) -> String {
        ontology().resolve_alias(name).to_string()
    }

    fn ontology_entity_summary(&self, entity_type: &str) -> Option<serde_json::Value> {
        let ontology = ontology();
        let entity = ontology.get_entity(entity_type)?;
        Some(json!({
            "description": entity.description,
            "db_table": {
                "schema": entity.db.schema,
                "table": entity.db.table,
                "primary_key": entity.db.pk,
            },
        }))
    }

    fn ontology_entity_fields(&self, entity_type: &str) -> Vec<serde_json::Value> {
        let ontology = ontology();
        let Some(entity) = ontology.get_entity(entity_type) else {
            return Vec::new();
        };

        let mut seen = std::collections::BTreeSet::new();
        let mut fields = Vec::new();

        if seen.insert(entity.db.pk.clone()) {
            fields.push(json!({
                "name": entity.db.pk,
                "required": true,
                "source": "ontology.db.pk",
            }));
        }

        for key in &entity.search_keys {
            match key {
                SearchKeyDef::Single { column, .. } => {
                    if seen.insert(column.clone()) {
                        fields.push(json!({
                            "name": column,
                            "required": false,
                            "source": "ontology.search_key",
                        }));
                    }
                }
                SearchKeyDef::Composite { columns, .. } => {
                    for column in columns {
                        if seen.insert(column.clone()) {
                            fields.push(json!({
                                "name": column,
                                "required": false,
                                "source": "ontology.search_key",
                            }));
                        }
                    }
                }
            }
        }

        fields
    }

    fn ontology_entity_relationships(&self, entity_type: &str) -> Vec<serde_json::Value> {
        let ontology = ontology();
        let entity = ontology.get_entity(entity_type);
        let category = entity.map(|def| def.category.as_str());

        ontology
            .taxonomy()
            .relationships()
            .iter()
            .filter(|rel| {
                rel.parent == entity_type
                    || rel.child == entity_type
                    || category == Some(rel.parent.as_str())
                    || category == Some(rel.child.as_str())
            })
            .map(|rel| {
                json!({
                    "parent": rel.parent,
                    "child": rel.child,
                    "fk_arg": rel.fk_arg,
                    "description": rel.description,
                })
            })
            .collect()
    }

    fn domain_verbs(&self, domain: &str) -> Vec<serde_json::Value> {
        let mut verbs: Vec<_> = registry()
            .verbs_for_domain(domain)
            .into_iter()
            .map(|verb| {
                let required: Vec<&str> = verb.required_arg_names();
                let optional: Vec<&str> = verb.optional_arg_names();
                json!({
                    "verb_fqn": verb.full_name(),
                    "description": verb.description,
                    "required_args": required,
                    "optional_args": optional,
                })
            })
            .collect();
        verbs.sort_by(|a, b| a["verb_fqn"].as_str().cmp(&b["verb_fqn"].as_str()));
        verbs
    }

    async fn entity_type_snapshot(
        &self,
        pool: &PgPool,
        entity_type: &str,
    ) -> Result<Option<serde_json::Value>> {
        // Try bare name first, then namespaced "entity.<name>" form.
        for candidate in [entity_type.to_string(), format!("entity.{entity_type}")] {
            if let Some(row) = SnapshotStore::find_active_by_definition_field(
                pool,
                ObjectType::EntityTypeDef,
                "fqn",
                &candidate,
            )
            .await?
            {
                if let Ok(body) =
                    serde_json::from_value::<EntityTypeDefBody>(row.definition.clone())
                {
                    return Ok(Some(json!({
                        "description": body.description,
                        "domain": body.domain,
                        "db_table": body.db_table,
                        "required_attributes": body.required_attributes,
                        "optional_attributes": body.optional_attributes,
                    })));
                }
            }
        }
        Ok(None)
    }
}
