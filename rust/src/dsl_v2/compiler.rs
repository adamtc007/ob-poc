//! Verb → Op Compiler
//!
//! Compiles AST VerbCalls to primitive Ops that can be DAG-sorted and executed.
//!
//! # Design
//!
//! Symbol resolution happens BEFORE compilation. The AST should have resolved
//! EntityRefs with resolved_key populated where possible.
//!
//! The compiler:
//! 1. Walks each VerbCall in the AST
//! 2. Extracts arguments and resolves symbol references
//! 3. Produces one or more Ops per VerbCall
//! 4. Tracks bindings (`:as @name`) in a symbol table
//!
//! # Two-Phase FK Strategy
//!
//! Entities are created with null FKs (Phase 1), then FKs are populated
//! by SetFK ops (Phase 2). This eliminates most circular dependencies.

use crate::dsl_v2::ast::{AstNode, Literal, Program, Statement, VerbCall};
use crate::dsl_v2::ops::{DocKey, EntityKey, Op};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;

/// Result of compiling a DSL program
#[derive(Debug)]
pub struct CompiledProgram {
    /// Ops in source order (not yet sorted)
    pub ops: Vec<Op>,
    /// Symbol table: binding name → EntityKey
    pub symbols: HashMap<String, EntityKey>,
    /// Errors encountered during compilation
    pub errors: Vec<CompileError>,
}

impl CompiledProgram {
    /// Check if compilation succeeded (no errors)
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Error during compilation
#[derive(Debug, Clone)]
pub struct CompileError {
    /// Source statement index
    pub stmt_idx: usize,
    /// Error message
    pub message: String,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "statement {}: {}", self.stmt_idx + 1, self.message)
    }
}

/// Compile an AST Program to Ops
pub fn compile_to_ops(program: &Program) -> CompiledProgram {
    let mut ops = Vec::new();
    let mut symbols: HashMap<String, EntityKey> = HashMap::new();
    let mut errors = Vec::new();

    for (stmt_idx, stmt) in program.statements.iter().enumerate() {
        if let Statement::VerbCall(vc) = stmt {
            match compile_verb_call(vc, stmt_idx, &symbols) {
                Ok((new_ops, binding)) => {
                    ops.extend(new_ops);
                    if let Some((name, key)) = binding {
                        symbols.insert(name, key);
                    }
                }
                Err(e) => {
                    errors.push(CompileError {
                        stmt_idx,
                        message: e,
                    });
                }
            }
        }
        // Comments are ignored
    }

    CompiledProgram {
        ops,
        symbols,
        errors,
    }
}

/// Compile a single VerbCall to Ops
///
/// Returns (ops, optional_binding) where binding is (name, EntityKey).
#[allow(clippy::type_complexity)]
fn compile_verb_call(
    vc: &VerbCall,
    stmt_idx: usize,
    symbols: &HashMap<String, EntityKey>,
) -> Result<(Vec<Op>, Option<(String, EntityKey)>), String> {
    let full_verb = format!("{}.{}", vc.domain, vc.verb);

    match full_verb.as_str() {
        // =====================================================================
        // CBU verbs
        // =====================================================================
        "cbu.ensure" | "cbu.create" => {
            let name = get_string_arg(vc, "name")?;
            let key = EntityKey::cbu(&name);

            let mut attrs = HashMap::new();
            attrs.insert("name".to_string(), serde_json::json!(name));
            if let Ok(j) = get_string_arg(vc, "jurisdiction") {
                attrs.insert("jurisdiction".to_string(), serde_json::json!(j));
            }
            if let Ok(ct) = get_string_arg(vc, "client-type") {
                attrs.insert("client_type".to_string(), serde_json::json!(ct));
            }

            let op = Op::EnsureEntity {
                entity_type: "cbu".to_string(),
                key: key.clone(),
                attrs,
                binding: vc.binding.clone(),
                source_stmt: stmt_idx,
            };

            let binding = vc.binding.as_ref().map(|b| (b.clone(), key));
            Ok((vec![op], binding))
        }

        "cbu.assign-role" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let entity = resolve_entity_arg(vc, "entity-id", symbols)?;
            let role = get_string_arg(vc, "role")?;
            let ownership = get_decimal_arg(vc, "ownership-percentage").ok();

            let op = Op::LinkRole {
                cbu,
                entity,
                role,
                ownership_percentage: ownership,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        "cbu.remove-role" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let entity = resolve_entity_arg(vc, "entity-id", symbols)?;
            let role = get_string_arg(vc, "role")?;

            let op = Op::UnlinkRole {
                cbu,
                entity,
                role,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        // =====================================================================
        // Entity verbs
        // =====================================================================
        "entity.create-proper-person" | "entity.ensure-proper-person" => {
            let first = get_string_arg(vc, "first-name")?;
            let last = get_string_arg(vc, "last-name")?;
            let name = format!("{} {}", first, last);
            let key = EntityKey::proper_person(&name);

            let mut attrs = HashMap::new();
            attrs.insert("first_name".to_string(), serde_json::json!(first));
            attrs.insert("last_name".to_string(), serde_json::json!(last));
            if let Ok(dob) = get_string_arg(vc, "date-of-birth") {
                attrs.insert("date_of_birth".to_string(), serde_json::json!(dob));
            }
            if let Ok(nat) = get_string_arg(vc, "nationality") {
                attrs.insert("nationality".to_string(), serde_json::json!(nat));
            }

            let op = Op::EnsureEntity {
                entity_type: "proper_person".to_string(),
                key: key.clone(),
                attrs,
                binding: vc.binding.clone(),
                source_stmt: stmt_idx,
            };

            let binding = vc.binding.as_ref().map(|b| (b.clone(), key));
            Ok((vec![op], binding))
        }

        "entity.create-limited-company" | "entity.ensure-limited-company" => {
            let name = get_string_arg(vc, "name")?;
            let key = EntityKey::limited_company(&name);

            let mut attrs = HashMap::new();
            attrs.insert("company_name".to_string(), serde_json::json!(name));
            if let Ok(j) = get_string_arg(vc, "jurisdiction") {
                attrs.insert("jurisdiction".to_string(), serde_json::json!(j));
            }
            if let Ok(reg) = get_string_arg(vc, "registration-number") {
                attrs.insert("registration_number".to_string(), serde_json::json!(reg));
            }

            let op = Op::EnsureEntity {
                entity_type: "limited_company".to_string(),
                key: key.clone(),
                attrs,
                binding: vc.binding.clone(),
                source_stmt: stmt_idx,
            };

            let binding = vc.binding.as_ref().map(|b| (b.clone(), key));
            Ok((vec![op], binding))
        }

        "entity.create-partnership-limited" => {
            let name = get_string_arg(vc, "name")?;
            let key = EntityKey::new("partnership", &name);

            let mut attrs = HashMap::new();
            attrs.insert("partnership_name".to_string(), serde_json::json!(name));
            if let Ok(j) = get_string_arg(vc, "jurisdiction") {
                attrs.insert("jurisdiction".to_string(), serde_json::json!(j));
            }

            let op = Op::EnsureEntity {
                entity_type: "partnership".to_string(),
                key: key.clone(),
                attrs,
                binding: vc.binding.clone(),
                source_stmt: stmt_idx,
            };

            let binding = vc.binding.as_ref().map(|b| (b.clone(), key));
            Ok((vec![op], binding))
        }

        "entity.create-trust-discretionary" => {
            let name = get_string_arg(vc, "name")?;
            let key = EntityKey::new("trust", &name);

            let mut attrs = HashMap::new();
            attrs.insert("trust_name".to_string(), serde_json::json!(name));
            if let Ok(j) = get_string_arg(vc, "jurisdiction") {
                attrs.insert("jurisdiction".to_string(), serde_json::json!(j));
            }

            let op = Op::EnsureEntity {
                entity_type: "trust".to_string(),
                key: key.clone(),
                attrs,
                binding: vc.binding.clone(),
                source_stmt: stmt_idx,
            };

            let binding = vc.binding.as_ref().map(|b| (b.clone(), key));
            Ok((vec![op], binding))
        }

        // =====================================================================
        // UBO verbs
        // =====================================================================
        "ubo.add-ownership" => {
            let owner = resolve_entity_arg(vc, "owner-entity-id", symbols)?;
            let owned = resolve_entity_arg(vc, "owned-entity-id", symbols)?;
            let pct = get_decimal_arg(vc, "percentage")?;
            let ownership_type = get_string_arg(vc, "ownership-type")?;

            let op = Op::AddOwnership {
                owner,
                owned,
                percentage: pct,
                ownership_type,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        "ubo.register-ubo" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let subject = resolve_entity_arg(vc, "subject-entity-id", symbols)?;
            let ubo_person = resolve_entity_arg(vc, "ubo-person-id", symbols)?;
            let reason = get_string_arg(vc, "qualifying-reason")?;
            let ownership = get_decimal_arg(vc, "ownership-percentage").ok();

            let op = Op::RegisterUBO {
                cbu,
                subject,
                ubo_person,
                qualifying_reason: reason,
                ownership_percentage: ownership,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        // =====================================================================
        // KYC Case verbs
        // =====================================================================
        "kyc-case.create" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let case_type =
                get_string_arg(vc, "case-type").unwrap_or_else(|_| "NEW_CLIENT".to_string());

            let key = vc
                .binding
                .as_ref()
                .map(|b| EntityKey::from_symbol(b))
                .unwrap_or_else(|| EntityKey::new("case", format!("{}:case", cbu.key)));

            let op = Op::CreateCase {
                cbu,
                case_type,
                binding: vc.binding.clone(),
                source_stmt: stmt_idx,
            };

            let binding = vc.binding.as_ref().map(|b| (b.clone(), key));
            Ok((vec![op], binding))
        }

        "kyc-case.update-status" => {
            let case = resolve_entity_arg(vc, "case-id", symbols)?;
            let status = get_string_arg(vc, "status")?;

            let op = Op::UpdateCaseStatus {
                case,
                status,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        // =====================================================================
        // Entity Workstream verbs
        // =====================================================================
        "entity-workstream.create" => {
            let case = resolve_entity_arg(vc, "case-id", symbols)?;
            let entity = resolve_entity_arg(vc, "entity-id", symbols)?;

            let key = vc
                .binding
                .as_ref()
                .map(|b| EntityKey::from_symbol(b))
                .unwrap_or_else(|| {
                    EntityKey::new("workstream", format!("{}:{}", case.key, entity.key))
                });

            let op = Op::CreateWorkstream {
                case,
                entity,
                binding: vc.binding.clone(),
                source_stmt: stmt_idx,
            };

            let binding = vc.binding.as_ref().map(|b| (b.clone(), key));
            Ok((vec![op], binding))
        }

        // =====================================================================
        // Screening verbs
        // =====================================================================
        "case-screening.run" => {
            let workstream = resolve_entity_arg(vc, "workstream-id", symbols)?;
            let screening_type = get_string_arg(vc, "screening-type")?;

            let op = Op::RunScreening {
                workstream,
                screening_type,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        "screening.pep" | "screening.sanctions" | "screening.adverse-media" => {
            // Legacy screening verbs - convert to entity-based
            let entity = resolve_entity_arg(vc, "entity-id", symbols)?;
            let screening_type = vc.verb.to_uppercase().replace('-', "_");

            // Create a synthetic workstream reference
            let workstream = EntityKey::new("workstream", format!("synthetic:{}", entity.key));

            let op = Op::RunScreening {
                workstream,
                screening_type,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        // =====================================================================
        // Custody verbs
        // =====================================================================
        "cbu-custody.add-universe" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let instrument_class = get_string_arg(vc, "instrument-class")?;
            let market = get_string_arg(vc, "market").ok();
            let currencies = get_string_list_arg(vc, "currencies").unwrap_or_default();
            let settlement_types = get_string_list_arg(vc, "settlement-types")
                .unwrap_or_else(|_| vec!["DVP".to_string()]);

            let op = Op::AddUniverse {
                cbu,
                instrument_class,
                market,
                currencies,
                settlement_types,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        "cbu-custody.create-ssi" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let name = get_string_arg(vc, "name")?;
            let ssi_type = get_string_arg(vc, "type")?;

            let key = vc
                .binding
                .as_ref()
                .map(|b| EntityKey::from_symbol(b))
                .unwrap_or_else(|| EntityKey::new("ssi", format!("{}:{}", cbu.key, name)));

            let mut attrs = HashMap::new();
            if let Ok(v) = get_string_arg(vc, "safekeeping-account") {
                attrs.insert("safekeeping_account".to_string(), serde_json::json!(v));
            }
            if let Ok(v) = get_string_arg(vc, "safekeeping-bic") {
                attrs.insert("safekeeping_bic".to_string(), serde_json::json!(v));
            }
            if let Ok(v) = get_string_arg(vc, "cash-account") {
                attrs.insert("cash_account".to_string(), serde_json::json!(v));
            }
            if let Ok(v) = get_string_arg(vc, "cash-bic") {
                attrs.insert("cash_bic".to_string(), serde_json::json!(v));
            }
            if let Ok(v) = get_string_arg(vc, "pset-bic") {
                attrs.insert("pset_bic".to_string(), serde_json::json!(v));
            }

            let op = Op::CreateSSI {
                cbu,
                name,
                ssi_type,
                attrs,
                binding: vc.binding.clone(),
                source_stmt: stmt_idx,
            };

            let binding = vc.binding.as_ref().map(|b| (b.clone(), key));
            Ok((vec![op], binding))
        }

        "cbu-custody.add-booking-rule" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let ssi = resolve_entity_arg(vc, "ssi-id", symbols)?;
            let name = get_string_arg(vc, "name")?;
            let priority = get_int_arg(vc, "priority").unwrap_or(50);

            let mut criteria = HashMap::new();
            if let Ok(v) = get_string_arg(vc, "instrument-class") {
                criteria.insert("instrument_class".to_string(), serde_json::json!(v));
            }
            if let Ok(v) = get_string_arg(vc, "market") {
                criteria.insert("market".to_string(), serde_json::json!(v));
            }
            if let Ok(v) = get_string_arg(vc, "currency") {
                criteria.insert("currency".to_string(), serde_json::json!(v));
            }
            if let Ok(v) = get_string_arg(vc, "settlement-type") {
                criteria.insert("settlement_type".to_string(), serde_json::json!(v));
            }

            let op = Op::AddBookingRule {
                cbu,
                ssi,
                name,
                priority,
                criteria,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        // =====================================================================
        // Trading Profile verbs
        // =====================================================================
        "trading-profile.import" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let key = DocKey::trading_profile(&cbu.key);

            let op = Op::UpsertDoc {
                key: key.clone(),
                content: serde_json::json!({}), // Actual content loaded at execution
                cbu: Some(cbu.clone()),
                source_stmt: stmt_idx,
            };

            // Binding uses the doc key as entity key
            let entity_key = EntityKey::new("trading_profile", &key.key);
            let binding = vc.binding.as_ref().map(|b| (b.clone(), entity_key));

            Ok((vec![op], binding))
        }

        "trading-profile.materialize" => {
            let profile = resolve_entity_arg(vc, "profile-id", symbols)?;
            let sections =
                get_string_list_arg(vc, "sections").unwrap_or_else(|_| vec!["all".to_string()]);
            let force = get_bool_arg(vc, "force").unwrap_or(false);

            let op = Op::Materialize {
                source: DocKey::trading_profile(&profile.key),
                sections,
                force,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        // =====================================================================
        // Document verbs
        // =====================================================================
        "document.catalog" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let doc_type = get_string_arg(vc, "doc-type")?;
            let title = get_string_arg(vc, "title")?;

            let key = DocKey::new(&doc_type, &title);

            let op = Op::UpsertDoc {
                key: key.clone(),
                content: serde_json::json!({
                    "doc_type": doc_type,
                    "title": title,
                }),
                cbu: Some(cbu),
                source_stmt: stmt_idx,
            };

            let entity_key = EntityKey::new("document", format!("{}:{}", key.doc_type, key.key));
            let binding = vc.binding.as_ref().map(|b| (b.clone(), entity_key));

            Ok((vec![op], binding))
        }

        // =====================================================================
        // Evidence verbs
        // =====================================================================
        "cbu.attach-evidence" => {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let evidence_type = get_string_arg(vc, "evidence-type")?;
            let doc = get_string_arg(vc, "document-id")
                .ok()
                .map(|d| DocKey::new("document", d));
            let attestation = get_string_arg(vc, "attestation-ref").ok();

            let op = Op::AttachEvidence {
                cbu,
                evidence_type,
                document: doc,
                attestation_ref: attestation,
                source_stmt: stmt_idx,
            };

            Ok((vec![op], None))
        }

        // =====================================================================
        // Unknown verb - error
        // =====================================================================
        _ => Err(format!("unknown verb for compilation: {}", full_verb)),
    }
}

// =============================================================================
// Helper functions
// =============================================================================

/// Get string argument from VerbCall
fn get_string_arg(vc: &VerbCall, key: &str) -> Result<String, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_string(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Get decimal argument from VerbCall
fn get_decimal_arg(vc: &VerbCall, key: &str) -> Result<Decimal, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_decimal(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Get integer argument from VerbCall
fn get_int_arg(vc: &VerbCall, key: &str) -> Result<i32, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_int(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Get boolean argument from VerbCall
fn get_bool_arg(vc: &VerbCall, key: &str) -> Result<bool, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_bool(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Get string list argument from VerbCall
fn get_string_list_arg(vc: &VerbCall, key: &str) -> Result<Vec<String>, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return extract_string_list(&arg.value);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Resolve an entity argument (symbol ref or literal)
fn resolve_entity_arg(
    vc: &VerbCall,
    key: &str,
    symbols: &HashMap<String, EntityKey>,
) -> Result<EntityKey, String> {
    for arg in &vc.arguments {
        if arg.key == key {
            return resolve_to_entity_key(&arg.value, symbols);
        }
    }
    Err(format!("missing required argument '{}'", key))
}

/// Resolve an AstNode to an EntityKey
fn resolve_to_entity_key(
    node: &AstNode,
    symbols: &HashMap<String, EntityKey>,
) -> Result<EntityKey, String> {
    match node {
        // Symbol reference → look up in symbol table
        AstNode::SymbolRef { name, .. } => symbols
            .get(name)
            .cloned()
            .ok_or_else(|| format!("undefined symbol '@{}'", name)),

        // String literal → create entity key from value
        AstNode::Literal(Literal::String(s)) => Ok(EntityKey::new("entity", s)),

        // EntityRef → use resolved key or value
        AstNode::EntityRef {
            entity_type,
            value,
            resolved_key,
            ..
        } => {
            let key_value = resolved_key.as_ref().unwrap_or(value);
            Ok(EntityKey::new(entity_type, key_value))
        }

        _ => Err(format!("cannot resolve {:?} to entity key", node)),
    }
}

/// Extract string from AstNode
fn extract_string(node: &AstNode) -> Result<String, String> {
    match node {
        AstNode::Literal(Literal::String(s)) => Ok(s.clone()),
        AstNode::EntityRef { value, .. } => Ok(value.clone()),
        _ => Err(format!("expected string, got {:?}", node)),
    }
}

/// Extract decimal from AstNode
fn extract_decimal(node: &AstNode) -> Result<Decimal, String> {
    match node {
        AstNode::Literal(Literal::Decimal(d)) => Ok(*d),
        AstNode::Literal(Literal::Integer(i)) => Ok(Decimal::from(*i)),
        AstNode::Literal(Literal::String(s)) => {
            Decimal::from_str(s).map_err(|e| format!("invalid decimal '{}': {}", s, e))
        }
        _ => Err(format!("expected number, got {:?}", node)),
    }
}

/// Extract integer from AstNode
fn extract_int(node: &AstNode) -> Result<i32, String> {
    match node {
        AstNode::Literal(Literal::Integer(i)) => (*i)
            .try_into()
            .map_err(|_| format!("integer {} out of i32 range", i)),
        AstNode::Literal(Literal::Decimal(d)) => {
            // Try to convert decimal to i32 if it's a whole number
            use rust_decimal::prelude::ToPrimitive;
            d.to_i32()
                .ok_or_else(|| format!("decimal {} cannot be converted to i32", d))
        }
        AstNode::Literal(Literal::String(s)) => s
            .parse::<i32>()
            .map_err(|e| format!("invalid integer '{}': {}", s, e)),
        _ => Err(format!("expected integer, got {:?}", node)),
    }
}

/// Extract boolean from AstNode
fn extract_bool(node: &AstNode) -> Result<bool, String> {
    match node {
        AstNode::Literal(Literal::Boolean(b)) => Ok(*b),
        AstNode::Literal(Literal::String(s)) => match s.to_lowercase().as_str() {
            "true" | "yes" | "1" => Ok(true),
            "false" | "no" | "0" => Ok(false),
            _ => Err(format!("invalid boolean '{}'", s)),
        },
        _ => Err(format!("expected boolean, got {:?}", node)),
    }
}

/// Extract string list from AstNode
fn extract_string_list(node: &AstNode) -> Result<Vec<String>, String> {
    match node {
        AstNode::List { items, .. } => {
            let mut result = Vec::new();
            for item in items {
                result.push(extract_string(item)?);
            }
            Ok(result)
        }
        // Single string becomes single-element list
        AstNode::Literal(Literal::String(s)) => Ok(vec![s.clone()]),
        _ => Err(format!("expected list, got {:?}", node)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::parser::parse_program;

    #[test]
    fn test_compile_cbu_ensure() {
        let source = r#"(cbu.ensure :name "Apex Fund" :jurisdiction "LU" :as @fund)"#;
        let program = parse_program(source).unwrap();
        let compiled = compile_to_ops(&program);

        assert!(compiled.is_ok());
        assert_eq!(compiled.ops.len(), 1);
        assert!(compiled.symbols.contains_key("fund"));

        if let Op::EnsureEntity {
            entity_type, key, ..
        } = &compiled.ops[0]
        {
            assert_eq!(entity_type, "cbu");
            assert_eq!(key.key, "Apex Fund");
        } else {
            panic!("expected EnsureEntity");
        }
    }

    #[test]
    fn test_compile_entity_create() {
        let source =
            r#"(entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)"#;
        let program = parse_program(source).unwrap();
        let compiled = compile_to_ops(&program);

        assert!(compiled.is_ok());
        assert_eq!(compiled.ops.len(), 1);
        assert!(compiled.symbols.contains_key("john"));

        if let Op::EnsureEntity {
            entity_type, key, ..
        } = &compiled.ops[0]
        {
            assert_eq!(entity_type, "proper_person");
            assert_eq!(key.key, "John Smith");
        } else {
            panic!("expected EnsureEntity");
        }
    }

    #[test]
    fn test_compile_assign_role_with_symbols() {
        let source = r#"
            (cbu.ensure :name "Fund" :as @fund)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
            (cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
        "#;
        let program = parse_program(source).unwrap();
        let compiled = compile_to_ops(&program);

        assert!(compiled.is_ok());
        assert_eq!(compiled.ops.len(), 3);

        // Third op should be LinkRole
        if let Op::LinkRole {
            cbu, entity, role, ..
        } = &compiled.ops[2]
        {
            assert_eq!(cbu.key, "Fund");
            assert_eq!(entity.key, "John Smith");
            assert_eq!(role, "DIRECTOR");
        } else {
            panic!("expected LinkRole");
        }
    }

    #[test]
    fn test_undefined_symbol_error() {
        let source = r#"(cbu.assign-role :cbu-id @undefined :entity-id @also_undefined :role "X")"#;
        let program = parse_program(source).unwrap();
        let compiled = compile_to_ops(&program);

        assert!(!compiled.is_ok());
        assert!(!compiled.errors.is_empty());
        assert!(compiled.errors[0].message.contains("undefined symbol"));
    }

    #[test]
    fn test_compile_multiple_ops() {
        let source = r#"
            (cbu.ensure :name "Fund A" :as @a)
            (cbu.ensure :name "Fund B" :as @b)
            (cbu.ensure :name "Fund C" :as @c)
        "#;
        let program = parse_program(source).unwrap();
        let compiled = compile_to_ops(&program);

        assert!(compiled.is_ok());
        assert_eq!(compiled.ops.len(), 3);
        assert_eq!(compiled.symbols.len(), 3);
    }
}
