//! ob-poc verb compiler — consumer-side `VerbHandler` for ob-poc DSL programs.
//!
//! This crate depends only on `dsl-core` + `serde_json`, making it safe for
//! **any** consumer to pull in without dragging ob-poc internals:
//!
//! - `dsl-lsp` — LSP diagnostics for ob-poc DSL files
//! - `ob-agentic` — LLM-powered DSL generation
//! - `dsl_v2` — ob-poc binary's execution layer
//! - Future: `bpmn-lite` integration can supply an analogous crate
//!
//! # Adding new ob-poc verbs
//!
//! Add an arm to `ob_poc_verb_handler`. Return `Some(Ok(...))` on success,
//! `Some(Err(msg))` on argument error, `None` to fall through.

use std::collections::HashMap;

use dsl_core::ast::{Program, VerbCall};
use dsl_core::compiler::{
    compile_to_ops_ext, get_bool_arg, get_decimal_arg, get_int_arg, get_string_arg,
    get_string_list_arg, resolve_entity_arg, CompiledProgram, VerbHandler,
};
use dsl_core::ops::{DocKey, EntityKey, Op};

/// Compile using the ob-poc verb handler.
pub fn compile_to_ops(program: &Program) -> CompiledProgram {
    compile_to_ops_ext(program, Some(ob_poc_verb_handler))
}

/// ob-poc verb handler — implements `VerbHandler` for all ob-poc domain verbs.
///
/// Returns `Some(Ok(...))` for a handled verb, `Some(Err(...))` for a
/// recognized-but-invalid call, `None` for verbs not in ob-poc vocabulary.
///
/// Each arm uses an IIFE so that `?` propagates to `Result<_, String>` inside
/// the closure, not to the outer `Option<Result<...>>` return.
pub fn ob_poc_verb_handler(
    vc: &VerbCall,
    stmt_idx: usize,
    symbols: &HashMap<String, EntityKey>,
) -> Option<Result<(Vec<Op>, Option<(String, EntityKey)>), String>> {
    type R = Result<(Vec<Op>, Option<(String, EntityKey)>), String>;

    let full_verb = format!("{}.{}", vc.domain, vc.verb);
    match full_verb.as_str() {
        // =================================================================
        // CBU verbs
        // =================================================================
        "cbu.ensure" | "cbu.create" => Some((|| -> R {
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
        })()),

        "cbu.assign-role" => Some((|| -> R {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let entity = resolve_entity_arg(vc, "entity-id", symbols)?;
            let role = get_string_arg(vc, "role")?;
            let ownership = get_decimal_arg(vc, "ownership-percentage").ok();
            Ok((
                vec![Op::LinkRole {
                    cbu,
                    entity,
                    role,
                    ownership_percentage: ownership,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        "cbu.remove-role" => Some((|| -> R {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let entity = resolve_entity_arg(vc, "entity-id", symbols)?;
            let role = get_string_arg(vc, "role")?;
            Ok((
                vec![Op::UnlinkRole {
                    cbu,
                    entity,
                    role,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        // =================================================================
        // Entity verbs
        // =================================================================
        "entity.create" | "entity.ensure" => Some((|| -> R {
            let entity_type = get_string_arg(vc, "entity-type")?;
            let normalized = entity_type.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "proper-person" | "proper_person" | "natural-person" | "natural_person" => {
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
                "limited-company" | "limited_company" => {
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
                "partnership" => {
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
                "trust" => {
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
                _ => Err(format!(
                    "unsupported entity-type for compilation: {}",
                    entity_type
                )),
            }
        })()),

        "entity.create-proper-person" | "entity.ensure-proper-person" => Some((|| -> R {
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
        })()),

        "entity.create-limited-company" | "entity.ensure-limited-company" => Some((|| -> R {
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
        })()),

        "entity.create-partnership-limited" => Some((|| -> R {
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
        })()),

        "entity.create-trust-discretionary" => Some((|| -> R {
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
        })()),

        // =================================================================
        // UBO verbs
        // =================================================================
        "ubo.add-ownership" => Some((|| -> R {
            let owner = resolve_entity_arg(vc, "owner-entity-id", symbols)?;
            let owned = resolve_entity_arg(vc, "owned-entity-id", symbols)?;
            let pct = get_decimal_arg(vc, "percentage")?;
            let ownership_type = get_string_arg(vc, "ownership-type")?;
            Ok((
                vec![Op::AddOwnership {
                    owner,
                    owned,
                    percentage: pct,
                    ownership_type,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        "ubo.register-ubo" => Some((|| -> R {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let subject = resolve_entity_arg(vc, "subject-entity-id", symbols)?;
            let ubo_person = resolve_entity_arg(vc, "ubo-person-id", symbols)?;
            let reason = get_string_arg(vc, "qualifying-reason")?;
            let ownership = get_decimal_arg(vc, "ownership-percentage").ok();
            Ok((
                vec![Op::RegisterUBO {
                    cbu,
                    subject,
                    ubo_person,
                    qualifying_reason: reason,
                    ownership_percentage: ownership,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        // =================================================================
        // KYC Case verbs
        // =================================================================
        "kyc-case.create" => Some((|| -> R {
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
        })()),

        "kyc-case.update-status" => Some((|| -> R {
            let case = resolve_entity_arg(vc, "case-id", symbols)?;
            let status = get_string_arg(vc, "status")?;
            Ok((
                vec![Op::UpdateCaseStatus {
                    case,
                    status,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        // =================================================================
        // Entity Workstream verbs
        // =================================================================
        "entity-workstream.create" => Some((|| -> R {
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
        })()),

        // =================================================================
        // Screening verbs
        // =================================================================
        "case-screening.run" => Some((|| -> R {
            let workstream = resolve_entity_arg(vc, "workstream-id", symbols)?;
            let screening_type = get_string_arg(vc, "screening-type")?;
            Ok((
                vec![Op::RunScreening {
                    workstream,
                    screening_type,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        "screening.pep" | "screening.sanctions" | "screening.adverse-media" => Some((|| -> R {
            let entity = resolve_entity_arg(vc, "entity-id", symbols)?;
            let screening_type = vc.verb.to_uppercase().replace('-', "_");
            let workstream = EntityKey::new("workstream", format!("synthetic:{}", entity.key));
            Ok((
                vec![Op::RunScreening {
                    workstream,
                    screening_type,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        // =================================================================
        // Custody verbs
        // =================================================================
        "cbu-custody.add-universe" => Some((|| -> R {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let instrument_class = get_string_arg(vc, "instrument-class")?;
            let market = get_string_arg(vc, "market").ok();
            let currencies = get_string_list_arg(vc, "currencies").unwrap_or_default();
            let settlement_types = get_string_list_arg(vc, "settlement-types")
                .unwrap_or_else(|_| vec!["DVP".to_string()]);
            Ok((
                vec![Op::AddUniverse {
                    cbu,
                    instrument_class,
                    market,
                    currencies,
                    settlement_types,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        "cbu-custody.create-ssi" => Some((|| -> R {
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
        })()),

        "cbu-custody.add-booking-rule" => Some((|| -> R {
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
            Ok((
                vec![Op::AddBookingRule {
                    cbu,
                    ssi,
                    name,
                    priority,
                    criteria,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        // =================================================================
        // Trading Profile verbs
        // =================================================================
        "trading-profile.import" => Some((|| -> R {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let key = DocKey::trading_profile(&cbu.key);
            let op = Op::UpsertDoc {
                key: key.clone(),
                content: serde_json::json!({}),
                cbu: Some(cbu.clone()),
                source_stmt: stmt_idx,
            };
            let entity_key = EntityKey::new("trading_profile", &key.key);
            let binding = vc.binding.as_ref().map(|b| (b.clone(), entity_key));
            Ok((vec![op], binding))
        })()),

        "trading-profile.materialize" => Some((|| -> R {
            let profile = resolve_entity_arg(vc, "profile-id", symbols)?;
            let sections =
                get_string_list_arg(vc, "sections").unwrap_or_else(|_| vec!["all".to_string()]);
            let force = get_bool_arg(vc, "force").unwrap_or(false);
            Ok((
                vec![Op::Materialize {
                    source: DocKey::trading_profile(&profile.key),
                    sections,
                    force,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        // =================================================================
        // Document verbs
        // =================================================================
        "document.catalog" => Some((|| -> R {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let doc_type = get_string_arg(vc, "doc-type")?;
            let title = get_string_arg(vc, "title")?;
            let key = DocKey::new(&doc_type, &title);
            let op = Op::UpsertDoc {
                key: key.clone(),
                content: serde_json::json!({ "doc_type": doc_type, "title": title }),
                cbu: Some(cbu),
                source_stmt: stmt_idx,
            };
            let entity_key = EntityKey::new("document", format!("{}:{}", key.doc_type, key.key));
            let binding = vc.binding.as_ref().map(|b| (b.clone(), entity_key));
            Ok((vec![op], binding))
        })()),

        // =================================================================
        // Evidence verbs
        // =================================================================
        "cbu.attach-evidence" => Some((|| -> R {
            let cbu = resolve_entity_arg(vc, "cbu-id", symbols)?;
            let evidence_type = get_string_arg(vc, "evidence-type")?;
            let doc = get_string_arg(vc, "document-id")
                .ok()
                .map(|d| DocKey::new("document", d));
            let attestation = get_string_arg(vc, "attestation-ref").ok();
            Ok((
                vec![Op::AttachEvidence {
                    cbu,
                    evidence_type,
                    document: doc,
                    attestation_ref: attestation,
                    source_stmt: stmt_idx,
                }],
                None,
            ))
        })()),

        _ => None,
    }
}

/// The ob-poc verb handler as a typed `VerbHandler` constant.
pub const OB_POC_VERB_HANDLER: VerbHandler = ob_poc_verb_handler;

#[cfg(test)]
mod tests {
    use super::*;
    use dsl_core::parser::parse_program;

    fn compile_ok(source: &str) {
        let program = parse_program(source).expect("parse failed");
        let compiled = compile_to_ops(&program);
        assert!(
            compiled.is_ok(),
            "compile failed for `{}`: {:?}",
            source,
            compiled.errors
        );
    }

    /// Startup-safety coverage: one representative verb from every family.
    /// If any arm is accidentally deleted or broken, this test fails at CI time
    /// rather than at runtime when a user tries to execute the verb.
    #[test]
    fn test_ob_poc_verb_handler_coverage() {
        compile_ok(r#"(cbu.ensure :name "Fund" :jurisdiction "LU")"#);
        compile_ok(r#"(cbu.ensure :name "Fund" :as @f)"#);

        compile_ok(
            r#"(entity.create :entity-type "proper-person" :first-name "Jane" :last-name "Doe")"#,
        );
        compile_ok(r#"(entity.create-proper-person :first-name "Jane" :last-name "Doe")"#);
        compile_ok(r#"(entity.create-limited-company :name "Acme Ltd")"#);
        compile_ok(r#"(entity.create-partnership-limited :name "Acme LP")"#);
        compile_ok(r#"(entity.create-trust-discretionary :name "Acme Trust")"#);

        compile_ok(
            r#"(ubo.add-ownership :owner-entity-id "e1" :owned-entity-id "e2" :percentage 50.0 :ownership-type "direct")"#,
        );

        compile_ok(r#"(kyc-case.create :cbu-id "cbu-1")"#);
        compile_ok(r#"(kyc-case.update-status :case-id "case-1" :status "APPROVED")"#);

        compile_ok(r#"(case-screening.run :workstream-id "ws-1" :screening-type "PEP")"#);
        compile_ok(r#"(screening.pep :entity-id "e-1")"#);

        compile_ok(r#"(cbu-custody.add-universe :cbu-id "cbu-1" :instrument-class "EQ")"#);
        compile_ok(r#"(cbu-custody.create-ssi :cbu-id "cbu-1" :name "SSI-1" :type "EUROCLEAR")"#);

        compile_ok(r#"(trading-profile.import :cbu-id "cbu-1")"#);

        compile_ok(r#"(document.catalog :cbu-id "cbu-1" :doc-type "KYC" :title "Passport")"#);

        compile_ok(r#"(cbu.attach-evidence :cbu-id "cbu-1" :evidence-type "passport")"#);
    }
}
