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

// CBU Operations (Phase 4)
fn word_cbu_create(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "cbu.create", 3) // :cbu-name, :client-type, :jurisdiction
}

fn word_cbu_read(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "cbu.read", 1) // :cbu-id
}

fn word_cbu_update(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "cbu.update", 2) // :cbu-id, :status or other fields
}

fn word_cbu_delete(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "cbu.delete", 1) // :cbu-id
}

fn word_cbu_list(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "cbu.list", 1) // :filter (optional)
}

fn word_cbu_attach_entity(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "cbu.attach-entity", 2) // :entity-id, :role
}

fn word_cbu_attach_proper_person(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "cbu.attach-proper-person", 2) // :person-name, :role
}

fn word_cbu_finalize(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "cbu.finalize", 2) // :cbu-id, :status
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

// Document Operations (Phase 3) - extended
fn word_document_link_to_cbu(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.link-to-cbu", 3) // :cbu-id, :document-id, :relationship-type
}

fn word_document_extract_attributes(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.extract-attributes", 2) // :document-id, :document-type
}

fn word_document_require(vm: &mut VM) -> Result<(), VmError> {
    typed_word(vm, "document.require", 1) // @doc reference
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
    ];
    Vocab::new(specs)
}
