//! Word implementations for DSL vocabulary
//!
//! Each function: fn(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError>
//!
//! Ported from kyc_vocab.rs stack-based implementation to direct argument passing.

use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::errors::EngineError;
use crate::forth_engine::runtime::Arg;
use crate::forth_engine::value::{
    AttributeId, CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate, DataUpsert, Value,
};
use std::collections::HashMap;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Convert args to CRUD values HashMap, stripping leading colons from keys
fn args_to_crud_values(args: &[Arg]) -> HashMap<String, Value> {
    args.iter()
        .map(|a| {
            let key = a.key.trim_start_matches(':').to_string();
            (key, a.value.clone())
        })
        .collect()
}

/// Process pairs: extract case_id and store all as attributes in env
fn process_args(args: &[Arg], env: &mut RuntimeEnv) {
    for arg in args {
        // Extract case_id and store in environment
        if arg.key == ":case-id" {
            if let Value::Str(case_id) = &arg.value {
                env.set_case_id(case_id.clone());
            }
        }

        // Store all keyword-value pairs as attributes
        let attr_id = AttributeId(arg.key.clone());
        env.set_attribute(attr_id, arg.value.clone());
    }
}

// ============================================================================
// CONTEXT INJECTION HELPERS
// ============================================================================

/// Inject cbu_id from RuntimeEnv into values if not already present
fn inject_cbu_id_if_missing(values: &mut HashMap<String, Value>, env: &RuntimeEnv) {
    if !values.contains_key("cbu-id") {
        if let Some(cbu_id) = &env.cbu_id {
            values.insert("cbu-id".to_string(), Value::Str(cbu_id.to_string()));
        }
    }
}

/// Inject entity_id from RuntimeEnv into values if not already present
fn inject_entity_id_if_missing(values: &mut HashMap<String, Value>, env: &RuntimeEnv) {
    if !values.contains_key("entity-id") {
        if let Some(entity_id) = &env.entity_id {
            values.insert("entity-id".to_string(), Value::Str(entity_id.to_string()));
        }
    }
}

/// Inject investigation_id from RuntimeEnv into values if not already present
fn inject_investigation_id_if_missing(values: &mut HashMap<String, Value>, env: &RuntimeEnv) {
    if !values.contains_key("investigation-id") {
        if let Some(inv_id) = &env.investigation_id {
            values.insert(
                "investigation-id".to_string(),
                Value::Str(inv_id.to_string()),
            );
        }
    }
}

/// Inject decision_id from RuntimeEnv into values if not already present
fn inject_decision_id_if_missing(values: &mut HashMap<String, Value>, env: &RuntimeEnv) {
    if !values.contains_key("decision-id") {
        if let Some(dec_id) = &env.decision_id {
            values.insert("decision-id".to_string(), Value::Str(dec_id.to_string()));
        }
    }
}

// ============================================================================
// CASE DOMAIN
// ============================================================================

pub fn case_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn case_update(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn case_validate(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn case_approve(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn case_close(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

// ============================================================================
// ENTITY DOMAIN
// ============================================================================

pub fn entity_register(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn entity_classify(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn entity_link(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn identity_verify(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn identity_attest(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

// ============================================================================
// PRODUCT DOMAIN
// ============================================================================

pub fn products_add(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn products_configure(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn services_discover(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn services_provision(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn services_activate(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

// ============================================================================
// KYC DOMAIN
// ============================================================================

pub fn kyc_start(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn kyc_collect(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn kyc_verify(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn kyc_assess(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn compliance_screen(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn compliance_monitor(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

// ============================================================================
// UBO DOMAIN
// ============================================================================

pub fn ubo_collect_entity_data(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn ubo_get_ownership_structure(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn ubo_resolve_ubos(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn ubo_calculate_indirect_ownership(
    args: &[Arg],
    env: &mut RuntimeEnv,
) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

// ============================================================================
// DOCUMENT DOMAIN - emit CrudStatements
// ============================================================================

pub fn document_catalog(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn document_verify(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);
    values.insert(
        "verification-status".to_string(),
        Value::Str("verified".to_string()),
    );

    let where_clause: HashMap<String, Value> = values
        .iter()
        .filter(|(k, _)| *k == "doc-id")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "DOCUMENT".to_string(),
        values,
        where_clause,
    }));

    Ok(())
}

pub fn document_extract(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    // Check if this is a metadata extraction (has attr-id) or just status update
    if values.contains_key("attr-id") {
        env.push_crud(CrudStatement::DataCreate(DataCreate {
            asset: "DOCUMENT_METADATA".to_string(),
            values,
        
        capture_result: None,}));
    } else {
        let mut update_values = values.clone();
        update_values.insert(
            "extraction-status".to_string(),
            Value::Str("extracted".to_string()),
        );

        let where_clause: HashMap<String, Value> = update_values
            .iter()
            .filter(|(k, _)| *k == "doc-id")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        env.push_crud(CrudStatement::DataUpdate(DataUpdate {
            asset: "DOCUMENT".to_string(),
            values: update_values,
            where_clause,
        }));
    }

    Ok(())
}

pub fn document_link(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT_ENTITY_LINK".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn document_link_to_cbu(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT_CBU_LINK".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn document_extract_attributes(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);
    values.insert(
        "extraction-status".to_string(),
        Value::Str("pending".to_string()),
    );

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT_EXTRACTION".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn document_require(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // This is a validation/check operation
    // Track as required source attribute if we have an attribute reference
    for arg in args {
        if let Value::Attr(attr_id) = &arg.value {
            if let Ok(uuid) = uuid::Uuid::parse_str(&attr_id.0) {
                env.source_attributes.insert(uuid);
            }
        }
    }
    Ok(())
}

// ============================================================================
// CORE/ATTRIBUTE DOMAIN
// ============================================================================

pub fn require_attribute(args: &[Arg], _env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // Validate we have an attribute reference in positional args
    for arg in args {
        if let Value::Attr(_) = &arg.value {
            return Ok(());
        }
    }
    Err(EngineError::MissingArgument("attribute reference".into()))
}

pub fn set_attribute(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    // Find attribute ref and value
    let mut attr_id = None;
    let mut value = None;

    for arg in args {
        if let Value::Attr(id) = &arg.value {
            attr_id = Some(id.clone());
        } else if attr_id.is_some() && value.is_none() {
            value = Some(arg.value.clone());
        }
    }

    if let (Some(id), Some(val)) = (attr_id, value) {
        env.set_attribute(id, val);
        Ok(())
    } else {
        Err(EngineError::MissingArgument(
            "attribute reference and value".into(),
        ))
    }
}

pub fn attr_require(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn attr_set(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn attr_validate(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

// ============================================================================
// CBU DOMAIN - emit CrudStatements
// ============================================================================

pub fn cbu_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU".to_string(),
        values,
        capture_result: Some("cbu_id".to_string()),
    }));

    Ok(())
}

pub fn cbu_read(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "CBU".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));

    Ok(())
}

pub fn cbu_update(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    // Separate cbu-id (where clause) from update values
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "cbu-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "CBU".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

pub fn cbu_delete(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "CBU".to_string(),
        where_clause,
    }));

    Ok(())
}

pub fn cbu_list(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "CBU".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: None,
    }));

    Ok(())
}

pub fn cbu_attach_entity(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU_ENTITY_ROLE".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn cbu_attach_proper_person(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU_PROPER_PERSON".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn cbu_finalize(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    // Separate cbu-id from status
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "cbu-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "CBU".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

// ============================================================================
// CRUD DOMAIN
// ============================================================================

pub fn crud_begin(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

pub fn crud_commit(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);
    Ok(())
}

// ============================================================================
// TAXONOMY DOMAIN: Product, Service, Lifecycle Resource
// ============================================================================

pub fn product_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "Product".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn product_read(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut where_clause = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "Product".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));

    Ok(())
}

pub fn product_update(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        if clean_key == "product_id" || clean_key == "product_code" {
            where_clause.insert(clean_key, arg.value.clone());
        } else {
            values.insert(clean_key, arg.value.clone());
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "Product".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

pub fn product_delete(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut where_clause = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "Product".to_string(),
        where_clause,
    }));

    Ok(())
}

pub fn service_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "Service".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn service_read(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut where_clause = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "Service".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));

    Ok(())
}

pub fn service_update(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        if clean_key == "service_id" || clean_key == "service_code" {
            where_clause.insert(clean_key, arg.value.clone());
        } else {
            values.insert(clean_key, arg.value.clone());
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "Service".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

pub fn service_delete(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut where_clause = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "Service".to_string(),
        where_clause,
    }));

    Ok(())
}

pub fn service_link_product(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut values = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "ProductService".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn lifecycle_resource_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "LifecycleResource".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

pub fn lifecycle_resource_read(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut where_clause = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "LifecycleResource".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));

    Ok(())
}

pub fn lifecycle_resource_update(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        if clean_key == "resource_id" || clean_key == "resource_code" {
            where_clause.insert(clean_key, arg.value.clone());
        } else {
            values.insert(clean_key, arg.value.clone());
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "LifecycleResource".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

pub fn lifecycle_resource_delete(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    let mut where_clause = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        where_clause.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "LifecycleResource".to_string(),
        where_clause,
    }));

    Ok(())
}

pub fn lifecycle_resource_link_service(
    args: &[Arg],
    env: &mut RuntimeEnv,
) -> Result<(), EngineError> {
    let mut values = HashMap::new();
    for arg in args {
        let clean_key = arg.key.trim_start_matches(':').replace('-', "_");
        values.insert(clean_key, arg.value.clone());
    }

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "ServiceResource".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

// ============================================================================
// ENTITY TYPE CREATION WORDS
// ============================================================================

/// Create a proper person (natural individual)
pub fn entity_create_proper_person(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "PROPER_PERSON".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Create a limited company entity
pub fn entity_create_limited_company(
    args: &[Arg],
    env: &mut RuntimeEnv,
) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "LIMITED_COMPANY".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Create a partnership entity (LP, LLP, GP)
pub fn entity_create_partnership(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "PARTNERSHIP".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Create a trust entity
pub fn entity_create_trust(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "TRUST".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Read an entity by ID (returns base + type extension)
pub fn entity_read(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "ENTITY".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: Some(1),
    }));

    Ok(())
}

/// Update an entity's base fields
pub fn entity_update(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    // Separate entity-id (where clause) from update values
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "entity-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "ENTITY".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

/// Delete an entity (cascades to type extension)
pub fn entity_delete(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "ENTITY".to_string(),
        where_clause,
    }));

    Ok(())
}

/// List entities with optional filters
pub fn entity_list(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "ENTITY_LIST".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: None,
    }));

    Ok(())
}

// ============================================================================
// CBU ENTITY ATTACHMENT WORDS (Hub-Spoke Model)
// ============================================================================

/// Detach an entity from a CBU (optionally for specific role)
pub fn cbu_detach_entity(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "CBU_ENTITY_ROLE".to_string(),
        where_clause,
    }));

    Ok(())
}

/// List all entities attached to a CBU (optionally filtered by role)
pub fn cbu_list_entities(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "CBU_ENTITY_ROLE".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: None,
    }));

    Ok(())
}

/// Change an entity's role within a CBU
pub fn cbu_update_entity_role(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    // Separate identifiers (where clause) from new role value
    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "cbu-id" || key == "entity-id" || key == "old-role" {
            where_clause.insert(key, val);
        } else if key == "new-role" {
            values.insert("role".to_string(), val);
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "CBU_ENTITY_ROLE".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

// ============================================================================
// IDEMPOTENT ENSURE VERBS (UPSERT semantics)
// ============================================================================

/// Idempotent CBU create-or-update using natural key (name, jurisdiction)
pub fn cbu_ensure(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataUpsert(DataUpsert {
        asset: "CBU".to_string(),
        values,
        conflict_keys: vec!["cbu-name".to_string()],
        capture_result: Some("cbu_id".to_string()),
    }));

    Ok(())
}

/// Idempotent limited company create-or-update using natural key (company_number)
pub fn entity_ensure_limited_company(
    args: &[Arg],
    env: &mut RuntimeEnv,
) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataUpsert(DataUpsert {
        asset: "LIMITED_COMPANY".to_string(),
        values,
        conflict_keys: vec!["company-number".to_string()],
    
        capture_result: None,}));

    Ok(())
}

/// Idempotent proper person create-or-update using natural key (tax_id or name+dob)
pub fn entity_ensure_proper_person(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataUpsert(DataUpsert {
        asset: "PROPER_PERSON".to_string(),
        values,
        conflict_keys: vec!["tax-id".to_string()],
    
        capture_result: None,}));

    Ok(())
}

/// Idempotent ownership edge create-or-update
pub fn entity_ensure_ownership(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataUpsert(DataUpsert {
        asset: "OWNERSHIP_EDGE".to_string(),
        values,
        conflict_keys: vec![
            "from-entity-id".to_string(),
            "to-entity-id".to_string(),
            "ownership-type".to_string(),
        ],
    
        capture_result: None,}));

    Ok(())
}

/// Update ownership percentage/details
pub fn entity_update_ownership(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "from-entity-id" || key == "to-entity-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "OWNERSHIP_EDGE".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

/// Remove ownership edge
pub fn entity_remove_ownership(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataDelete(DataDelete {
        asset: "OWNERSHIP_EDGE".to_string(),
        where_clause,
    }));

    Ok(())
}

/// Get full ownership chain to a target
pub fn entity_get_ownership_chain(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let where_clause = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataRead(DataRead {
        asset: "OWNERSHIP_CHAIN".to_string(),
        where_clause,
        select: vec!["*".to_string()],
        limit: None,
    }));

    Ok(())
}

// ============================================================================
// INVESTIGATION DOMAIN
// ============================================================================

/// Create a KYC investigation
pub fn investigation_create(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);

    // Inject cbu_id from context if not provided
    inject_cbu_id_if_missing(&mut values, env);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "INVESTIGATION".to_string(),
        values,
        capture_result: Some("investigation_id".to_string()),
    }));

    Ok(())
}

/// Update investigation status
pub fn investigation_update_status(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut all_values = args_to_crud_values(args);

    // Inject investigation_id from context if not provided
    inject_investigation_id_if_missing(&mut all_values, env);

    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "investigation-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "INVESTIGATION".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

/// Assign analyst to investigation
pub fn investigation_assign(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "INVESTIGATION_ASSIGNMENT".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Complete investigation
pub fn investigation_complete(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "investigation-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    // Add completion status
    values.insert("status".to_string(), Value::Str("COMPLETE".to_string()));

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "INVESTIGATION".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

// ============================================================================
// DOCUMENT COLLECTION DOMAIN (new words)
// ============================================================================

/// Request a document for an investigation
pub fn document_request(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT_REQUEST".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Record document received
pub fn document_receive(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "request-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    values.insert("status".to_string(), Value::Str("RECEIVED".to_string()));

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "DOCUMENT_REQUEST".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

// ============================================================================
// TRUST DOMAIN
// ============================================================================

/// Add party to a trust (settlor, trustee, beneficiary, protector)
pub fn trust_add_party(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "TRUST_PARTY".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

// ============================================================================
// PARTNERSHIP DOMAIN
// ============================================================================

/// Add partner to a partnership
pub fn partnership_add_partner(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "PARTNERSHIP_PARTNER".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

// ============================================================================
// UBO DOMAIN (new words)
// ============================================================================

/// Calculate UBOs for a CBU (traverses ownership graph)
pub fn ubo_calculate(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "UBO_CALCULATION".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Manually flag a UBO (override calculation)
pub fn ubo_flag(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "UBO_FLAG".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Verify a calculated UBO
pub fn ubo_verify(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "ubo-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    values.insert("verified".to_string(), Value::Bool(true));

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "UBO_REGISTRY".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

/// Clear a UBO flag
pub fn ubo_clear(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "ubo-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    values.insert("status".to_string(), Value::Str("CLEARED".to_string()));

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "UBO_REGISTRY".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

// ============================================================================
// SCREENING DOMAIN
// ============================================================================

/// PEP screening
pub fn screening_pep(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "SCREENING_PEP".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Sanctions screening
pub fn screening_sanctions(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "SCREENING_SANCTIONS".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Adverse media screening
pub fn screening_adverse_media(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "SCREENING_ADVERSE_MEDIA".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Record screening result
pub fn screening_record_result(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "SCREENING_RESULT".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Resolve screening match
pub fn screening_resolve(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "SCREENING_RESOLUTION".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

// ============================================================================
// RISK DOMAIN
// ============================================================================

/// Assess entity risk
pub fn risk_assess_entity(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "RISK_ASSESSMENT_ENTITY".to_string(),
        values,
    
        capture_result: None,}));

    Ok(())
}

/// Assess CBU overall risk
pub fn risk_assess_cbu(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);

    // Inject context IDs
    inject_cbu_id_if_missing(&mut values, env);
    inject_investigation_id_if_missing(&mut values, env);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "RISK_ASSESSMENT_CBU".to_string(),
        values,
        capture_result: None,
    }));

    Ok(())
}

/// Set risk rating
pub fn risk_set_rating(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);

    // Inject context IDs
    inject_cbu_id_if_missing(&mut values, env);
    inject_investigation_id_if_missing(&mut values, env);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "RISK_RATING".to_string(),
        values,
        capture_result: None,
    }));

    Ok(())
}

/// Add risk flag
pub fn risk_add_flag(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);

    // Inject context ID
    inject_cbu_id_if_missing(&mut values, env);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "RISK_FLAG".to_string(),
        values,
        capture_result: None,
    }));

    Ok(())
}

// ============================================================================
// DECISION DOMAIN
// ============================================================================

/// Record onboarding decision
pub fn decision_record(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);

    // Inject context IDs
    inject_cbu_id_if_missing(&mut values, env);
    inject_investigation_id_if_missing(&mut values, env);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DECISION".to_string(),
        values,
        capture_result: Some("decision_id".to_string()),
    }));

    Ok(())
}

/// Add condition to conditional acceptance
pub fn decision_add_condition(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);

    // Inject decision_id from context
    inject_decision_id_if_missing(&mut values, env);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DECISION_CONDITION".to_string(),
        values,
        capture_result: None,
    }));

    Ok(())
}

/// Satisfy a condition
pub fn decision_satisfy_condition(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "condition-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    values.insert("status".to_string(), Value::Str("SATISFIED".to_string()));

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "DECISION_CONDITION".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

/// Review decision
pub fn decision_review(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "decision-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "DECISION".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}

// ============================================================================
// MONITORING DOMAIN
// ============================================================================

/// Setup ongoing monitoring
pub fn monitoring_setup(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);

    // Inject cbu_id from context
    inject_cbu_id_if_missing(&mut values, env);

    // Use UPSERT since monitoring_setup has unique constraint on cbu_id
    env.push_crud(CrudStatement::DataUpsert(DataUpsert {
        asset: "MONITORING_SETUP".to_string(),
        values,
        conflict_keys: vec!["cbu-id".to_string()],
        capture_result: None,
    }));

    Ok(())
}

/// Record monitoring event
pub fn monitoring_record_event(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);

    // Inject cbu_id from context
    inject_cbu_id_if_missing(&mut values, env);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "MONITORING_EVENT".to_string(),
        values,
        capture_result: None,
    }));

    Ok(())
}

/// Schedule a review
pub fn monitoring_schedule_review(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let mut values = args_to_crud_values(args);

    // Inject cbu_id from context
    inject_cbu_id_if_missing(&mut values, env);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "SCHEDULED_REVIEW".to_string(),
        values,
        capture_result: None,
    }));

    Ok(())
}

/// Complete a scheduled review
pub fn monitoring_complete_review(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let all_values = args_to_crud_values(args);

    let mut where_clause = HashMap::new();
    let mut values = HashMap::new();

    for (key, val) in all_values {
        if key == "review-id" {
            where_clause.insert(key, val);
        } else {
            values.insert(key, val);
        }
    }

    values.insert("status".to_string(), Value::Str("COMPLETED".to_string()));

    env.push_crud(CrudStatement::DataUpdate(DataUpdate {
        asset: "SCHEDULED_REVIEW".to_string(),
        where_clause,
        values,
    }));

    Ok(())
}
