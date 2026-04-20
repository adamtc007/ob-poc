//! Schema introspection access — projects ontology + verb registry +
//! sem_reg snapshot reads onto a JSON-shaped boundary.
//!
//! The `schema.*` verbs (`config/verbs/sem-reg/schema.yaml`) describe
//! entity types, their fields, relationships, and available DSL verbs.
//! In ob-poc this information is split across three modules:
//!  - `crate::ontology` — YAML-loaded entity defs / lifecycles / taxonomy
//!  - `crate::dsl_v2::verb_registry` — runtime verb registry (loaded from
//!    `config/verbs/**/*.yaml`)
//!  - `crate::sem_reg::store::SnapshotStore` — published `EntityTypeDef`
//!    snapshots in `sem_reg.snapshots`
//!
//! Rather than relocate all three modules into `dsl-runtime` (~3000+
//! LOC across 14+ ob-poc consumers), this trait projects the
//! schema_ops-specific subset onto JSON-shaped values that the
//! consumer ops can re-emit directly via
//! `VerbExecutionOutcome::Record`. Slice #9 lesson: when both
//! consumer ops wrap their result as `Record(serde_json::Value)`,
//! the trait can return `Value` directly and dodge types-extraction.
//!
//! Introduced in Phase 5a composite-blocker #25 for `sem_os_schema_ops`.
//! The ob-poc bridge (`ObPocSchemaIntrospectionAccess`) reads from
//! `crate::ontology::ontology()`, `crate::dsl_v2::verb_registry::registry()`,
//! and `crate::sem_reg::store::SnapshotStore`. Consumers obtain the
//! impl via [`crate::VerbExecutionContext::service::<dyn SchemaIntrospectionAccess>`].

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

/// Read-only schema introspection projecting ontology, verb registry,
/// and sem_reg snapshot reads through a single JSON-shaped surface.
#[async_trait]
pub trait SchemaIntrospectionAccess: Send + Sync {
    /// Resolve a possibly-aliased entity-type name to its canonical
    /// form (e.g., `"documents"` → `"document"`). Returns the input
    /// unchanged when no alias is registered.
    fn resolve_entity_alias(&self, name: &str) -> String;

    /// Return the ontology-derived summary for an entity type, or
    /// `None` if the type is unknown to the ontology. Shape:
    ///   `{ description, db_table: { schema, table, primary_key } }`
    /// — the consumer ops merge this with sem_reg snapshot data.
    fn ontology_entity_summary(&self, entity_type: &str) -> Option<serde_json::Value>;

    /// Return ontology-derived fields for an entity type
    /// (primary key + search-key columns), each shaped as
    ///   `{ name, required, source }`.
    fn ontology_entity_fields(&self, entity_type: &str) -> Vec<serde_json::Value>;

    /// Return ontology-derived relationships involving the entity
    /// type (FK edges where the entity is parent or child, plus
    /// edges to its ontology category). Each relationship shaped as
    ///   `{ parent, child, fk_arg, description }`.
    fn ontology_entity_relationships(&self, entity_type: &str) -> Vec<serde_json::Value>;

    /// Return the runtime verb-registry summary for a domain. Each
    /// verb shaped as `{ verb_fqn, description, required_args[], optional_args[] }`.
    /// Returned list is sorted by `verb_fqn`.
    fn domain_verbs(&self, domain: &str) -> Vec<serde_json::Value>;

    /// Look up an active `EntityTypeDef` snapshot from `sem_reg.snapshots`.
    /// Tries `entity_type` first, then `"entity.{entity_type}"` to
    /// match either the bare or namespaced FQN convention. Returns
    /// the deserialised body shape (`{description, domain, db_table?,
    /// required_attributes[], optional_attributes[]}`) when found.
    async fn entity_type_snapshot(
        &self,
        pool: &PgPool,
        entity_type: &str,
    ) -> Result<Option<serde_json::Value>>;
}
