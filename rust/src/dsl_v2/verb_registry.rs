//! Unified Verb Registry
//!
//! Single source of truth for all verbs in the DSL system.
//! Combines CRUD verbs from `verbs.rs` with custom operations.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    UnifiedVerbRegistry                       │
//! ├─────────────────────────────────────────────────────────────┤
//! │  Sources:                                                    │
//! │  ├── CRUD verbs (from verbs.rs STANDARD_VERBS)              │
//! │  └── Custom ops (defined in this module)                    │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::OnceLock;

use super::verbs::{Behavior, VerbDef, STANDARD_VERBS};

// =============================================================================
// TYPES
// =============================================================================

/// How a verb is executed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbBehavior {
    /// Standard CRUD operation (generic executor)
    Crud,
    /// Custom operation with specialized handler
    CustomOp,
    /// Composite operation (expands to multiple steps)
    Composite,
}

/// Argument definition for unified verbs
#[derive(Debug, Clone)]
pub struct ArgDef {
    pub name: &'static str,
    pub arg_type: &'static str,
    pub required: bool,
    pub description: &'static str,
}

/// Unified verb definition combining CRUD and custom ops
#[derive(Debug, Clone)]
pub struct UnifiedVerbDef {
    pub domain: &'static str,
    pub verb: &'static str,
    pub description: &'static str,
    pub args: Vec<ArgDef>,
    pub behavior: VerbBehavior,
    /// For custom ops, the handler ID
    pub custom_op_id: Option<&'static str>,
    /// Original CRUD verb def (if applicable)
    pub crud_def: Option<&'static VerbDef>,
    /// Original CRUD behavior (for executor dispatch)
    pub crud_behavior: Option<Behavior>,
}

impl UnifiedVerbDef {
    /// Full verb name: "domain.verb"
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.domain, self.verb)
    }

    /// Check if verb accepts a given argument key
    pub fn accepts_arg(&self, key: &str) -> bool {
        self.args.iter().any(|a| a.name == key)
    }

    /// Get required arguments
    pub fn required_args(&self) -> Vec<&ArgDef> {
        self.args.iter().filter(|a| a.required).collect()
    }

    /// Get required argument names (for compatibility)
    pub fn required_arg_names(&self) -> Vec<&'static str> {
        self.args
            .iter()
            .filter(|a| a.required)
            .map(|a| a.name)
            .collect()
    }

    /// Get optional argument names (for compatibility)
    pub fn optional_arg_names(&self) -> Vec<&'static str> {
        self.args
            .iter()
            .filter(|a| !a.required)
            .map(|a| a.name)
            .collect()
    }
}

// =============================================================================
// REGISTRY
// =============================================================================

/// The unified verb registry - singleton
static UNIFIED_REGISTRY: OnceLock<UnifiedVerbRegistry> = OnceLock::new();

pub struct UnifiedVerbRegistry {
    /// All verbs indexed by "domain.verb"
    verbs: HashMap<String, UnifiedVerbDef>,
    /// Verbs grouped by domain
    by_domain: HashMap<String, Vec<String>>,
    /// All domain names (sorted)
    domains: Vec<String>,
}

impl UnifiedVerbRegistry {
    /// Get the global registry instance
    pub fn global() -> &'static UnifiedVerbRegistry {
        UNIFIED_REGISTRY.get_or_init(Self::build)
    }

    /// Build the registry from all sources
    fn build() -> Self {
        let mut verbs = HashMap::new();
        let mut by_domain: HashMap<String, Vec<String>> = HashMap::new();

        // 1. Load CRUD verbs from verbs.rs
        for crud_verb in STANDARD_VERBS.iter() {
            let key = format!("{}.{}", crud_verb.domain, crud_verb.verb);

            // Convert required/optional args to ArgDef
            let mut args = Vec::new();
            for &arg_name in crud_verb.required_args {
                args.push(ArgDef {
                    name: arg_name,
                    arg_type: infer_arg_type(arg_name),
                    required: true,
                    description: "",
                });
            }
            for &arg_name in crud_verb.optional_args {
                args.push(ArgDef {
                    name: arg_name,
                    arg_type: infer_arg_type(arg_name),
                    required: false,
                    description: "",
                });
            }

            let unified = UnifiedVerbDef {
                domain: crud_verb.domain,
                verb: crud_verb.verb,
                description: crud_verb.description,
                args,
                behavior: VerbBehavior::Crud,
                custom_op_id: None,
                crud_def: Some(crud_verb),
                crud_behavior: Some(crud_verb.behavior.clone()),
            };
            verbs.insert(key.clone(), unified);
            by_domain
                .entry(crud_verb.domain.to_string())
                .or_default()
                .push(key);
        }

        // 2. Load custom ops (may override CRUD verbs)
        for custom_op in custom_ops_definitions() {
            let key = format!("{}.{}", custom_op.domain, custom_op.verb);
            let unified = UnifiedVerbDef {
                domain: custom_op.domain,
                verb: custom_op.verb,
                description: custom_op.description,
                args: custom_op.args,
                behavior: VerbBehavior::CustomOp,
                custom_op_id: Some(custom_op.op_id),
                crud_def: None,
                crud_behavior: None,
            };
            // Custom ops override CRUD if same name
            verbs.insert(key.clone(), unified);
            // Only add to domain list if not already present
            let domain_list = by_domain.entry(custom_op.domain.to_string()).or_default();
            if !domain_list.contains(&key) {
                domain_list.push(key);
            }
        }

        // Sort domain verb lists
        for list in by_domain.values_mut() {
            list.sort();
            list.dedup();
        }

        let mut domains: Vec<String> = by_domain.keys().cloned().collect();
        domains.sort();

        Self {
            verbs,
            by_domain,
            domains,
        }
    }

    /// Look up a verb by domain and verb name
    pub fn get(&self, domain: &str, verb: &str) -> Option<&UnifiedVerbDef> {
        let key = format!("{}.{}", domain, verb);
        self.verbs.get(&key)
    }

    /// Look up by full name "domain.verb"
    pub fn get_by_name(&self, full_name: &str) -> Option<&UnifiedVerbDef> {
        self.verbs.get(full_name)
    }

    /// Get all verbs for a domain
    pub fn verbs_for_domain(&self, domain: &str) -> Vec<&UnifiedVerbDef> {
        self.by_domain
            .get(domain)
            .map(|keys| keys.iter().filter_map(|k| self.verbs.get(k)).collect())
            .unwrap_or_default()
    }

    /// Get all domain names
    pub fn domains(&self) -> &[String] {
        &self.domains
    }

    /// Get all verbs
    pub fn all_verbs(&self) -> impl Iterator<Item = &UnifiedVerbDef> {
        self.verbs.values()
    }

    /// Total verb count
    pub fn len(&self) -> usize {
        self.verbs.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.verbs.is_empty()
    }

    /// Check if a verb exists
    pub fn contains(&self, domain: &str, verb: &str) -> bool {
        self.get(domain, verb).is_some()
    }
}

// =============================================================================
// CUSTOM OPS DEFINITIONS
// =============================================================================

/// Static definition for a custom operation
#[derive(Debug)]
pub struct CustomOpStaticDef {
    pub domain: &'static str,
    pub verb: &'static str,
    pub op_id: &'static str,
    pub description: &'static str,
    pub args: Vec<ArgDef>,
}

/// Get all custom operation definitions
/// This bridges the gap between custom_ops/mod.rs and the registry
fn custom_ops_definitions() -> Vec<CustomOpStaticDef> {
    vec![
        // =====================================================================
        // Document operations
        // =====================================================================
        CustomOpStaticDef {
            domain: "document",
            verb: "catalog",
            op_id: "document.catalog",
            description: "Catalog a document for an entity within a CBU",
            args: vec![
                ArgDef {
                    name: "cbu-id",
                    arg_type: "ref:cbu",
                    required: true,
                    description: "CBU reference",
                },
                ArgDef {
                    name: "entity-id",
                    arg_type: "ref:entity",
                    required: true,
                    description: "Entity reference",
                },
                ArgDef {
                    name: "document-type",
                    arg_type: "ref:document_type",
                    required: true,
                    description: "Document type code",
                },
                ArgDef {
                    name: "file-path",
                    arg_type: "string",
                    required: false,
                    description: "Path to document file",
                },
                ArgDef {
                    name: "metadata",
                    arg_type: "map",
                    required: false,
                    description: "Additional metadata",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "document",
            verb: "extract",
            op_id: "document.extract",
            description: "Extract attributes from a cataloged document",
            args: vec![
                ArgDef {
                    name: "document-id",
                    arg_type: "ref:document",
                    required: true,
                    description: "Document reference",
                },
                ArgDef {
                    name: "attributes",
                    arg_type: "list:string",
                    required: false,
                    description: "Specific attributes to extract",
                },
                ArgDef {
                    name: "use-ocr",
                    arg_type: "boolean",
                    required: false,
                    description: "Enable OCR extraction",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "document",
            verb: "request",
            op_id: "document.request",
            description: "Request a document from client",
            args: vec![
                ArgDef {
                    name: "cbu-id",
                    arg_type: "ref:cbu",
                    required: true,
                    description: "CBU reference",
                },
                ArgDef {
                    name: "entity-id",
                    arg_type: "ref:entity",
                    required: true,
                    description: "Entity reference",
                },
                ArgDef {
                    name: "document-type",
                    arg_type: "ref:document_type",
                    required: true,
                    description: "Document type code",
                },
                ArgDef {
                    name: "due-date",
                    arg_type: "date",
                    required: false,
                    description: "Request due date",
                },
                ArgDef {
                    name: "priority",
                    arg_type: "string",
                    required: false,
                    description: "Priority level",
                },
            ],
        },
        // =====================================================================
        // UBO operations
        // =====================================================================
        CustomOpStaticDef {
            domain: "ubo",
            verb: "calculate",
            op_id: "ubo.calculate",
            description: "Calculate ultimate beneficial ownership chain",
            args: vec![
                ArgDef {
                    name: "cbu-id",
                    arg_type: "ref:cbu",
                    required: true,
                    description: "CBU reference",
                },
                ArgDef {
                    name: "entity-id",
                    arg_type: "ref:entity",
                    required: true,
                    description: "Entity to analyze",
                },
                ArgDef {
                    name: "threshold",
                    arg_type: "number",
                    required: false,
                    description: "Ownership threshold (default 25%)",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "ubo",
            verb: "validate",
            op_id: "ubo.validate",
            description: "Validate UBO structure completeness",
            args: vec![ArgDef {
                name: "cbu-id",
                arg_type: "ref:cbu",
                required: true,
                description: "CBU reference",
            }],
        },
        // =====================================================================
        // Screening operations
        // =====================================================================
        CustomOpStaticDef {
            domain: "screening",
            verb: "pep",
            op_id: "screening.pep",
            description: "Run PEP (Politically Exposed Person) screening",
            args: vec![
                ArgDef {
                    name: "entity-id",
                    arg_type: "ref:entity",
                    required: true,
                    description: "Entity to screen",
                },
                ArgDef {
                    name: "provider",
                    arg_type: "string",
                    required: false,
                    description: "Screening provider",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "screening",
            verb: "sanctions",
            op_id: "screening.sanctions",
            description: "Run sanctions list screening",
            args: vec![
                ArgDef {
                    name: "entity-id",
                    arg_type: "ref:entity",
                    required: true,
                    description: "Entity to screen",
                },
                ArgDef {
                    name: "lists",
                    arg_type: "list:string",
                    required: false,
                    description: "Specific sanction lists",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "screening",
            verb: "adverse-media",
            op_id: "screening.adverse_media",
            description: "Run adverse media screening",
            args: vec![
                ArgDef {
                    name: "entity-id",
                    arg_type: "ref:entity",
                    required: true,
                    description: "Entity to screen",
                },
                ArgDef {
                    name: "lookback-months",
                    arg_type: "number",
                    required: false,
                    description: "Months to search back",
                },
            ],
        },
        // =====================================================================
        // KYC operations
        // =====================================================================
        CustomOpStaticDef {
            domain: "kyc",
            verb: "initiate",
            op_id: "kyc.initiate",
            description: "Initiate KYC investigation",
            args: vec![
                ArgDef {
                    name: "cbu-id",
                    arg_type: "ref:cbu",
                    required: true,
                    description: "CBU reference",
                },
                ArgDef {
                    name: "investigation-type",
                    arg_type: "string",
                    required: true,
                    description: "Type of investigation",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "kyc",
            verb: "decide",
            op_id: "kyc.decide",
            description: "Record KYC decision",
            args: vec![
                ArgDef {
                    name: "investigation-id",
                    arg_type: "ref:investigation",
                    required: true,
                    description: "Investigation reference",
                },
                ArgDef {
                    name: "decision",
                    arg_type: "string",
                    required: true,
                    description: "Decision: approve, reject, escalate",
                },
                ArgDef {
                    name: "rationale",
                    arg_type: "string",
                    required: true,
                    description: "Decision rationale",
                },
            ],
        },
        // =====================================================================
        // Resource Instance operations
        // =====================================================================
        CustomOpStaticDef {
            domain: "resource",
            verb: "create",
            op_id: "resource.create",
            description: "Create a resource instance for a CBU",
            args: vec![
                ArgDef {
                    name: "cbu-id",
                    arg_type: "ref:cbu",
                    required: true,
                    description: "CBU reference",
                },
                ArgDef {
                    name: "resource-type",
                    arg_type: "string",
                    required: true,
                    description: "Resource type code (e.g., DTCC_SETTLE)",
                },
                ArgDef {
                    name: "instance-url",
                    arg_type: "string",
                    required: true,
                    description: "Unique URL/endpoint for this instance",
                },
                ArgDef {
                    name: "instance-id",
                    arg_type: "string",
                    required: false,
                    description: "Instance identifier (account #, user ID)",
                },
                ArgDef {
                    name: "instance-name",
                    arg_type: "string",
                    required: false,
                    description: "Human-readable instance name",
                },
                ArgDef {
                    name: "product-id",
                    arg_type: "ref:product",
                    required: false,
                    description: "Product reference",
                },
                ArgDef {
                    name: "service-id",
                    arg_type: "ref:service",
                    required: false,
                    description: "Service reference",
                },
                ArgDef {
                    name: "config",
                    arg_type: "map",
                    required: false,
                    description: "Instance configuration JSON",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "resource",
            verb: "set-attr",
            op_id: "resource.set_attr",
            description: "Set an attribute value on a resource instance",
            args: vec![
                ArgDef {
                    name: "instance-id",
                    arg_type: "ref:instance",
                    required: true,
                    description: "Resource instance reference",
                },
                ArgDef {
                    name: "attr",
                    arg_type: "string",
                    required: true,
                    description: "Attribute name from dictionary",
                },
                ArgDef {
                    name: "value",
                    arg_type: "string",
                    required: true,
                    description: "Attribute value",
                },
                ArgDef {
                    name: "state",
                    arg_type: "string",
                    required: false,
                    description: "Value state: proposed, confirmed, derived, system",
                },
                ArgDef {
                    name: "source",
                    arg_type: "map",
                    required: false,
                    description: "Value provenance metadata",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "resource",
            verb: "activate",
            op_id: "resource.activate",
            description: "Activate a resource instance (PENDING -> ACTIVE)",
            args: vec![ArgDef {
                name: "instance-id",
                arg_type: "ref:instance",
                required: true,
                description: "Resource instance reference",
            }],
        },
        CustomOpStaticDef {
            domain: "resource",
            verb: "suspend",
            op_id: "resource.suspend",
            description: "Suspend a resource instance",
            args: vec![
                ArgDef {
                    name: "instance-id",
                    arg_type: "ref:instance",
                    required: true,
                    description: "Resource instance reference",
                },
                ArgDef {
                    name: "reason",
                    arg_type: "string",
                    required: false,
                    description: "Suspension reason",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "resource",
            verb: "decommission",
            op_id: "resource.decommission",
            description: "Decommission a resource instance",
            args: vec![
                ArgDef {
                    name: "instance-id",
                    arg_type: "ref:instance",
                    required: true,
                    description: "Resource instance reference",
                },
                ArgDef {
                    name: "reason",
                    arg_type: "string",
                    required: false,
                    description: "Decommission reason",
                },
            ],
        },
        // =====================================================================
        // Service Delivery operations
        // =====================================================================
        CustomOpStaticDef {
            domain: "delivery",
            verb: "record",
            op_id: "delivery.record",
            description: "Record a service delivery for a CBU",
            args: vec![
                ArgDef {
                    name: "cbu-id",
                    arg_type: "ref:cbu",
                    required: true,
                    description: "CBU reference",
                },
                ArgDef {
                    name: "product",
                    arg_type: "string",
                    required: true,
                    description: "Product code",
                },
                ArgDef {
                    name: "service",
                    arg_type: "string",
                    required: true,
                    description: "Service code",
                },
                ArgDef {
                    name: "instance-id",
                    arg_type: "ref:instance",
                    required: false,
                    description: "Resource instance reference",
                },
                ArgDef {
                    name: "config",
                    arg_type: "map",
                    required: false,
                    description: "Service configuration options",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "delivery",
            verb: "complete",
            op_id: "delivery.complete",
            description: "Mark a service delivery as complete",
            args: vec![
                ArgDef {
                    name: "cbu-id",
                    arg_type: "ref:cbu",
                    required: true,
                    description: "CBU reference",
                },
                ArgDef {
                    name: "product",
                    arg_type: "string",
                    required: true,
                    description: "Product code",
                },
                ArgDef {
                    name: "service",
                    arg_type: "string",
                    required: true,
                    description: "Service code",
                },
                ArgDef {
                    name: "instance-id",
                    arg_type: "ref:instance",
                    required: false,
                    description: "Resource instance reference (optional update)",
                },
            ],
        },
        CustomOpStaticDef {
            domain: "delivery",
            verb: "fail",
            op_id: "delivery.fail",
            description: "Mark a service delivery as failed",
            args: vec![
                ArgDef {
                    name: "cbu-id",
                    arg_type: "ref:cbu",
                    required: true,
                    description: "CBU reference",
                },
                ArgDef {
                    name: "product",
                    arg_type: "string",
                    required: true,
                    description: "Product code",
                },
                ArgDef {
                    name: "service",
                    arg_type: "string",
                    required: true,
                    description: "Service code",
                },
                ArgDef {
                    name: "reason",
                    arg_type: "string",
                    required: true,
                    description: "Failure reason",
                },
            ],
        },
    ]
}

// =============================================================================
// HELPERS
// =============================================================================

/// Infer argument type from argument name (for CRUD verbs that don't specify types)
fn infer_arg_type(arg_name: &str) -> &'static str {
    if arg_name.ends_with("-id") {
        // It's a reference type
        let domain = arg_name.trim_end_matches("-id");
        match domain {
            "cbu" => "ref:cbu",
            "entity" => "ref:entity",
            "document" => "ref:document",
            "investigation" => "ref:investigation",
            "decision" => "ref:decision",
            "product" => "ref:product",
            "service" => "ref:service",
            "screening" => "ref:screening",
            "flag" => "ref:flag",
            "condition" => "ref:condition",
            "review" => "ref:review",
            "rating" => "ref:rating",
            "config" => "ref:config",
            "event" => "ref:event",
            "link" => "ref:link",
            "result" => "ref:result",
            "instance" => "ref:instance",
            "delivery" => "ref:delivery",
            _ => "uuid",
        }
    } else {
        match arg_name {
            // String types
            "name"
            | "first-name"
            | "last-name"
            | "middle-names"
            | "description"
            | "rationale"
            | "notes"
            | "status"
            | "decision"
            | "outcome"
            | "resolution"
            | "role"
            | "provider"
            | "result"
            | "title"
            | "condition-type"
            | "flag-type"
            | "event-type"
            | "review-type"
            | "monitoring-level"
            | "investigation-type"
            | "client-type"
            | "entity-type"
            | "trust-type"
            | "partnership-type"
            | "product-code"
            | "service-code"
            | "product-category"
            | "service-category"
            | "company-number"
            | "id-document-type"
            | "id-document-number"
            | "nationality"
            | "jurisdiction"
            | "doc-type"
            | "relationship-type"
            | "verification-status"
            | "severity"
            | "rating"
            | "nature-purpose"
            | "governing-law"
            | "trust-purpose"
            | "principal-place-business"
            | "business-nature"
            | "regulatory-framework"
            | "components"
            | "frequency"
            | "assigned-to"
            | "decided-by"
            | "completed-by"
            | "satisfied-by"
            | "flagged-by"
            | "assessed-by"
            | "reviewed-by"
            | "registered-address"
            | "residence-address" => "string",

            // Date types
            "date-of-birth" | "incorporation-date" | "formation-date" | "establishment-date"
            | "due-date" | "deadline" | "next-review-date" | "satisfied-date" => "date",

            // Boolean types
            "is-active" | "is-required" | "requires-review" => "boolean",

            // Number types
            "limit" | "offset" | "threshold" | "ubo-threshold" => "number",

            // Complex types
            "factors" | "evidence" | "match-details" | "metadata" => "map",

            // Default to string
            _ => "string",
        }
    }
}

// =============================================================================
// CONVENIENCE FUNCTIONS
// =============================================================================

/// Get the global registry
pub fn registry() -> &'static UnifiedVerbRegistry {
    UnifiedVerbRegistry::global()
}

/// Look up a verb (convenience function)
pub fn find_unified_verb(domain: &str, verb: &str) -> Option<&'static UnifiedVerbDef> {
    registry().get(domain, verb)
}

/// Check if verb exists (convenience function)
pub fn verb_exists(domain: &str, verb: &str) -> bool {
    registry().contains(domain, verb)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_loads() {
        let reg = UnifiedVerbRegistry::global();
        assert!(reg.len() > 0, "Registry should have verbs");
        println!("Registry has {} verbs", reg.len());
    }

    #[test]
    fn test_crud_verb_exists() {
        let reg = registry();
        let verb = reg.get("cbu", "create");
        assert!(verb.is_some(), "cbu.create should exist");
        assert_eq!(verb.unwrap().behavior, VerbBehavior::Crud);
    }

    #[test]
    fn test_custom_op_exists() {
        let reg = registry();
        let verb = reg.get("document", "catalog");
        assert!(verb.is_some(), "document.catalog should exist");
        assert_eq!(verb.unwrap().behavior, VerbBehavior::CustomOp);
    }

    #[test]
    fn test_custom_op_has_args() {
        let reg = registry();
        let catalog = reg.get("document", "catalog").unwrap();

        assert!(catalog.accepts_arg("cbu-id"));
        assert!(catalog.accepts_arg("entity-id"));
        assert!(catalog.accepts_arg("document-type"));
        assert!(!catalog.accepts_arg("nonexistent-arg"));

        let required = catalog.required_arg_names();
        assert!(required.contains(&"cbu-id"));
        assert!(required.contains(&"entity-id"));
        assert!(required.contains(&"document-type"));
    }

    #[test]
    fn test_domains_list() {
        let reg = registry();
        let domains = reg.domains();
        assert!(domains.contains(&"cbu".to_string()));
        assert!(domains.contains(&"entity".to_string()));
        assert!(domains.contains(&"document".to_string()));
        assert!(domains.contains(&"ubo".to_string())); // From custom ops
        assert!(domains.contains(&"kyc".to_string())); // From custom ops
    }

    #[test]
    fn test_verbs_for_domain() {
        let reg = registry();
        let doc_verbs = reg.verbs_for_domain("document");

        // Should have both CRUD and custom ops
        let verb_names: Vec<_> = doc_verbs.iter().map(|v| v.verb).collect();
        assert!(
            verb_names.contains(&"catalog"),
            "Should have document.catalog (custom op)"
        );
        assert!(
            verb_names.contains(&"extract"),
            "Should have document.extract (custom op)"
        );
        assert!(
            verb_names.contains(&"read"),
            "Should have document.read (CRUD)"
        );
        assert!(
            verb_names.contains(&"update"),
            "Should have document.update (CRUD)"
        );
    }

    #[test]
    fn test_screening_custom_ops() {
        let reg = registry();

        // screening.pep and screening.sanctions should be custom ops
        let pep = reg.get("screening", "pep");
        assert!(pep.is_some(), "screening.pep should exist");
        assert_eq!(pep.unwrap().behavior, VerbBehavior::CustomOp);

        let sanctions = reg.get("screening", "sanctions");
        assert!(sanctions.is_some(), "screening.sanctions should exist");
        assert_eq!(sanctions.unwrap().behavior, VerbBehavior::CustomOp);
    }

    #[test]
    fn test_ubo_domain() {
        let reg = registry();

        // ubo is a custom-ops-only domain
        let ubo_verbs = reg.verbs_for_domain("ubo");
        assert!(!ubo_verbs.is_empty(), "ubo domain should have verbs");

        for verb in ubo_verbs {
            assert_eq!(
                verb.behavior,
                VerbBehavior::CustomOp,
                "All ubo verbs should be custom ops"
            );
        }
    }

    #[test]
    fn test_full_name() {
        let reg = registry();
        let verb = reg.get("document", "catalog").unwrap();
        assert_eq!(verb.full_name(), "document.catalog");
    }

    #[test]
    fn test_verb_count() {
        let reg = registry();
        // Should have CRUD verbs + custom ops
        // STANDARD_VERBS has ~53 verbs, custom ops adds ~10 more (some overlap with document)
        assert!(
            reg.len() >= 50,
            "Should have at least 50 verbs, got {}",
            reg.len()
        );
    }

    #[test]
    fn test_product_ensure_exists() {
        let reg = registry();
        let verb = reg.get("product", "ensure");
        assert!(verb.is_some(), "product.ensure should exist in registry");
        let v = verb.unwrap();
        assert_eq!(v.behavior, VerbBehavior::Crud);
        assert!(v.crud_behavior.is_some(), "Should have crud_behavior");
        // The crud_behavior should be Upsert
        if let Some(ref b) = v.crud_behavior {
            match b {
                Behavior::Upsert {
                    table,
                    conflict_keys,
                } => {
                    assert_eq!(*table, "products");
                    assert!(conflict_keys.contains(&"product-code"));
                }
                _ => panic!("Expected Upsert behavior, got {:?}", b),
            }
        }
    }

    #[test]
    fn test_infer_arg_type() {
        assert_eq!(infer_arg_type("cbu-id"), "ref:cbu");
        assert_eq!(infer_arg_type("entity-id"), "ref:entity");
        assert_eq!(infer_arg_type("name"), "string");
        assert_eq!(infer_arg_type("date-of-birth"), "date");
        assert_eq!(infer_arg_type("is-active"), "boolean");
        assert_eq!(infer_arg_type("limit"), "number");
    }
}
