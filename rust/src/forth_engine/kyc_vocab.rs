//! Core DSL Vocabulary for the DSL Forth Engine.
//!
//! This module provides the vocabulary (word definitions) for all DSL verbs
//! across all domains: case, entity, products, kyc, ubo, document, isda, etc.

use crate::forth_engine::errors::VmError;
use crate::forth_engine::value::{AttributeId, Value};
use crate::forth_engine::vm::VM;
use crate::forth_engine::vocab::{Vocab, WordId, WordSpec};
use std::collections::HashMap;
use std::sync::Arc;

/// Collect keyword-value pairs from the stack
/// Returns a HashMap of keyword -> value pairs
fn collect_keyword_pairs(vm: &mut VM, num_pairs: usize) -> Result<HashMap<String, Value>, VmError> {
    let mut pairs = HashMap::new();

    for _ in 0..num_pairs {
        let (keyword, value) = vm.pop_keyword_value()?;
        pairs.insert(keyword, value);
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

// Document Operations
fn word_document_catalog(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.catalog", 2) // :doc-id, :doc-type
}

fn word_document_verify(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.verify", 1) // :doc-id
}

fn word_document_extract(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.extract", 1) // :doc-id
}

fn word_document_link(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.link", 2) // :doc-id, :entity-id
}

// Low-level attribute operations (from original kyc_vocab)
fn word_require_attribute(vm: &mut VM) -> Result<(), VmError> {
    println!("[word] Executing 'require-attribute'");
    let attr_val = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 1,
        found: 0,
    })?;

    if let Value::Attr(attr_id) = attr_val {
        println!("[word] Checking for attribute: {}", attr_id.0);
        Ok(())
    } else {
        Err(VmError::TypeError {
            expected: "AttributeId".to_string(),
            found: format!("{:?}", attr_val),
        })
    }
}

fn word_set_attribute(vm: &mut VM) -> Result<(), VmError> {
    println!("[word] Executing 'set-attribute'");
    let value = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 2,
        found: 1,
    })?;
    let attr_val = vm.data_stack.pop_back().ok_or(VmError::StackUnderflow {
        expected: 2,
        found: 0,
    })?;

    if let Value::Attr(id) = attr_val {
        println!("[word] Setting attribute '{}' to '{}'", id.0, value);
        Ok(())
    } else {
        Err(VmError::TypeError {
            expected: "AttributeId".to_string(),
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
    ];
    Vocab::new(specs)
}
