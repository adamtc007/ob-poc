//! Word implementations for DSL vocabulary
//!
//! Each function: fn(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError>
//!
//! Ported from kyc_vocab.rs stack-based implementation to direct argument passing.

use crate::forth_engine::env::RuntimeEnv;
use crate::forth_engine::errors::EngineError;
use crate::forth_engine::runtime::Arg;
use crate::forth_engine::value::{
    AttributeId, CrudStatement, DataCreate, DataDelete, DataRead, DataUpdate, Value,
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
    }));

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
        }));
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
    }));

    Ok(())
}

pub fn document_link_to_cbu(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "DOCUMENT_CBU_LINK".to_string(),
        values,
    }));

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
    }));

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
    }));

    Ok(())
}

pub fn cbu_attach_proper_person(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "CBU_PROPER_PERSON".to_string(),
        values,
    }));

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
    }));

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
    }));

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
    }));

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
    }));

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
    }));

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
    }));

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
    }));

    Ok(())
}

/// Create a partnership entity (LP, LLP, GP)
pub fn entity_create_partnership(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "PARTNERSHIP".to_string(),
        values,
    }));

    Ok(())
}

/// Create a trust entity
pub fn entity_create_trust(args: &[Arg], env: &mut RuntimeEnv) -> Result<(), EngineError> {
    process_args(args, env);

    let values = args_to_crud_values(args);

    env.push_crud(CrudStatement::DataCreate(DataCreate {
        asset: "TRUST".to_string(),
        values,
    }));

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
