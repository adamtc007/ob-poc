//! DSL Assembler - Deterministic DSL generation from resolved intents
//!
//! This module takes structured intents with resolved entity references
//! and produces valid DSL source code. NO AI involved - pure template assembly.
//!
//! The flow is:
//! 1. Receive DslIntent with ArgIntent values
//! 2. Resolve each ArgIntent via EntityGateway (lookups) or directly (literals/symbols)
//! 3. Assemble DSL string using verb registry for arg ordering/validation
//! 4. Return valid DSL or structured errors

use std::collections::HashMap;

use crate::dsl_v2::intent::{ArgIntent, DslIntent, DslIntentBatch, ResolvedArg};
use crate::dsl_v2::verb_registry::{find_unified_verb, registry};

/// Result of assembling a single DSL statement
#[derive(Debug, Clone)]
pub struct AssembledStatement {
    /// The generated DSL source
    pub dsl: String,
    /// The verb that was used
    pub verb: String,
    /// Symbol bound (if any)
    pub bind_as: Option<String>,
    /// Resolution details for debugging
    pub resolutions: HashMap<String, ResolvedArg>,
}

/// Error during assembly
#[derive(Debug, Clone)]
pub enum AssemblyError {
    /// Verb not found in registry
    UnknownVerb { verb: String },
    /// Required argument missing
    MissingRequiredArg { verb: String, arg: String },
    /// Entity lookup failed
    LookupFailed {
        arg: String,
        search: String,
        entity_type: String,
    },
    /// Could not infer verb from action/domain
    CannotInferVerb { action: String, domain: String },
    /// Invalid argument type
    InvalidArgType {
        arg: String,
        expected: String,
        got: String,
    },
}

impl std::fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssemblyError::UnknownVerb { verb } => {
                write!(f, "Unknown verb: {}", verb)
            }
            AssemblyError::MissingRequiredArg { verb, arg } => {
                write!(f, "Verb {} requires argument :{}", verb, arg)
            }
            AssemblyError::LookupFailed {
                arg,
                search,
                entity_type,
            } => {
                write!(
                    f,
                    "Lookup failed for :{} - no {} found matching '{}'",
                    arg, entity_type, search
                )
            }
            AssemblyError::CannotInferVerb { action, domain } => {
                write!(
                    f,
                    "Cannot infer verb for action '{}' in domain '{}'",
                    action, domain
                )
            }
            AssemblyError::InvalidArgType { arg, expected, got } => {
                write!(f, "Argument :{} expects {} but got {}", arg, expected, got)
            }
        }
    }
}

impl std::error::Error for AssemblyError {}

/// DSL Assembler - converts intents to DSL source
pub struct DslAssembler {
    /// Resolved arguments cache (for symbol references within a batch)
    symbols: HashMap<String, String>,
}

impl DslAssembler {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }

    /// Assemble a batch of intents into DSL source
    pub fn assemble_batch(
        &mut self,
        batch: &DslIntentBatch,
        resolver: &dyn ArgResolver,
    ) -> Result<String, Vec<AssemblyError>> {
        let mut statements = Vec::new();
        let mut errors = Vec::new();

        for intent in &batch.actions {
            match self.assemble_one(intent, resolver) {
                Ok(assembled) => {
                    // Track symbol binding for later references
                    if let Some(ref symbol) = assembled.bind_as {
                        self.symbols.insert(symbol.clone(), symbol.clone());
                    }
                    statements.push(assembled.dsl);
                }
                Err(e) => errors.push(e),
            }
        }

        if errors.is_empty() {
            Ok(statements.join("\n"))
        } else {
            Err(errors)
        }
    }

    /// Assemble a single intent into a DSL statement
    pub fn assemble_one(
        &self,
        intent: &DslIntent,
        resolver: &dyn ArgResolver,
    ) -> Result<AssembledStatement, AssemblyError> {
        // Determine the verb
        let verb_name = self.resolve_verb(intent)?;

        // Get verb definition from registry
        let parts: Vec<&str> = verb_name.split('.').collect();
        if parts.len() != 2 {
            return Err(AssemblyError::UnknownVerb { verb: verb_name });
        }

        let verb_def =
            find_unified_verb(parts[0], parts[1]).ok_or_else(|| AssemblyError::UnknownVerb {
                verb: verb_name.clone(),
            })?;

        // Check required args are present
        for required_arg in verb_def.required_arg_names() {
            if !intent.args.contains_key(required_arg) {
                return Err(AssemblyError::MissingRequiredArg {
                    verb: verb_name.clone(),
                    arg: required_arg.to_string(),
                });
            }
        }

        // Resolve all arguments
        let mut resolutions = HashMap::new();
        let mut resolved_args = Vec::new();

        // Process args in registry order for consistent output
        for arg_def in &verb_def.args {
            if let Some(arg_intent) = intent.args.get(&arg_def.name) {
                let resolved = self.resolve_arg(&arg_def.name, arg_intent, resolver, intent)?;
                resolved_args.push((arg_def.name.clone(), resolved.clone()));
                resolutions.insert(arg_def.name.clone(), resolved);
            }
        }

        // Build DSL string
        let mut dsl = format!("({}", verb_name);

        for (name, resolved) in &resolved_args {
            let value_str = if resolved.is_symbol_ref {
                format!("@{}", resolved.value)
            } else if resolved.needs_quotes {
                format!("\"{}\"", resolved.value)
            } else {
                resolved.value.clone()
            };
            dsl.push_str(&format!(" :{} {}", name, value_str));
        }

        // Add :as binding if present
        if let Some(ref bind_as) = intent.bind_as {
            dsl.push_str(&format!(" :as @{}", bind_as));
        }

        dsl.push(')');

        Ok(AssembledStatement {
            dsl,
            verb: verb_name,
            bind_as: intent.bind_as.clone(),
            resolutions,
        })
    }

    /// Resolve verb from intent (explicit or inferred)
    fn resolve_verb(&self, intent: &DslIntent) -> Result<String, AssemblyError> {
        if let Some(ref verb) = intent.verb {
            return Ok(verb.clone());
        }

        // Try to infer verb from action + domain
        let inferred = self.infer_verb(&intent.action, &intent.domain);
        inferred.ok_or_else(|| AssemblyError::CannotInferVerb {
            action: intent.action.clone(),
            domain: intent.domain.clone(),
        })
    }

    /// Infer verb from action and domain
    fn infer_verb(&self, action: &str, domain: &str) -> Option<String> {
        let reg = registry();
        let domain_verbs = reg.verbs_for_domain(domain);

        // Common action → verb mappings
        let verb_name = match action.to_lowercase().as_str() {
            "create" | "add" | "new" => {
                // Try ensure first (idempotent), then create
                domain_verbs
                    .iter()
                    .find(|v| v.verb == "ensure")
                    .or_else(|| domain_verbs.iter().find(|v| v.verb == "create"))
                    .map(|v| v.verb.clone())
            }
            "assign" | "link" => domain_verbs
                .iter()
                .find(|v| v.verb.contains("assign") || v.verb.contains("role"))
                .map(|v| v.verb.clone()),
            "remove" | "delete" | "unlink" => domain_verbs
                .iter()
                .find(|v| v.verb.contains("remove") || v.verb == "delete")
                .map(|v| v.verb.clone()),
            "update" | "modify" | "change" => domain_verbs
                .iter()
                .find(|v| v.verb == "update")
                .map(|v| v.verb.clone()),
            "list" | "get" | "read" | "show" => domain_verbs
                .iter()
                .find(|v| v.verb == "list" || v.verb == "read")
                .map(|v| v.verb.clone()),
            _ => None,
        }?;

        Some(format!("{}.{}", domain, verb_name))
    }

    /// Resolve a single argument
    fn resolve_arg(
        &self,
        arg_name: &str,
        arg_intent: &ArgIntent,
        resolver: &dyn ArgResolver,
        intent: &DslIntent,
    ) -> Result<ResolvedArg, AssemblyError> {
        match arg_intent {
            ArgIntent::Literal { value } => {
                let (value_str, needs_quotes) = match value {
                    serde_json::Value::String(s) => (s.clone(), true),
                    serde_json::Value::Number(n) => (n.to_string(), false),
                    serde_json::Value::Bool(b) => (b.to_string(), false),
                    serde_json::Value::Null => ("nil".to_string(), false),
                    _ => (value.to_string(), true),
                };
                Ok(ResolvedArg {
                    value: value_str,
                    is_symbol_ref: false,
                    needs_quotes,
                    display: None,
                })
            }

            ArgIntent::SymbolRef { symbol } => {
                // Check if symbol is defined (in current batch or externally)
                // For now, trust it - validation will catch undefined symbols
                Ok(ResolvedArg {
                    value: symbol.clone(),
                    is_symbol_ref: true,
                    needs_quotes: false,
                    display: Some(format!("@{}", symbol)),
                })
            }

            ArgIntent::EntityLookup {
                search_text,
                entity_type,
            } => {
                // Use the verb's LookupConfig to determine entity_type if not specified
                let effective_type = entity_type.clone().unwrap_or_else(|| {
                    self.get_entity_type_from_verb(intent, arg_name)
                        .unwrap_or_else(|| "entity".to_string())
                });

                resolver
                    .resolve_entity(search_text, &effective_type)
                    .map_err(|_| AssemblyError::LookupFailed {
                        arg: arg_name.to_string(),
                        search: search_text.clone(),
                        entity_type: effective_type,
                    })
            }

            ArgIntent::RefDataLookup {
                search_text,
                ref_type,
            } => resolver
                .resolve_ref_data(search_text, ref_type)
                .map_err(|_| AssemblyError::LookupFailed {
                    arg: arg_name.to_string(),
                    search: search_text.clone(),
                    entity_type: ref_type.clone(),
                }),
        }
    }

    /// Get entity_type from verb's LookupConfig for an argument
    fn get_entity_type_from_verb(&self, intent: &DslIntent, arg_name: &str) -> Option<String> {
        let verb_name = intent.verb.as_ref()?;
        let parts: Vec<&str> = verb_name.split('.').collect();
        if parts.len() != 2 {
            return None;
        }

        let verb_def = find_unified_verb(parts[0], parts[1])?;

        for arg in &verb_def.args {
            if arg.name == arg_name {
                if let Some(ref lookup) = arg.lookup {
                    return lookup.entity_type.clone();
                }
            }
        }
        None
    }
}

impl Default for DslAssembler {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for resolving entity and reference data lookups
/// This abstracts the EntityGateway so we can test without network calls
pub trait ArgResolver: Send + Sync {
    /// Resolve an entity by search text and type
    fn resolve_entity(&self, search: &str, entity_type: &str) -> Result<ResolvedArg, String>;

    /// Resolve reference data (role, jurisdiction, etc.)
    fn resolve_ref_data(&self, search: &str, ref_type: &str) -> Result<ResolvedArg, String>;
}

/// Mock resolver for testing
#[cfg(test)]
pub struct MockResolver {
    entities: HashMap<(String, String), ResolvedArg>,
    ref_data: HashMap<(String, String), ResolvedArg>,
}

#[cfg(test)]
impl MockResolver {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
            ref_data: HashMap::new(),
        }
    }

    pub fn add_entity(mut self, search: &str, entity_type: &str, id: &str, display: &str) -> Self {
        self.entities.insert(
            (search.to_lowercase(), entity_type.to_lowercase()),
            ResolvedArg {
                value: id.to_string(),
                is_symbol_ref: false,
                needs_quotes: false,
                display: Some(display.to_string()),
            },
        );
        self
    }

    pub fn add_ref(mut self, search: &str, ref_type: &str, code: &str) -> Self {
        self.ref_data.insert(
            (search.to_lowercase(), ref_type.to_lowercase()),
            ResolvedArg {
                value: code.to_string(),
                is_symbol_ref: false,
                needs_quotes: false,
                display: Some(code.to_string()),
            },
        );
        self
    }
}

#[cfg(test)]
impl ArgResolver for MockResolver {
    fn resolve_entity(&self, search: &str, entity_type: &str) -> Result<ResolvedArg, String> {
        self.entities
            .get(&(search.to_lowercase(), entity_type.to_lowercase()))
            .cloned()
            .ok_or_else(|| format!("No {} found for '{}'", entity_type, search))
    }

    fn resolve_ref_data(&self, search: &str, ref_type: &str) -> Result<ResolvedArg, String> {
        self.ref_data
            .get(&(search.to_lowercase(), ref_type.to_lowercase()))
            .cloned()
            .ok_or_else(|| format!("No {} found for '{}'", ref_type, search))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_resolver() -> MockResolver {
        MockResolver::new()
            .add_entity("apex fund", "cbu", "uuid-apex", "Apex Fund")
            .add_entity("john smith", "entity", "uuid-john", "John Smith (PERSON)")
            .add_ref("director", "role", "DIRECTOR")
            .add_ref("lu", "jurisdiction", "LU")
    }

    #[test]
    fn test_assemble_simple_literal() {
        let resolver = setup_resolver();
        let assembler = DslAssembler::new();

        let intent = DslIntent {
            verb: Some("cbu.ensure".to_string()),
            action: "create".to_string(),
            domain: "cbu".to_string(),
            args: HashMap::from([
                DslIntent::literal("name", "Test Fund"),
                DslIntent::literal("jurisdiction", "LU"),
            ]),
            bind_as: Some("fund".to_string()),
            source_text: None,
        };

        let result = assembler.assemble_one(&intent, &resolver).unwrap();
        assert!(result.dsl.contains("cbu.ensure"));
        assert!(result.dsl.contains(":name \"Test Fund\""));
        assert!(result.dsl.contains(":as @fund"));
    }

    #[test]
    fn test_assemble_with_symbol_ref() {
        let resolver = setup_resolver();
        let assembler = DslAssembler::new();

        let intent = DslIntent {
            verb: Some("cbu.assign-role".to_string()),
            action: "assign".to_string(),
            domain: "cbu".to_string(),
            args: HashMap::from([
                DslIntent::symbol_ref("cbu-id", "fund"),
                DslIntent::symbol_ref("entity-id", "john"),
                DslIntent::ref_lookup("role", "director", "role"),
            ]),
            bind_as: None,
            source_text: None,
        };

        let result = assembler.assemble_one(&intent, &resolver).unwrap();
        assert!(result.dsl.contains(":cbu-id @fund"));
        assert!(result.dsl.contains(":entity-id @john"));
        assert!(result.dsl.contains(":role DIRECTOR"));
    }

    #[test]
    fn test_assemble_batch() {
        let resolver = setup_resolver();
        let mut assembler = DslAssembler::new();

        let batch = DslIntentBatch::new("Create fund and add director")
            .add_action(DslIntent {
                verb: Some("cbu.ensure".to_string()),
                action: "create".to_string(),
                domain: "cbu".to_string(),
                args: HashMap::from([
                    DslIntent::literal("name", "Test Fund"),
                    DslIntent::literal("jurisdiction", "LU"),
                ]),
                bind_as: Some("fund".to_string()),
                source_text: None,
            })
            .add_action(DslIntent {
                verb: Some("cbu.assign-role".to_string()),
                action: "assign".to_string(),
                domain: "cbu".to_string(),
                args: HashMap::from([
                    DslIntent::symbol_ref("cbu-id", "fund"),
                    DslIntent::ref_lookup("role", "director", "role"),
                    DslIntent::entity_lookup("entity-id", "john smith", Some("entity")),
                ]),
                bind_as: None,
                source_text: None,
            });

        let result = assembler.assemble_batch(&batch, &resolver).unwrap();
        assert!(result.contains("cbu.ensure"));
        assert!(result.contains("cbu.assign-role"));
        assert!(result.contains(":cbu-id @fund")); // Symbol ref from first statement
    }

    #[test]
    fn test_missing_required_arg() {
        let resolver = setup_resolver();
        let assembler = DslAssembler::new();

        let intent = DslIntent {
            verb: Some("cbu.ensure".to_string()),
            action: "create".to_string(),
            domain: "cbu".to_string(),
            args: HashMap::new(), // Missing required :name
            bind_as: None,
            source_text: None,
        };

        let result = assembler.assemble_one(&intent, &resolver);
        assert!(result.is_err());
        if let Err(AssemblyError::MissingRequiredArg { verb, arg }) = result {
            assert_eq!(verb, "cbu.ensure");
            assert_eq!(arg, "name");
        }
    }
}
