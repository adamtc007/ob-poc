//! Core DSL Vocabulary for the DSL Forth Engine.
//!
//! This module provides the vocabulary (word definitions) for all DSL verbs
//! across all domains: case, entity, products, kyc, ubo, document, isda, etc.

use crate::forth_engine::errors::VmError;
use crate::forth_engine::value::{AttributeId, Value};
use crate::forth_engine::value::{CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate};
use crate::forth_engine::vm::VM;
use crate::forth_engine::vocab::{Vocab, WordId, WordSpec};
use std::collections::HashMap;
use std::sync::Arc;

/// Collect keyword-value pairs from the stack
/// Returns a HashMap of keyword -> value pairs
fn collect_keyword_pairs(vm: &mut VM, num_pairs: usize) -> Result<HashMap<String, Value>, VmError> {
    let mut pairs = HashMap::new();

    for _ in 0..num_pairs {
        // Try to pop a keyword-value pair, but don't fail if stack is empty
        match vm.pop_keyword_value() {
            Ok((keyword, value)) => {
                pairs.insert(keyword, value);
            }
            Err(VmError::StackUnderflow { .. }) => {
                // No more pairs available, that's okay
                break;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(pairs)
}

/// Process collected pairs: extract case_id and store all as attributes
fn process_pairs(vm: &mut VM, pairs: &HashMap<String, Value>) {
    for (key, value) in pairs {
        // Extract case_id and store in environment
        if key == ":case-id" {
            if let Value::Str(case_id) = value {
                vm.env.set_case_id(case_id.clone());
            }
        }

        // Store all keyword-value pairs as attributes
        let attr_id = AttributeId(key.clone());
        vm.env.set_attribute(attr_id, value.clone());
    }
}

/// Typed word implementation that consumes a specific number of keyword-value pairs
fn typed_word(vm: &mut VM, _word_name: &str, num_pairs: usize) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, num_pairs)?;
    process_pairs(vm, &pairs);
    Ok(())
}

// Case Operations - stack_effect is (num_pairs * 2, 0)
fn word_case_create(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.create", 2) // :case-id, :case-type
}

fn word_case_update(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.update", 1) // :case-id
}

fn word_case_validate(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.validate", 1) // :case-id
}

fn word_case_approve(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.approve", 1) // :case-id
}

fn word_case_close(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "case.close", 1) // :case-id
}

// Entity Operations
fn word_entity_register(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "entity.register", 2) // :entity-id, :entity-type
}

fn word_entity_classify(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "entity.classify", 1) // :entity-id
}

fn word_entity_link(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "entity.link", 2) // :entity-id, :target-id
}

fn word_identity_verify(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "identity.verify", 1) // :entity-id
}

fn word_identity_attest(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "identity.attest", 1) // :entity-id
}

// Product Operations
fn word_products_add(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "products.add", 2) // :case-id, :product-type
}

fn word_products_configure(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "products.configure", 2) // :product-id, :config
}

fn word_services_discover(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "services.discover", 1) // :case-id
}

fn word_services_provision(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "services.provision", 2) // :service-id, :config
}

fn word_services_activate(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "services.activate", 1) // :service-id
}

// KYC Operations
fn word_kyc_start(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "kyc.start", 1) // :entity-id
}

fn word_kyc_collect(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "kyc.collect", 2) // :case-id, :collection-type
}

fn word_kyc_verify(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "kyc.verify", 1) // :entity-id
}

fn word_kyc_assess(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "kyc.assess", 1) // :entity-id
}

fn word_compliance_screen(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "compliance.screen", 1) // :entity-id
}

fn word_compliance_monitor(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "compliance.monitor", 1) // :entity-id
}

// UBO Operations
fn word_ubo_collect_entity_data(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "ubo.collect-entity-data", 1) // :entity-id
}

fn word_ubo_get_ownership_structure(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "ubo.get-ownership-structure", 1) // :entity-id
}

fn word_ubo_resolve_ubos(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "ubo.resolve-ubos", 1) // :entity-id
}

fn word_ubo_calculate_indirect_ownership(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "ubo.calculate-indirect-ownership", 1) // :entity-id
}

// Document Operations - emit CrudStatements
fn word_document_catalog(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :doc-id, :doc-type
    process_pairs(vm, &pairs);

    // Convert pairs to CRUD values
    let values: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                Value::Int(i) => Value::Float(i as f64),
                Value::Bool(b) => {
                    Value::Bool(b)
                }
                _ => Value::Str(format!("{:?}", v)),
            };
            (key, val)
        })
        .collect();

    // Emit CrudStatement for document creation
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT".to_string(),
        values,
    }));

    Ok(())
}

fn word_document_verify(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?; // :doc-id
    process_pairs(vm, &pairs);

    // Convert pairs to CRUD values
    let mut values: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                _ => Value::Str(format!("{:?}", v)),
            };
            (key, val)
        })
        .collect();

    // Add verification status update
    values.insert(
        "verification-status".to_string(),
        Value::Str(
            "verified".to_string(),
        ),
    );

    // Emit CrudStatement for document update
    vm.env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "DOCUMENT".to_string(),
        values: values.clone(),
        where_clause: values.into_iter().filter(|(k, _)| k == "doc-id").collect(),
    }));

    Ok(())
}

fn word_document_extract(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?; // :doc-id required, :attr-id, :cbu-id, :method, :value optional
    process_pairs(vm, &pairs);

    // Convert pairs to CRUD values
    let values: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => Value::Str(s),
                _ => Value::Str(format!("{:?}", v)),
            };
            (key, val)
        })
        .collect();

    // Check if this is a metadata extraction (has attr-id) or just status update
    if values.contains_key("attr-id") {
        // Emit CREATE for document_metadata
        vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
            asset: "DOCUMENT_METADATA".to_string(),
            values: values.clone(),
        }));
    } else {
        // Original behavior: update document extraction status
        let mut update_values = values.clone();
        update_values.insert(
            "extraction-status".to_string(),
            Value::Str("extracted".to_string()),
        );
        
        vm.env.push_crud(CrudStatement::DataUpdate(DataUpdate {
            asset: "DOCUMENT".to_string(),
            values: update_values.clone(),
            where_clause: update_values.into_iter().filter(|(k, _)| k == "doc-id").collect(),
        }));
    }

    Ok(())
}

fn word_document_link(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :doc-id, :entity-id
    process_pairs(vm, &pairs);

    // Convert pairs to CRUD values
    let values: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                _ => Value::Str(format!("{:?}", v)),
            };
            (key, val)
        })
        .collect();

    // Emit CrudStatement for document-entity relationship
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT_ENTITY_LINK".to_string(),
        values,
    }));

    Ok(())
}

// Low-level attribute operations (from original kyc_vocab)
fn word_require_attribute(vm: &mut VM) -> Result<(), VmError> {
    let attr_val = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 1,
        found: 0,
    })?;

    if let Value::Attr(_attr_id) = attr_val {
        Ok(())
    } else {
        Err(VmError::TypeError {
            expected: "AttributeId".to_string(),
            found: format!("{:?}", attr_val),
        })
    }
}

fn word_set_attribute(vm: &mut VM) -> Result<(), VmError> {
    let value = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 2,
        found: 1,
    })?;
    let attr_val = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 2,
        found: 0,
    })?;

    if let Value::Attr(id) = attr_val {
        vm.env.set_attribute(id, value);
        Ok(())
    } else {
        Err(VmError::TypeError {
            expected: "AttributeId".to_string(),
            found: format!("{:?}", attr_val),
        })
    }
}

// CBU Operations (Phase 4) - Now emit CrudStatements
fn word_cbu_create(vm: &mut VM) -> Result<(), VmError> {
    // Collect all keyword pairs from the stack (up to 5: :cbu-name, :client-type, :jurisdiction, :nature-purpose, :description)
    let pairs = collect_keyword_pairs(vm, 5)?;
    process_pairs(vm, &pairs);

    // Convert pairs to CRUD values
    let values: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                Value::Int(i) => Value::Float(i as f64),
                Value::Bool(b) => {
                    Value::Bool(b)
                }
                _ => Value::Str(
                    format!("{:?}", v),
),
            };
            (key, val)
        })
        .collect();

    // Emit CrudStatement
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU".to_string(),
        values,
    }));

    Ok(())
}

fn word_cbu_read(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?; // :cbu-id
    process_pairs(vm, &pairs);

    // Convert to where clause
    let where_clause: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                _ => Value::Str(
                    format!("{:?}", v),
),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "CBU".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));

    Ok(())
}

fn word_cbu_update(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :cbu-id, :status or other fields
    process_pairs(vm, &pairs);

    // Separate cbu-id (where clause) from update values
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (k, v) in pairs {
        let key = k.trim_start_matches(':').to_string();
        let val = match v {
            Value::Str(s) => {
                Value::Str(s)
            }
            Value::Int(i) => {
                Value::Float(i as f64)
            }
            Value::Bool(b) => {
                Value::Bool(b)
            }
            _ => Value::Str(format!("{:?}", v)),
        };

        if key == "cbu-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    vm.env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "CBU".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

fn word_cbu_delete(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?; // :cbu-id
    process_pairs(vm, &pairs);

    let where_clause: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                _ => Value::Str(
                    format!("{:?}", v),
),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "CBU".to_string(),
        where_clause,
    }));

    Ok(())
}

fn word_cbu_list(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?; // :filter (optional)
    process_pairs(vm, &pairs);

    let where_clause: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                _ => Value::Str(
                    format!("{:?}", v),
),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "CBU".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: None,
    }));

    Ok(())
}

fn word_cbu_attach_entity(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :entity-id, :role
    process_pairs(vm, &pairs);

    // Create a relationship record
    let values: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                _ => Value::Str(
                    format!("{:?}", v),
),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU_ENTITY_RELATIONSHIP".to_string(),
        values,
    }));

    Ok(())
}

fn word_cbu_attach_proper_person(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :person-name, :role
    process_pairs(vm, &pairs);

    let values: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                _ => Value::Str(
                    format!("{:?}", v),
),
            };
            (key, val)
        })
        .collect();

    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU_PROPER_PERSON".to_string(),
        values,
    }));

    Ok(())
}

fn word_cbu_finalize(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :cbu-id, :status
    process_pairs(vm, &pairs);

    // Separate cbu-id from status
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (k, v) in pairs {
        let key = k.trim_start_matches(':').to_string();
        let val = match v {
            Value::Str(s) => {
                Value::Str(s)
            }
            _ => Value::Str(format!("{:?}", v)),
        };

        if key == "cbu-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    vm.env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "CBU".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

// CRUD Operations (Phase 5)
fn word_crud_begin(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "crud.begin", 2) // :operation-type, :asset-type
}

fn word_crud_commit(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "crud.commit", 3) // :entity-table, :ai-instruction, :ai-provider
}

// Attribute Operations (Phase 2)
fn word_attr_require(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "attr.require", 1) // @attr reference
}

fn word_attr_set(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "attr.set", 2) // @attr reference, value
}

fn word_attr_validate(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "attr.validate", 2) // @attr reference, value
}

// Document Operations (Phase 3) - extended - emit CrudStatements
fn word_document_link_to_cbu(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 3)?; // :cbu-id, :document-id, :relationship-type
    process_pairs(vm, &pairs);

    // Convert pairs to CRUD values
    let values: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                _ => Value::Str(format!("{:?}", v)),
            };
            (key, val)
        })
        .collect();

    // Emit CrudStatement for document-CBU link
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT_CBU_LINK".to_string(),
        values,
    }));

    Ok(())
}

fn word_document_extract_attributes(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?; // :document-id, :document-type
    process_pairs(vm, &pairs);

    // Convert pairs to CRUD values
    let mut values: HashMap<String, Value> = pairs
        .into_iter()
        .map(|(k, v)| {
            let key = k.trim_start_matches(':').to_string();
            let val = match v {
                Value::Str(s) => {
                    Value::Str(s)
                }
                _ => Value::Str(format!("{:?}", v)),
            };
            (key, val)
        })
        .collect();

    // Add extraction status
    values.insert(
        "extraction-status".to_string(),
        Value::Str(
            "pending".to_string(),
        ),
    );

    // Emit CrudStatement for attribute extraction task
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT_EXTRACTION".to_string(),
        values,
    }));

    Ok(())
}

fn word_document_require(vm: &mut VM) -> Result<(), VmError> {
    // This is a validation/check operation, not a CRUD operation
    // Just validate the stack and track as required source attribute
    let attr_val = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 1,
        found: 0,
    })?;

    if let Value::Attr(attr_id) = attr_val {
        // Track this as a required source attribute
        // AttributeId.0 is a String that may be a UUID
        if let Ok(uuid) = uuid::Uuid::parse_str(&attr_id.0) {
            vm.env.source_attributes.insert(uuid);
        }
        // If not a valid UUID, we still consider the operation successful
        // The attribute reference was valid DSL syntax
        Ok(())
    } else {
        Err(VmError::TypeError {
            expected: "AttributeId (@doc reference)".to_string(),
            found: format!("{:?}", attr_val),
        })
    }
}

/// Constructs the complete DSL Vocabulary with all domain verbs.
pub fn kyc_orch_vocab() -> Vocab {
    let specs = vec![
        // Case Operations - stack_effect = (num_pairs * 2, 0)
        WordSpec {
            id: WordId(0),
            name: "case.create".to_string(),
            domain: "case".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_case_create),
        },
        WordSpec {
            id: WordId(1),
            name: "case.update".to_string(),
            domain: "case".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_case_update),
        },
        WordSpec {
            id: WordId(2),
            name: "case.validate".to_string(),
            domain: "case".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_case_validate),
        },
        WordSpec {
            id: WordId(3),
            name: "case.approve".to_string(),
            domain: "case".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_case_approve),
        },
        WordSpec {
            id: WordId(4),
            name: "case.close".to_string(),
            domain: "case".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_case_close),
        },
        // Entity Operations
        WordSpec {
            id: WordId(5),
            name: "entity.register".to_string(),
            domain: "entity".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_entity_register),
        },
        WordSpec {
            id: WordId(6),
            name: "entity.classify".to_string(),
            domain: "entity".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_entity_classify),
        },
        WordSpec {
            id: WordId(7),
            name: "entity.link".to_string(),
            domain: "entity".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_entity_link),
        },
        WordSpec {
            id: WordId(8),
            name: "identity.verify".to_string(),
            domain: "entity".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_identity_verify),
        },
        WordSpec {
            id: WordId(9),
            name: "identity.attest".to_string(),
            domain: "entity".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_identity_attest),
        },
        // Product Operations
        WordSpec {
            id: WordId(10),
            name: "products.add".to_string(),
            domain: "products".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_products_add),
        },
        WordSpec {
            id: WordId(11),
            name: "products.configure".to_string(),
            domain: "products".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_products_configure),
        },
        WordSpec {
            id: WordId(12),
            name: "services.discover".to_string(),
            domain: "services".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_services_discover),
        },
        WordSpec {
            id: WordId(13),
            name: "services.provision".to_string(),
            domain: "services".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_services_provision),
        },
        WordSpec {
            id: WordId(14),
            name: "services.activate".to_string(),
            domain: "services".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_services_activate),
        },
        // KYC Operations
        WordSpec {
            id: WordId(15),
            name: "kyc.start".to_string(),
            domain: "kyc".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_kyc_start),
        },
        WordSpec {
            id: WordId(16),
            name: "kyc.collect".to_string(),
            domain: "kyc".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_kyc_collect),
        },
        WordSpec {
            id: WordId(17),
            name: "kyc.verify".to_string(),
            domain: "kyc".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_kyc_verify),
        },
        WordSpec {
            id: WordId(18),
            name: "kyc.assess".to_string(),
            domain: "kyc".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_kyc_assess),
        },
        WordSpec {
            id: WordId(19),
            name: "compliance.screen".to_string(),
            domain: "compliance".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_compliance_screen),
        },
        WordSpec {
            id: WordId(20),
            name: "compliance.monitor".to_string(),
            domain: "compliance".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_compliance_monitor),
        },
        // UBO Operations
        WordSpec {
            id: WordId(21),
            name: "ubo.collect-entity-data".to_string(),
            domain: "ubo".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_ubo_collect_entity_data),
        },
        WordSpec {
            id: WordId(22),
            name: "ubo.get-ownership-structure".to_string(),
            domain: "ubo".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_ubo_get_ownership_structure),
        },
        WordSpec {
            id: WordId(23),
            name: "ubo.resolve-ubos".to_string(),
            domain: "ubo".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_ubo_resolve_ubos),
        },
        WordSpec {
            id: WordId(24),
            name: "ubo.calculate-indirect-ownership".to_string(),
            domain: "ubo".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_ubo_calculate_indirect_ownership),
        },
        // Document Operations
        WordSpec {
            id: WordId(25),
            name: "document.catalog".to_string(),
            domain: "document".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_document_catalog),
        },
        WordSpec {
            id: WordId(26),
            name: "document.verify".to_string(),
            domain: "document".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_document_verify),
        },
        WordSpec {
            id: WordId(27),
            name: "document.extract".to_string(),
            domain: "document".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_document_extract),
        },
        WordSpec {
            id: WordId(28),
            name: "document.link".to_string(),
            domain: "document".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_document_link),
        },
        // Low-level attribute operations
        WordSpec {
            id: WordId(29),
            name: "require-attribute".to_string(),
            domain: "core".to_string(),
            stack_effect: (1, 0),
            impl_fn: Arc::new(word_require_attribute),
        },
        WordSpec {
            id: WordId(30),
            name: "set-attribute".to_string(),
            domain: "core".to_string(),
            stack_effect: (2, 0),
            impl_fn: Arc::new(word_set_attribute),
        },
        // CBU Operations (Phase 4)
        WordSpec {
            id: WordId(31),
            name: "cbu.create".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (6, 0), // 3 pairs
            impl_fn: Arc::new(word_cbu_create),
        },
        WordSpec {
            id: WordId(32),
            name: "cbu.read".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_cbu_read),
        },
        WordSpec {
            id: WordId(33),
            name: "cbu.update".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_cbu_update),
        },
        WordSpec {
            id: WordId(34),
            name: "cbu.delete".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_cbu_delete),
        },
        WordSpec {
            id: WordId(35),
            name: "cbu.list".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_cbu_list),
        },
        WordSpec {
            id: WordId(36),
            name: "cbu.attach-entity".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_cbu_attach_entity),
        },
        WordSpec {
            id: WordId(37),
            name: "cbu.attach-proper-person".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_cbu_attach_proper_person),
        },
        WordSpec {
            id: WordId(38),
            name: "cbu.finalize".to_string(),
            domain: "cbu".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_cbu_finalize),
        },
        // CRUD Operations (Phase 5)
        WordSpec {
            id: WordId(39),
            name: "crud.begin".to_string(),
            domain: "crud".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_crud_begin),
        },
        WordSpec {
            id: WordId(40),
            name: "crud.commit".to_string(),
            domain: "crud".to_string(),
            stack_effect: (6, 0), // 3 pairs
            impl_fn: Arc::new(word_crud_commit),
        },
        // Attribute Operations (Phase 2)
        WordSpec {
            id: WordId(41),
            name: "attr.require".to_string(),
            domain: "attr".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_attr_require),
        },
        WordSpec {
            id: WordId(42),
            name: "attr.set".to_string(),
            domain: "attr".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_attr_set),
        },
        WordSpec {
            id: WordId(43),
            name: "attr.validate".to_string(),
            domain: "attr".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_attr_validate),
        },
        // Document Operations (Phase 3) - extended
        WordSpec {
            id: WordId(44),
            name: "document.link-to-cbu".to_string(),
            domain: "document".to_string(),
            stack_effect: (6, 0), // 3 pairs
            impl_fn: Arc::new(word_document_link_to_cbu),
        },
        WordSpec {
            id: WordId(45),
            name: "document.extract-attributes".to_string(),
            domain: "document".to_string(),
            stack_effect: (4, 0), // 2 pairs
            impl_fn: Arc::new(word_document_extract_attributes),
        },
        WordSpec {
            id: WordId(46),
            name: "document.require".to_string(),
            domain: "document".to_string(),
            stack_effect: (2, 0), // 1 pair
            impl_fn: Arc::new(word_document_require),
        },
        WordSpec {
            id: WordId(50),
            name: "product.create".to_string(),
            domain: "product".to_string(),
            stack_effect: (10, 0),
            impl_fn: Arc::new(word_product_create),
        },
        WordSpec {
            id: WordId(51),
            name: "product.read".to_string(),
            domain: "product".to_string(),
            stack_effect: (2, 0),
            impl_fn: Arc::new(word_product_read),
        },
        WordSpec {
            id: WordId(52),
            name: "product.update".to_string(),
            domain: "product".to_string(),
            stack_effect: (4, 0),
            impl_fn: Arc::new(word_product_update),
        },
        WordSpec {
            id: WordId(53),
            name: "product.delete".to_string(),
            domain: "product".to_string(),
            stack_effect: (2, 0),
            impl_fn: Arc::new(word_product_delete),
        },
        WordSpec {
            id: WordId(54),
            name: "service.create".to_string(),
            domain: "service".to_string(),
            stack_effect: (6, 0),
            impl_fn: Arc::new(word_service_create),
        },
        WordSpec {
            id: WordId(55),
            name: "service.read".to_string(),
            domain: "service".to_string(),
            stack_effect: (2, 0),
            impl_fn: Arc::new(word_service_read),
        },
        WordSpec {
            id: WordId(56),
            name: "service.update".to_string(),
            domain: "service".to_string(),
            stack_effect: (4, 0),
            impl_fn: Arc::new(word_service_update),
        },
        WordSpec {
            id: WordId(57),
            name: "service.delete".to_string(),
            domain: "service".to_string(),
            stack_effect: (2, 0),
            impl_fn: Arc::new(word_service_delete),
        },
        WordSpec {
            id: WordId(58),
            name: "service.link-product".to_string(),
            domain: "service".to_string(),
            stack_effect: (6, 0),
            impl_fn: Arc::new(word_service_link_product),
        },
        WordSpec {
            id: WordId(59),
            name: "lifecycle-resource.create".to_string(),
            domain: "lifecycle-resource".to_string(),
            stack_effect: (8, 0),
            impl_fn: Arc::new(word_lifecycle_resource_create),
        },
        WordSpec {
            id: WordId(60),
            name: "lifecycle-resource.read".to_string(),
            domain: "lifecycle-resource".to_string(),
            stack_effect: (2, 0),
            impl_fn: Arc::new(word_lifecycle_resource_read),
        },
        WordSpec {
            id: WordId(61),
            name: "lifecycle-resource.update".to_string(),
            domain: "lifecycle-resource".to_string(),
            stack_effect: (4, 0),
            impl_fn: Arc::new(word_lifecycle_resource_update),
        },
        WordSpec {
            id: WordId(62),
            name: "lifecycle-resource.delete".to_string(),
            domain: "lifecycle-resource".to_string(),
            stack_effect: (2, 0),
            impl_fn: Arc::new(word_lifecycle_resource_delete),
        },
        WordSpec {
            id: WordId(63),
            name: "lifecycle-resource.link-service".to_string(),
            domain: "lifecycle-resource".to_string(),
            stack_effect: (4, 0),
            impl_fn: Arc::new(word_lifecycle_resource_link_service),
        },
    ];
    Vocab::new(specs)
}

// ============================================================================
// TAXONOMY DOMAIN VERBS: Product, Service, Lifecycle Resource
// ============================================================================

/// product.create - Create a new product
fn word_product_create(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 5)?;
    process_pairs(vm, &pairs);
    
    let mut values = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "Product".to_string(),
        values,
    }));
    
    Ok(())
}

/// product.read - Read a product by ID or code
fn word_product_read(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?;
    
    let mut where_clause = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "Product".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));
    
    Ok(())
}

/// product.update - Update a product
fn word_product_update(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?;
    
    let mut where_clause = std::collections::HashMap::new();
    let mut values = std::collections::HashMap::new();
    
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        if clean_key == "product_id" || clean_key == "product_code" {
            where_clause.insert(clean_key, val);
        } else {
            values.insert(clean_key, val);
        }
    }
    
    vm.env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "Product".to_string(),
        where_clause,
        values,
    }));
    
    Ok(())
}

/// product.delete - Delete a product
fn word_product_delete(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?;
    
    let mut where_clause = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "Product".to_string(),
        where_clause,
    }));
    
    Ok(())
}

/// service.create - Create a new service
fn word_service_create(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 3)?;
    process_pairs(vm, &pairs);
    
    let mut values = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "Service".to_string(),
        values,
    }));
    
    Ok(())
}

/// service.read - Read a service
fn word_service_read(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?;
    
    let mut where_clause = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "Service".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));
    
    Ok(())
}

/// service.update - Update a service
fn word_service_update(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?;
    
    let mut where_clause = std::collections::HashMap::new();
    let mut values = std::collections::HashMap::new();
    
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        if clean_key == "service_id" || clean_key == "service_code" {
            where_clause.insert(clean_key, val);
        } else {
            values.insert(clean_key, val);
        }
    }
    
    vm.env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "Service".to_string(),
        where_clause,
        values,
    }));
    
    Ok(())
}

/// service.delete - Delete a service
fn word_service_delete(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?;
    
    let mut where_clause = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "Service".to_string(),
        where_clause,
    }));
    
    Ok(())
}

/// service.link-product - Link a service to a product
fn word_service_link_product(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 3)?;
    
    let mut values = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "ProductService".to_string(),
        values,
    }));
    
    Ok(())
}

/// lifecycle-resource.create - Create a lifecycle resource
fn word_lifecycle_resource_create(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 4)?;
    process_pairs(vm, &pairs);
    
    let mut values = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "LifecycleResource".to_string(),
        values,
    }));
    
    Ok(())
}

/// lifecycle-resource.read - Read a lifecycle resource
fn word_lifecycle_resource_read(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?;
    
    let mut where_clause = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "LifecycleResource".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));
    
    Ok(())
}

/// lifecycle-resource.update - Update a lifecycle resource
fn word_lifecycle_resource_update(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?;
    
    let mut where_clause = std::collections::HashMap::new();
    let mut values = std::collections::HashMap::new();
    
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        if clean_key == "resource_id" || clean_key == "resource_code" {
            where_clause.insert(clean_key, val);
        } else {
            values.insert(clean_key, val);
        }
    }
    
    vm.env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "LifecycleResource".to_string(),
        where_clause,
        values,
    }));
    
    Ok(())
}

/// lifecycle-resource.delete - Delete a lifecycle resource
fn word_lifecycle_resource_delete(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 1)?;
    
    let mut where_clause = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "LifecycleResource".to_string(),
        where_clause,
    }));
    
    Ok(())
}

/// lifecycle-resource.link-service - Link a resource to a service
fn word_lifecycle_resource_link_service(vm: &mut VM) -> Result<(), VmError> {
    let pairs = collect_keyword_pairs(vm, 2)?;
    
    let mut values = std::collections::HashMap::new();
    for (key, val) in pairs {
        let clean_key = key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, val);
    }
    
    vm.env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "ServiceResource".to_string(),
        values,
    }));
    
    Ok(())
}
