//! Op enum - Primitive operations that verbs compile to
//!
//! # Design Principles
//!
//! 1. **Ops are nodes, not entities** - The DAG contains Op nodes. Entities are
//!    just identifiers that Ops reference.
//!
//! 2. **Two-phase FK strategy** - Create entities with null FKs first, then
//!    populate FKs with SetFK ops. This eliminates most circular dependencies.
//!
//! 3. **Source tracking** - Every Op tracks its source statement index for
//!    error mapping back to DSL source lines.
//!
//! 4. **Stable sort** - When ops have no dependency relationship, preserve
//!    source order to prevent LSP thrashing.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for an entity in the execution plan
///
/// This is NOT the database UUID - it's a compile-time key used to track
/// dependencies between ops. The actual UUID is resolved at execution time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityKey {
    /// Entity type: "cbu", "proper_person", "limited_company", etc.
    pub entity_type: String,
    /// Natural key or symbol name (without @)
    pub key: String,
}

impl EntityKey {
    pub fn new(entity_type: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            entity_type: entity_type.into(),
            key: key.into(),
        }
    }

    /// Create from a symbol reference (strips @ prefix if present)
    pub fn from_symbol(symbol: &str) -> Self {
        let key = symbol.strip_prefix('@').unwrap_or(symbol);
        Self {
            entity_type: "symbol".to_string(),
            key: key.to_string(),
        }
    }

    /// Create for a CBU
    pub fn cbu(name: impl Into<String>) -> Self {
        Self::new("cbu", name)
    }

    /// Create for a proper person
    pub fn proper_person(name: impl Into<String>) -> Self {
        Self::new("proper_person", name)
    }

    /// Create for a limited company
    pub fn limited_company(name: impl Into<String>) -> Self {
        Self::new("limited_company", name)
    }
}

impl std::fmt::Display for EntityKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.entity_type, self.key)
    }
}

/// Document key for trading profiles and other documents
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocKey {
    pub doc_type: String,
    pub key: String,
}

impl DocKey {
    pub fn new(doc_type: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            doc_type: doc_type.into(),
            key: key.into(),
        }
    }

    pub fn trading_profile(cbu_key: &str) -> Self {
        Self::new("trading_profile", cbu_key)
    }
}

impl std::fmt::Display for DocKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.doc_type, self.key)
    }
}

/// Primitive operations that verbs compile to
///
/// These are the nodes in the execution DAG. Dependencies are computed
/// based on what each Op produces and consumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Op {
    // =========================================================================
    // Entity Operations (Phase 1 in execution)
    // =========================================================================
    /// Create or update an entity
    ///
    /// Creates with null FKs - FKs are populated by SetFK ops in phase 2.
    EnsureEntity {
        entity_type: String,
        key: EntityKey,
        attrs: HashMap<String, serde_json::Value>,
        /// Binding name if `:as @name` was used
        binding: Option<String>,
        /// Source statement index for error reporting
        source_stmt: usize,
    },

    // =========================================================================
    // Relationship Operations (Phase 2 in execution)
    // =========================================================================
    /// Set a foreign key on an entity
    ///
    /// Used for FKs that reference other entities created in the same DSL.
    SetFK {
        source: EntityKey,
        field: String,
        target: EntityKey,
        source_stmt: usize,
    },

    /// Link role to entity within CBU
    LinkRole {
        cbu: EntityKey,
        entity: EntityKey,
        role: String,
        /// Optional ownership percentage for ownership roles
        ownership_percentage: Option<Decimal>,
        source_stmt: usize,
    },

    /// Remove role from entity within CBU
    UnlinkRole {
        cbu: EntityKey,
        entity: EntityKey,
        role: String,
        source_stmt: usize,
    },

    /// Add ownership relationship
    AddOwnership {
        owner: EntityKey,
        owned: EntityKey,
        percentage: Decimal,
        ownership_type: String,
        source_stmt: usize,
    },

    /// Register UBO determination
    RegisterUBO {
        cbu: EntityKey,
        subject: EntityKey,
        ubo_person: EntityKey,
        qualifying_reason: String,
        ownership_percentage: Option<Decimal>,
        source_stmt: usize,
    },

    // =========================================================================
    // Document Operations
    // =========================================================================
    /// Upload/upsert a document (e.g., trading profile YAML)
    UpsertDoc {
        key: DocKey,
        content: serde_json::Value,
        /// Reference to CBU this document belongs to
        cbu: Option<EntityKey>,
        source_stmt: usize,
    },

    /// Attach evidence to CBU
    AttachEvidence {
        cbu: EntityKey,
        evidence_type: String,
        document: Option<DocKey>,
        attestation_ref: Option<String>,
        source_stmt: usize,
    },

    // =========================================================================
    // KYC Operations
    // =========================================================================
    /// Create KYC case
    CreateCase {
        cbu: EntityKey,
        case_type: String,
        binding: Option<String>,
        source_stmt: usize,
    },

    /// Update case status
    UpdateCaseStatus {
        case: EntityKey,
        status: String,
        source_stmt: usize,
    },

    /// Create entity workstream within a case
    CreateWorkstream {
        case: EntityKey,
        entity: EntityKey,
        binding: Option<String>,
        source_stmt: usize,
    },

    /// Run screening on entity
    RunScreening {
        workstream: EntityKey,
        screening_type: String,
        source_stmt: usize,
    },

    // =========================================================================
    // Custody Operations
    // =========================================================================
    /// Add to trading universe
    AddUniverse {
        cbu: EntityKey,
        instrument_class: String,
        market: Option<String>,
        currencies: Vec<String>,
        settlement_types: Vec<String>,
        source_stmt: usize,
    },

    /// Create SSI
    CreateSSI {
        cbu: EntityKey,
        name: String,
        ssi_type: String,
        attrs: HashMap<String, serde_json::Value>,
        binding: Option<String>,
        source_stmt: usize,
    },

    /// Add booking rule
    AddBookingRule {
        cbu: EntityKey,
        ssi: EntityKey,
        name: String,
        priority: i32,
        criteria: HashMap<String, serde_json::Value>,
        source_stmt: usize,
    },

    // =========================================================================
    // Materialization Operations (Phase 3 in execution)
    // =========================================================================
    /// Materialize trading profile to operational tables
    ///
    /// Dependencies: UpsertDoc for the profile, and optionally entities
    /// referenced in the profile (but only if they're in THIS DSL program).
    Materialize {
        source: DocKey,
        sections: Vec<String>,
        force: bool,
        source_stmt: usize,
    },

    // =========================================================================
    // Reference Lookups (validation only, no execution)
    // =========================================================================
    /// Placeholder for reference data that must exist
    ///
    /// Doesn't execute anything but declares that this ref must exist.
    /// Used for validation - if the ref doesn't exist, compilation fails.
    RequireRef {
        ref_type: String,
        value: String,
        source_stmt: usize,
    },

    // =========================================================================
    // Generic CRUD Operations
    // =========================================================================
    /// Generic CRUD operation for YAML-defined verbs
    ///
    /// Used for verbs with `behavior: crud` that don't need special Op handling.
    /// The executor routes these to GenericCrudExecutor based on the verb name.
    GenericCrud {
        /// Full verb name: "domain.verb"
        verb: String,
        /// Arguments as JSON for generic executor
        args: HashMap<String, serde_json::Value>,
        /// Dependencies on other entities (by symbol name)
        depends_on: Vec<EntityKey>,
        /// Binding name if `:as @name` was used
        binding: Option<String>,
        /// Source statement index
        source_stmt: usize,
    },
}

impl Op {
    /// Get the source statement index for error reporting
    pub fn source_stmt(&self) -> usize {
        match self {
            Op::EnsureEntity { source_stmt, .. } => *source_stmt,
            Op::SetFK { source_stmt, .. } => *source_stmt,
            Op::LinkRole { source_stmt, .. } => *source_stmt,
            Op::UnlinkRole { source_stmt, .. } => *source_stmt,
            Op::AddOwnership { source_stmt, .. } => *source_stmt,
            Op::RegisterUBO { source_stmt, .. } => *source_stmt,
            Op::UpsertDoc { source_stmt, .. } => *source_stmt,
            Op::AttachEvidence { source_stmt, .. } => *source_stmt,
            Op::CreateCase { source_stmt, .. } => *source_stmt,
            Op::UpdateCaseStatus { source_stmt, .. } => *source_stmt,
            Op::CreateWorkstream { source_stmt, .. } => *source_stmt,
            Op::RunScreening { source_stmt, .. } => *source_stmt,
            Op::AddUniverse { source_stmt, .. } => *source_stmt,
            Op::CreateSSI { source_stmt, .. } => *source_stmt,
            Op::AddBookingRule { source_stmt, .. } => *source_stmt,
            Op::Materialize { source_stmt, .. } => *source_stmt,
            Op::RequireRef { source_stmt, .. } => *source_stmt,
            Op::GenericCrud { source_stmt, .. } => *source_stmt,
        }
    }

    /// Get a human-readable description for plan output
    pub fn describe(&self) -> String {
        match self {
            Op::EnsureEntity {
                entity_type, key, ..
            } => {
                format!("Ensure {} '{}'", entity_type, key.key)
            }
            Op::SetFK {
                source,
                field,
                target,
                ..
            } => {
                format!("Set {}.{} → {}", source.key, field, target.key)
            }
            Op::LinkRole {
                cbu, entity, role, ..
            } => {
                format!("Link {} to {} as {}", entity.key, cbu.key, role)
            }
            Op::UnlinkRole {
                cbu, entity, role, ..
            } => {
                format!("Unlink {} from {} as {}", entity.key, cbu.key, role)
            }
            Op::AddOwnership {
                owner,
                owned,
                percentage,
                ..
            } => {
                format!("{} owns {}% of {}", owner.key, percentage, owned.key)
            }
            Op::RegisterUBO {
                cbu,
                ubo_person,
                qualifying_reason,
                ..
            } => {
                format!(
                    "Register {} as UBO of {} ({})",
                    ubo_person.key, cbu.key, qualifying_reason
                )
            }
            Op::UpsertDoc { key, .. } => {
                format!("Upsert {} '{}'", key.doc_type, key.key)
            }
            Op::AttachEvidence {
                cbu, evidence_type, ..
            } => {
                format!("Attach {} evidence to {}", evidence_type, cbu.key)
            }
            Op::CreateCase { cbu, case_type, .. } => {
                format!("Create {} case for {}", case_type, cbu.key)
            }
            Op::UpdateCaseStatus { case, status, .. } => {
                format!("Update case {} to {}", case.key, status)
            }
            Op::CreateWorkstream { case, entity, .. } => {
                format!("Create workstream for {} in case {}", entity.key, case.key)
            }
            Op::RunScreening {
                workstream,
                screening_type,
                ..
            } => {
                format!("Run {} screening on {}", screening_type, workstream.key)
            }
            Op::AddUniverse {
                cbu,
                instrument_class,
                market,
                ..
            } => {
                let market_str = market.as_deref().unwrap_or("*");
                format!(
                    "Add {}/{} to {} universe",
                    instrument_class, market_str, cbu.key
                )
            }
            Op::CreateSSI { cbu, name, .. } => {
                format!("Create SSI '{}' for {}", name, cbu.key)
            }
            Op::AddBookingRule { cbu, ssi, name, .. } => {
                format!("Add booking rule '{}' → {} for {}", name, ssi.key, cbu.key)
            }
            Op::Materialize {
                source, sections, ..
            } => {
                if sections.is_empty() || sections == &["all"] {
                    format!("Materialize {}", source.key)
                } else {
                    format!("Materialize {} → {:?}", source.key, sections)
                }
            }
            Op::RequireRef {
                ref_type, value, ..
            } => {
                format!("Require {} '{}'", ref_type, value)
            }
            Op::GenericCrud { verb, binding, .. } => {
                if let Some(b) = binding {
                    format!("{} :as @{}", verb, b)
                } else {
                    verb.clone()
                }
            }
        }
    }

    /// Get the binding name if this op produces one
    pub fn binding(&self) -> Option<&str> {
        match self {
            Op::EnsureEntity { binding, .. } => binding.as_deref(),
            Op::CreateCase { binding, .. } => binding.as_deref(),
            Op::CreateWorkstream { binding, .. } => binding.as_deref(),
            Op::CreateSSI { binding, .. } => binding.as_deref(),
            Op::GenericCrud { binding, .. } => binding.as_deref(),
            _ => None,
        }
    }
}

// =============================================================================
// Dependency tracking
// =============================================================================

/// Reference to what an Op produces
///
/// Used to match dependencies between Ops.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OpRef {
    /// An entity was created/ensured
    Entity(EntityKey),
    /// A document was upserted
    Doc(DocKey),
    /// A case was created
    Case(EntityKey),
    /// A workstream was created
    Workstream(EntityKey),
    /// An SSI was created
    SSI(EntityKey),
}

impl Op {
    /// Get what this Op produces (for dependency matching)
    ///
    /// Only Ops that create something other Ops can depend on return Some.
    pub fn produces(&self) -> Option<OpRef> {
        match self {
            Op::EnsureEntity { key, .. } => Some(OpRef::Entity(key.clone())),
            Op::UpsertDoc { key, .. } => Some(OpRef::Doc(key.clone())),
            Op::CreateCase { cbu, binding, .. } => {
                // Use binding name if available, otherwise CBU key
                let key = binding
                    .as_ref()
                    .map(|b| EntityKey::from_symbol(b))
                    .unwrap_or_else(|| cbu.clone());
                Some(OpRef::Case(key))
            }
            Op::CreateWorkstream {
                binding, entity, ..
            } => {
                let key = binding
                    .as_ref()
                    .map(|b| EntityKey::from_symbol(b))
                    .unwrap_or_else(|| entity.clone());
                Some(OpRef::Workstream(key))
            }
            Op::CreateSSI {
                binding, cbu, name, ..
            } => {
                let key = binding
                    .as_ref()
                    .map(|b| EntityKey::from_symbol(b))
                    .unwrap_or_else(|| EntityKey::new("ssi", format!("{}:{}", cbu.key, name)));
                Some(OpRef::SSI(key))
            }
            // GenericCrud produces an entity if it has a binding
            Op::GenericCrud { binding, verb, .. } => binding.as_ref().map(|b| {
                // Use the verb domain as entity type hint
                let entity_type = verb.split('.').next().unwrap_or("generic");
                OpRef::Entity(EntityKey::new(entity_type, b.clone()))
            }),
            _ => None,
        }
    }

    /// Get dependencies for this operation
    ///
    /// Returns OpRefs that must be satisfied before this Op can execute.
    /// If a dependency is not produced by any Op in the program, it's assumed
    /// to exist in the database (external reference).
    pub fn dependencies(&self) -> Vec<OpRef> {
        match self {
            // Entity ensure has no deps (created with null FKs)
            Op::EnsureEntity { .. } => vec![],

            // SetFK depends on both source and target existing
            Op::SetFK { source, target, .. } => {
                vec![OpRef::Entity(source.clone()), OpRef::Entity(target.clone())]
            }

            // LinkRole depends on CBU and entity
            Op::LinkRole { cbu, entity, .. } => {
                vec![OpRef::Entity(cbu.clone()), OpRef::Entity(entity.clone())]
            }

            // UnlinkRole same as LinkRole
            Op::UnlinkRole { cbu, entity, .. } => {
                vec![OpRef::Entity(cbu.clone()), OpRef::Entity(entity.clone())]
            }

            // AddOwnership depends on both entities
            Op::AddOwnership { owner, owned, .. } => {
                vec![OpRef::Entity(owner.clone()), OpRef::Entity(owned.clone())]
            }

            // RegisterUBO depends on CBU, subject, and UBO person
            Op::RegisterUBO {
                cbu,
                subject,
                ubo_person,
                ..
            } => vec![
                OpRef::Entity(cbu.clone()),
                OpRef::Entity(subject.clone()),
                OpRef::Entity(ubo_person.clone()),
            ],

            // UpsertDoc may depend on CBU if specified
            Op::UpsertDoc { cbu, .. } => cbu
                .as_ref()
                .map(|c| vec![OpRef::Entity(c.clone())])
                .unwrap_or_default(),

            // AttachEvidence depends on CBU and optionally document
            Op::AttachEvidence { cbu, document, .. } => {
                let mut deps = vec![OpRef::Entity(cbu.clone())];
                if let Some(doc) = document {
                    deps.push(OpRef::Doc(doc.clone()));
                }
                deps
            }

            // CreateCase depends on CBU
            Op::CreateCase { cbu, .. } => vec![OpRef::Entity(cbu.clone())],

            // UpdateCaseStatus depends on case existing
            Op::UpdateCaseStatus { case, .. } => vec![OpRef::Case(case.clone())],

            // CreateWorkstream depends on case and entity
            Op::CreateWorkstream { case, entity, .. } => {
                vec![OpRef::Case(case.clone()), OpRef::Entity(entity.clone())]
            }

            // RunScreening depends on workstream
            Op::RunScreening { workstream, .. } => vec![OpRef::Workstream(workstream.clone())],

            // AddUniverse depends on CBU
            Op::AddUniverse { cbu, .. } => vec![OpRef::Entity(cbu.clone())],

            // CreateSSI depends on CBU
            Op::CreateSSI { cbu, .. } => vec![OpRef::Entity(cbu.clone())],

            // AddBookingRule depends on CBU and SSI
            Op::AddBookingRule { cbu, ssi, .. } => {
                vec![OpRef::Entity(cbu.clone()), OpRef::SSI(ssi.clone())]
            }

            // Materialize depends on the document
            // NOTE: Does NOT depend on entities referenced in the doc that exist
            // in DB but not in this DSL program - those are runtime lookups
            Op::Materialize { source, .. } => vec![OpRef::Doc(source.clone())],

            // RequireRef has no runtime deps (validated at compile time)
            Op::RequireRef { .. } => vec![],

            // GenericCrud depends on entities specified in depends_on
            Op::GenericCrud { depends_on, .. } => depends_on
                .iter()
                .map(|k| OpRef::Entity(k.clone()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_key_display() {
        let key = EntityKey::cbu("Apex Fund");
        assert_eq!(key.to_string(), "cbu:Apex Fund");
    }

    #[test]
    fn test_entity_key_from_symbol() {
        let key = EntityKey::from_symbol("@fund");
        assert_eq!(key.entity_type, "symbol");
        assert_eq!(key.key, "fund");

        let key2 = EntityKey::from_symbol("fund");
        assert_eq!(key2.key, "fund");
    }

    #[test]
    fn test_op_produces() {
        let op = Op::EnsureEntity {
            entity_type: "cbu".to_string(),
            key: EntityKey::cbu("Test"),
            attrs: HashMap::new(),
            binding: Some("test".to_string()),
            source_stmt: 0,
        };
        assert!(matches!(op.produces(), Some(OpRef::Entity(_))));

        let op2 = Op::LinkRole {
            cbu: EntityKey::cbu("Test"),
            entity: EntityKey::proper_person("John"),
            role: "DIRECTOR".to_string(),
            ownership_percentage: None,
            source_stmt: 0,
        };
        assert!(op2.produces().is_none());
    }

    #[test]
    fn test_op_dependencies() {
        let op = Op::LinkRole {
            cbu: EntityKey::cbu("Fund"),
            entity: EntityKey::proper_person("John"),
            role: "DIRECTOR".to_string(),
            ownership_percentage: None,
            source_stmt: 0,
        };
        let deps = op.dependencies();
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&OpRef::Entity(EntityKey::cbu("Fund"))));
        assert!(deps.contains(&OpRef::Entity(EntityKey::proper_person("John"))));
    }

    #[test]
    fn test_ensure_entity_has_no_deps() {
        let op = Op::EnsureEntity {
            entity_type: "cbu".to_string(),
            key: EntityKey::cbu("Test"),
            attrs: HashMap::new(),
            binding: None,
            source_stmt: 0,
        };
        assert!(op.dependencies().is_empty());
    }
}
