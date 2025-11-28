//! Standard verb definitions (Tier 1)
//!
//! This module contains the static definitions for all data-driven verbs.
//! These verbs are executed by the generic CRUD functions in the executor.
//!
//! IMPORTANT: If a verb cannot be expressed using the Behavior enum,
//! it belongs in custom_ops/, not here.

use crate::dsl_v2::executor::ReturnType;

/// Behavior patterns for data-driven execution
///
/// Each variant maps to a generic executor function.
/// If your operation doesn't fit these patterns, use CustomOperation instead.
#[derive(Debug, Clone)]
pub enum Behavior {
    /// INSERT into a single table
    /// Generates: INSERT INTO table (cols...) VALUES (vals...) RETURNING pk
    Insert { table: &'static str },

    /// SELECT from a single table
    /// Generates: SELECT * FROM table WHERE conditions...
    Select { table: &'static str },

    /// UPDATE a single table
    /// Generates: UPDATE table SET cols... WHERE pk = $1
    Update { table: &'static str },

    /// DELETE from a single table
    /// Generates: DELETE FROM table WHERE pk = $1
    Delete { table: &'static str },

    /// INSERT with ON CONFLICT DO UPDATE (upsert)
    /// Generates: INSERT ... ON CONFLICT (keys) DO UPDATE SET ...
    Upsert {
        table: &'static str,
        conflict_keys: &'static [&'static str],
    },

    /// INSERT into junction table (taxonomy link)
    /// For linking entities with roles: CBU ↔ Entity, Document ↔ Entity, etc.
    Link {
        junction: &'static str,
        from_col: &'static str,
        to_col: &'static str,
        role_col: Option<&'static str>,
    },

    /// DELETE from junction table (taxonomy unlink)
    Unlink {
        junction: &'static str,
        from_col: &'static str,
        to_col: &'static str,
    },

    /// SELECT filtered by foreign key
    /// Generates: SELECT * FROM table WHERE fk_col = $1
    ListByFk {
        table: &'static str,
        fk_col: &'static str,
    },

    /// SELECT with JOIN for related data
    SelectWithJoin {
        primary_table: &'static str,
        join_table: &'static str,
        join_col: &'static str,
    },
}

/// Complete verb definition
#[derive(Debug, Clone)]
pub struct VerbDef {
    /// Domain name (e.g., "entity", "cbu", "document")
    pub domain: &'static str,

    /// Verb name (e.g., "create-limited-company", "link")
    pub verb: &'static str,

    /// Execution behavior pattern
    pub behavior: Behavior,

    /// Required arguments (error if missing)
    pub required_args: &'static [&'static str],

    /// Optional arguments
    pub optional_args: &'static [&'static str],

    /// Return type specification
    pub returns: ReturnType,

    /// Human-readable description (for docs/help)
    pub description: &'static str,
}

/// All standard verbs - THE source of truth for Tier 1
///
/// To add a new verb:
/// 1. First, try to express it using existing Behavior variants
/// 2. If you need a new Behavior variant, add it and implement in executor
/// 3. If it truly can't be data-driven, add to custom_ops/ instead
pub static STANDARD_VERBS: &[VerbDef] = &[
    // =========================================================================
    // ENTITY DOMAIN - Singleton CRUD for different entity types
    // =========================================================================
    VerbDef {
        domain: "entity",
        verb: "create-limited-company",
        behavior: Behavior::Insert {
            table: "limited_companies",
        },
        required_args: &["name"],
        optional_args: &[
            "jurisdiction",
            "company-number",
            "incorporation-date",
            "registered-address",
            "business-nature",
        ],
        returns: ReturnType::Uuid {
            name: "entity_id",
            capture: true,
        },
        description: "Create a new limited company entity",
    },
    VerbDef {
        domain: "entity",
        verb: "create-proper-person",
        behavior: Behavior::Insert {
            table: "proper_persons",
        },
        required_args: &["first-name", "last-name"],
        optional_args: &[
            "middle-names",
            "date-of-birth",
            "nationality",
            "tax-id",
            "residence-address",
        ],
        returns: ReturnType::Uuid {
            name: "entity_id",
            capture: true,
        },
        description: "Create a new natural person entity",
    },
    VerbDef {
        domain: "entity",
        verb: "create-partnership",
        behavior: Behavior::Insert {
            table: "partnerships",
        },
        required_args: &["name"],
        optional_args: &[
            "jurisdiction",
            "partnership-type",
            "formation-date",
            "principal-place-business",
        ],
        returns: ReturnType::Uuid {
            name: "entity_id",
            capture: true,
        },
        description: "Create a new partnership entity (LP, LLP, GP)",
    },
    VerbDef {
        domain: "entity",
        verb: "create-trust",
        behavior: Behavior::Insert { table: "trusts" },
        required_args: &["name", "jurisdiction"],
        optional_args: &[
            "trust-type",
            "establishment-date",
            "governing-law",
            "trust-purpose",
        ],
        returns: ReturnType::Uuid {
            name: "entity_id",
            capture: true,
        },
        description: "Create a new trust entity",
    },
    VerbDef {
        domain: "entity",
        verb: "read",
        behavior: Behavior::Select { table: "entities" },
        required_args: &["entity-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read an entity by ID",
    },
    VerbDef {
        domain: "entity",
        verb: "update",
        behavior: Behavior::Update { table: "entities" },
        required_args: &["entity-id"],
        optional_args: &["name", "status", "jurisdiction"],
        returns: ReturnType::Affected,
        description: "Update an entity's base fields",
    },
    VerbDef {
        domain: "entity",
        verb: "delete",
        behavior: Behavior::Delete { table: "entities" },
        required_args: &["entity-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Delete an entity (cascades to type extension)",
    },
    VerbDef {
        domain: "entity",
        verb: "list",
        behavior: Behavior::Select { table: "entities" },
        required_args: &[],
        optional_args: &["entity-type", "jurisdiction", "status", "limit", "offset"],
        returns: ReturnType::RecordSet,
        description: "List entities with optional filters",
    },
    // Upsert variants for idempotent operations
    VerbDef {
        domain: "entity",
        verb: "ensure-limited-company",
        behavior: Behavior::Upsert {
            table: "limited_companies",
            conflict_keys: &["company-number", "jurisdiction"],
        },
        required_args: &["name"],
        optional_args: &["jurisdiction", "company-number", "incorporation-date"],
        returns: ReturnType::Uuid {
            name: "entity_id",
            capture: true,
        },
        description:
            "Create or update a limited company by natural key (company-number + jurisdiction)",
    },
    VerbDef {
        domain: "entity",
        verb: "ensure-proper-person",
        behavior: Behavior::Upsert {
            table: "proper_persons",
            conflict_keys: &["tax-id"],
        },
        required_args: &["first-name", "last-name"],
        optional_args: &["nationality", "tax-id", "date-of-birth"],
        returns: ReturnType::Uuid {
            name: "entity_id",
            capture: true,
        },
        description: "Create or update a proper person by tax ID",
    },
    // =========================================================================
    // CBU DOMAIN - Singleton + Taxonomy operations
    // =========================================================================
    VerbDef {
        domain: "cbu",
        verb: "create",
        behavior: Behavior::Insert { table: "cbus" },
        required_args: &["name"],
        optional_args: &[
            "jurisdiction",
            "client-type",
            "nature-purpose",
            "description",
        ],
        returns: ReturnType::Uuid {
            name: "cbu_id",
            capture: true,
        },
        description: "Create a new Client Business Unit",
    },
    VerbDef {
        domain: "cbu",
        verb: "read",
        behavior: Behavior::Select { table: "cbus" },
        required_args: &["cbu-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read a CBU by ID",
    },
    VerbDef {
        domain: "cbu",
        verb: "update",
        behavior: Behavior::Update { table: "cbus" },
        required_args: &["cbu-id"],
        optional_args: &["name", "status", "client-type", "jurisdiction"],
        returns: ReturnType::Affected,
        description: "Update a CBU",
    },
    VerbDef {
        domain: "cbu",
        verb: "delete",
        behavior: Behavior::Delete { table: "cbus" },
        required_args: &["cbu-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Delete a CBU",
    },
    VerbDef {
        domain: "cbu",
        verb: "list",
        behavior: Behavior::Select { table: "cbus" },
        required_args: &[],
        optional_args: &["status", "client-type", "jurisdiction", "limit", "offset"],
        returns: ReturnType::RecordSet,
        description: "List CBUs with optional filters",
    },
    VerbDef {
        domain: "cbu",
        verb: "ensure",
        behavior: Behavior::Upsert {
            table: "cbus",
            conflict_keys: &["name", "jurisdiction"],
        },
        required_args: &["name"],
        optional_args: &["jurisdiction", "client-type", "nature-purpose"],
        returns: ReturnType::Uuid {
            name: "cbu_id",
            capture: true,
        },
        description: "Create or update a CBU by natural key (name + jurisdiction)",
    },
    // Taxonomy: Link entities to CBU with roles
    VerbDef {
        domain: "cbu",
        verb: "link",
        behavior: Behavior::Link {
            junction: "cbu_entity_roles",
            from_col: "cbu_id",
            to_col: "entity_id",
            role_col: Some("role"),
        },
        required_args: &["cbu-id", "entity-id", "role"],
        optional_args: &["ownership-percent", "effective-date", "notes"],
        returns: ReturnType::Uuid {
            name: "cbu_entity_role_id",
            capture: false,
        },
        description: "Link an entity to a CBU with a role",
    },
    VerbDef {
        domain: "cbu",
        verb: "unlink",
        behavior: Behavior::Unlink {
            junction: "cbu_entity_roles",
            from_col: "cbu_id",
            to_col: "entity_id",
        },
        required_args: &["cbu-id", "entity-id"],
        optional_args: &["role"],
        returns: ReturnType::Affected,
        description: "Unlink an entity from a CBU (optionally by specific role)",
    },
    VerbDef {
        domain: "cbu",
        verb: "entities",
        behavior: Behavior::ListByFk {
            table: "cbu_entity_roles",
            fk_col: "cbu_id",
        },
        required_args: &["cbu-id"],
        optional_args: &["role"],
        returns: ReturnType::RecordSet,
        description: "List all entities linked to a CBU",
    },
    // =========================================================================
    // DOCUMENT DOMAIN - Note: catalog and extract are CUSTOM
    // =========================================================================
    VerbDef {
        domain: "document",
        verb: "read",
        behavior: Behavior::Select {
            table: "document_catalog",
        },
        required_args: &["document-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read a document by ID",
    },
    VerbDef {
        domain: "document",
        verb: "update",
        behavior: Behavior::Update {
            table: "document_catalog",
        },
        required_args: &["document-id"],
        optional_args: &["title", "status", "verification-status"],
        returns: ReturnType::Affected,
        description: "Update document metadata",
    },
    VerbDef {
        domain: "document",
        verb: "delete",
        behavior: Behavior::Delete {
            table: "document_catalog",
        },
        required_args: &["document-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Delete a document",
    },
    VerbDef {
        domain: "document",
        verb: "link-cbu",
        behavior: Behavior::Link {
            junction: "document_cbu_links",
            from_col: "document_id",
            to_col: "cbu_id",
            role_col: Some("relationship_type"),
        },
        required_args: &["document-id", "cbu-id"],
        optional_args: &["relationship-type"],
        returns: ReturnType::Uuid {
            name: "link_id",
            capture: false,
        },
        description: "Link a document to a CBU",
    },
    VerbDef {
        domain: "document",
        verb: "link-entity",
        behavior: Behavior::Link {
            junction: "document_entity_links",
            from_col: "document_id",
            to_col: "entity_id",
            role_col: Some("relationship_type"),
        },
        required_args: &["document-id", "entity-id"],
        optional_args: &["relationship-type"],
        returns: ReturnType::Uuid {
            name: "link_id",
            capture: false,
        },
        description: "Link a document to an entity",
    },
    VerbDef {
        domain: "document",
        verb: "unlink-cbu",
        behavior: Behavior::Unlink {
            junction: "document_cbu_links",
            from_col: "document_id",
            to_col: "cbu_id",
        },
        required_args: &["document-id", "cbu-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Unlink a document from a CBU",
    },
    VerbDef {
        domain: "document",
        verb: "unlink-entity",
        behavior: Behavior::Unlink {
            junction: "document_entity_links",
            from_col: "document_id",
            to_col: "entity_id",
        },
        required_args: &["document-id", "entity-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Unlink a document from an entity",
    },
    VerbDef {
        domain: "document",
        verb: "list-by-cbu",
        behavior: Behavior::ListByFk {
            table: "document_catalog",
            fk_col: "cbu_id",
        },
        required_args: &["cbu-id"],
        optional_args: &["doc-type", "status"],
        returns: ReturnType::RecordSet,
        description: "List documents for a CBU",
    },
    VerbDef {
        domain: "document",
        verb: "list-by-entity",
        behavior: Behavior::SelectWithJoin {
            primary_table: "document_catalog",
            join_table: "document_entity_links",
            join_col: "document_id",
        },
        required_args: &["entity-id"],
        optional_args: &["doc-type", "relationship-type"],
        returns: ReturnType::RecordSet,
        description: "List documents linked to an entity",
    },
    // =========================================================================
    // PRODUCT DOMAIN
    // =========================================================================
    VerbDef {
        domain: "product",
        verb: "create",
        behavior: Behavior::Insert { table: "products" },
        required_args: &["name", "product-code"],
        optional_args: &["description", "product-category", "regulatory-framework"],
        returns: ReturnType::Uuid {
            name: "product_id",
            capture: true,
        },
        description: "Create a new product",
    },
    VerbDef {
        domain: "product",
        verb: "read",
        behavior: Behavior::Select { table: "products" },
        required_args: &[],
        optional_args: &["product-id", "product-code"],
        returns: ReturnType::Record,
        description: "Read a product by ID or code",
    },
    VerbDef {
        domain: "product",
        verb: "update",
        behavior: Behavior::Update { table: "products" },
        required_args: &["product-id"],
        optional_args: &["name", "description", "is-active"],
        returns: ReturnType::Affected,
        description: "Update a product",
    },
    VerbDef {
        domain: "product",
        verb: "delete",
        behavior: Behavior::Delete { table: "products" },
        required_args: &["product-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Delete a product",
    },
    VerbDef {
        domain: "product",
        verb: "list",
        behavior: Behavior::Select { table: "products" },
        required_args: &[],
        optional_args: &["product-category", "is-active", "limit", "offset"],
        returns: ReturnType::RecordSet,
        description: "List products with optional filters",
    },
    // =========================================================================
    // SERVICE DOMAIN
    // =========================================================================
    VerbDef {
        domain: "service",
        verb: "create",
        behavior: Behavior::Insert { table: "services" },
        required_args: &["name", "service-code"],
        optional_args: &["description", "service-category"],
        returns: ReturnType::Uuid {
            name: "service_id",
            capture: true,
        },
        description: "Create a new service",
    },
    VerbDef {
        domain: "service",
        verb: "read",
        behavior: Behavior::Select { table: "services" },
        required_args: &[],
        optional_args: &["service-id", "service-code"],
        returns: ReturnType::Record,
        description: "Read a service by ID or code",
    },
    VerbDef {
        domain: "service",
        verb: "link-product",
        behavior: Behavior::Link {
            junction: "product_services",
            from_col: "service_id",
            to_col: "product_id",
            role_col: None,
        },
        required_args: &["service-id", "product-id"],
        optional_args: &["is-required"],
        returns: ReturnType::Uuid {
            name: "link_id",
            capture: false,
        },
        description: "Link a service to a product",
    },
    VerbDef {
        domain: "service",
        verb: "unlink-product",
        behavior: Behavior::Unlink {
            junction: "product_services",
            from_col: "service_id",
            to_col: "product_id",
        },
        required_args: &["service-id", "product-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Unlink a service from a product",
    },
    // =========================================================================
    // INVESTIGATION DOMAIN
    // =========================================================================
    VerbDef {
        domain: "investigation",
        verb: "create",
        behavior: Behavior::Insert {
            table: "investigations",
        },
        required_args: &["cbu-id", "investigation-type"],
        optional_args: &["risk-rating", "ubo-threshold", "deadline", "assigned-to"],
        returns: ReturnType::Uuid {
            name: "investigation_id",
            capture: true,
        },
        description: "Create a KYC investigation",
    },
    VerbDef {
        domain: "investigation",
        verb: "read",
        behavior: Behavior::Select {
            table: "investigations",
        },
        required_args: &["investigation-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read an investigation by ID",
    },
    VerbDef {
        domain: "investigation",
        verb: "update-status",
        behavior: Behavior::Update {
            table: "investigations",
        },
        required_args: &["investigation-id", "status"],
        optional_args: &["notes"],
        returns: ReturnType::Affected,
        description: "Update investigation status",
    },
    VerbDef {
        domain: "investigation",
        verb: "complete",
        behavior: Behavior::Update {
            table: "investigations",
        },
        required_args: &["investigation-id", "outcome"],
        optional_args: &["notes", "completed-by"],
        returns: ReturnType::Affected,
        description: "Complete an investigation with outcome",
    },
    VerbDef {
        domain: "investigation",
        verb: "list-by-cbu",
        behavior: Behavior::ListByFk {
            table: "investigations",
            fk_col: "cbu_id",
        },
        required_args: &["cbu-id"],
        optional_args: &["status", "investigation-type"],
        returns: ReturnType::RecordSet,
        description: "List investigations for a CBU",
    },
    // =========================================================================
    // SCREENING DOMAIN - Note: pep and sanctions are CUSTOM (external APIs)
    // =========================================================================
    VerbDef {
        domain: "screening",
        verb: "record-result",
        behavior: Behavior::Insert {
            table: "screening_results",
        },
        required_args: &["screening-id", "result"],
        optional_args: &["match-details", "reviewed-by"],
        returns: ReturnType::Uuid {
            name: "result_id",
            capture: false,
        },
        description: "Record a screening result",
    },
    VerbDef {
        domain: "screening",
        verb: "resolve",
        behavior: Behavior::Update {
            table: "screening_results",
        },
        required_args: &["screening-id", "resolution"],
        optional_args: &["rationale", "resolved-by"],
        returns: ReturnType::Affected,
        description: "Resolve a screening match",
    },
    // =========================================================================
    // RISK DOMAIN
    // =========================================================================
    VerbDef {
        domain: "risk",
        verb: "set-rating",
        behavior: Behavior::Upsert {
            table: "risk_ratings",
            conflict_keys: &["cbu-id"],
        },
        required_args: &["rating"],
        optional_args: &["cbu-id", "entity-id", "factors", "rationale", "assessed-by"],
        returns: ReturnType::Uuid {
            name: "rating_id",
            capture: false,
        },
        description: "Set risk rating for CBU or entity (upserts by cbu-id)",
    },
    VerbDef {
        domain: "risk",
        verb: "add-flag",
        behavior: Behavior::Insert {
            table: "risk_flags",
        },
        required_args: &["flag-type", "description"],
        optional_args: &["cbu-id", "entity-id", "flagged-by", "severity"],
        returns: ReturnType::Uuid {
            name: "flag_id",
            capture: false,
        },
        description: "Add a risk flag",
    },
    VerbDef {
        domain: "risk",
        verb: "remove-flag",
        behavior: Behavior::Delete {
            table: "risk_flags",
        },
        required_args: &["flag-id"],
        optional_args: &[],
        returns: ReturnType::Affected,
        description: "Remove a risk flag",
    },
    VerbDef {
        domain: "risk",
        verb: "list-flags",
        behavior: Behavior::ListByFk {
            table: "risk_flags",
            fk_col: "cbu_id",
        },
        required_args: &["cbu-id"],
        optional_args: &["flag-type", "severity"],
        returns: ReturnType::RecordSet,
        description: "List risk flags for a CBU",
    },
    // =========================================================================
    // DECISION DOMAIN
    // =========================================================================
    VerbDef {
        domain: "decision",
        verb: "record",
        behavior: Behavior::Insert { table: "decisions" },
        required_args: &["cbu-id", "decision"],
        optional_args: &[
            "investigation-id",
            "decision-authority",
            "rationale",
            "decided-by",
        ],
        returns: ReturnType::Uuid {
            name: "decision_id",
            capture: true,
        },
        description: "Record an onboarding decision",
    },
    VerbDef {
        domain: "decision",
        verb: "read",
        behavior: Behavior::Select { table: "decisions" },
        required_args: &["decision-id"],
        optional_args: &[],
        returns: ReturnType::Record,
        description: "Read a decision by ID",
    },
    VerbDef {
        domain: "decision",
        verb: "add-condition",
        behavior: Behavior::Insert {
            table: "decision_conditions",
        },
        required_args: &["decision-id", "condition-type"],
        optional_args: &["description", "frequency", "due-date", "threshold"],
        returns: ReturnType::Uuid {
            name: "condition_id",
            capture: false,
        },
        description: "Add a condition to a decision",
    },
    VerbDef {
        domain: "decision",
        verb: "satisfy-condition",
        behavior: Behavior::Update {
            table: "decision_conditions",
        },
        required_args: &["condition-id"],
        optional_args: &["satisfied-by", "evidence", "satisfied-date"],
        returns: ReturnType::Affected,
        description: "Mark a condition as satisfied",
    },
    // =========================================================================
    // MONITORING DOMAIN
    // =========================================================================
    VerbDef {
        domain: "monitoring",
        verb: "setup",
        behavior: Behavior::Insert {
            table: "monitoring_configs",
        },
        required_args: &["cbu-id", "monitoring-level"],
        optional_args: &["components", "frequency"],
        returns: ReturnType::Uuid {
            name: "config_id",
            capture: false,
        },
        description: "Setup ongoing monitoring for a CBU",
    },
    VerbDef {
        domain: "monitoring",
        verb: "record-event",
        behavior: Behavior::Insert {
            table: "monitoring_events",
        },
        required_args: &["cbu-id", "event-type"],
        optional_args: &["description", "severity", "requires-review"],
        returns: ReturnType::Uuid {
            name: "event_id",
            capture: false,
        },
        description: "Record a monitoring event",
    },
    VerbDef {
        domain: "monitoring",
        verb: "schedule-review",
        behavior: Behavior::Insert {
            table: "scheduled_reviews",
        },
        required_args: &["cbu-id", "review-type", "due-date"],
        optional_args: &["assigned-to"],
        returns: ReturnType::Uuid {
            name: "review_id",
            capture: false,
        },
        description: "Schedule a periodic review",
    },
    VerbDef {
        domain: "monitoring",
        verb: "complete-review",
        behavior: Behavior::Update {
            table: "scheduled_reviews",
        },
        required_args: &["review-id"],
        optional_args: &["completed-by", "notes", "next-review-date"],
        returns: ReturnType::Affected,
        description: "Complete a scheduled review",
    },
];

// ============================================================================
// Lookup Functions
// ============================================================================

/// Find a verb definition by domain and verb name
pub fn find_verb(domain: &str, verb: &str) -> Option<&'static VerbDef> {
    STANDARD_VERBS
        .iter()
        .find(|v| v.domain == domain && v.verb == verb)
}

/// Get all verbs for a specific domain
pub fn verbs_for_domain(domain: &str) -> Vec<&'static VerbDef> {
    STANDARD_VERBS
        .iter()
        .filter(|v| v.domain == domain)
        .collect()
}

/// Get all unique domain names
pub fn domains() -> Vec<&'static str> {
    let mut seen = std::collections::HashSet::new();
    STANDARD_VERBS
        .iter()
        .filter_map(|v| {
            if seen.insert(v.domain) {
                Some(v.domain)
            } else {
                None
            }
        })
        .collect()
}

/// Count of standard verbs (for metrics)
pub fn verb_count() -> usize {
    STANDARD_VERBS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_verb() {
        let verb = find_verb("entity", "create-limited-company");
        assert!(verb.is_some());
        let v = verb.unwrap();
        assert_eq!(v.domain, "entity");
        assert_eq!(v.verb, "create-limited-company");
    }

    #[test]
    fn test_find_verb_not_found() {
        assert!(find_verb("nonexistent", "verb").is_none());
    }

    #[test]
    fn test_verbs_for_domain() {
        let entity_verbs = verbs_for_domain("entity");
        assert!(entity_verbs.len() > 5); // We have multiple entity verbs
        assert!(entity_verbs.iter().all(|v| v.domain == "entity"));
    }

    #[test]
    fn test_domains() {
        let all_domains = domains();
        assert!(all_domains.contains(&"entity"));
        assert!(all_domains.contains(&"cbu"));
        assert!(all_domains.contains(&"document"));
    }

    #[test]
    fn test_all_verbs_have_required_fields() {
        for verb in STANDARD_VERBS {
            assert!(!verb.domain.is_empty(), "Verb has empty domain");
            assert!(!verb.verb.is_empty(), "Verb has empty verb name");
            assert!(
                !verb.description.is_empty(),
                "Verb {} has empty description",
                verb.verb
            );
        }
    }

    #[test]
    fn test_no_duplicate_verbs() {
        let mut seen = std::collections::HashSet::new();
        for verb in STANDARD_VERBS {
            let key = (verb.domain, verb.verb);
            assert!(
                seen.insert(key),
                "Duplicate verb: {}.{}",
                verb.domain,
                verb.verb
            );
        }
    }
}
