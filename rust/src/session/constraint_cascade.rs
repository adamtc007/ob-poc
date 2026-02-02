//! Constraint Cascade Logic
//!
//! Derives search scopes and filters from session context.
//! The cascade flows: client → structure_type → current_structure → case
//!
//! Example:
//! - Client "Allianz" selected → entity search narrows from 10,000 to ~500
//! - Structure type "PE" selected → narrows to ~50
//! - Current structure selected → narrows to ~5-10 related entities
//!
//! This module provides functions to:
//! - Derive search scopes from session state
//! - Compute effective filters for entity lookups
//! - Check cascade validity (can't select structure without client, etc.)

use uuid::Uuid;

use super::unified::{
    CaseRef, ClientRef, Persona, SearchScope, StructureRef, StructureType, UnifiedSession,
};

// =============================================================================
// SEARCH SCOPE DERIVATION
// =============================================================================

/// Derive the current search scope from session state
///
/// The search scope is used to narrow entity queries based on the
/// constraint cascade. Each level in the cascade further constrains
/// the search space.
pub fn derive_search_scope(session: &UnifiedSession) -> SearchScope {
    SearchScope {
        client_id: session.client.as_ref().map(|c| c.client_id),
        structure_type: session.structure_type,
        structure_id: session.current_structure.as_ref().map(|s| s.structure_id),
    }
}

/// Extended search scope with case context
#[derive(Debug, Clone, Default)]
pub struct ExtendedSearchScope {
    pub base: SearchScope,
    pub case_id: Option<Uuid>,
    pub persona: Persona,
}

/// Derive extended search scope including case and persona
pub fn derive_extended_scope(session: &UnifiedSession) -> ExtendedSearchScope {
    ExtendedSearchScope {
        base: derive_search_scope(session),
        case_id: session.current_case.as_ref().map(|c| c.case_id),
        persona: session.persona,
    }
}

// =============================================================================
// CASCADE VALIDATION
// =============================================================================

/// Errors that can occur when modifying the constraint cascade
#[derive(Debug, Clone, thiserror::Error)]
pub enum CascadeError {
    #[error("Cannot select structure type without selecting a client first")]
    NoClientForStructureType,

    #[error("Cannot select structure without selecting a client first")]
    NoClientForStructure,

    #[error("Cannot select case without selecting a structure first")]
    NoStructureForCase,

    #[error("Structure type mismatch: selected structure is {actual}, expected {expected}")]
    StructureTypeMismatch {
        expected: StructureType,
        actual: StructureType,
    },

    #[error("Structure does not belong to current client")]
    StructureClientMismatch,

    #[error("Case does not belong to current structure")]
    CaseStructureMismatch,
}

/// Validate that setting a client is valid (always valid)
pub fn validate_set_client(
    _session: &UnifiedSession,
    _client: &ClientRef,
) -> Result<(), CascadeError> {
    // Setting a client is always valid - it may clear downstream selections
    Ok(())
}

/// Validate that setting structure type is valid
pub fn validate_set_structure_type(
    session: &UnifiedSession,
    _structure_type: StructureType,
) -> Result<(), CascadeError> {
    if session.client.is_none() {
        return Err(CascadeError::NoClientForStructureType);
    }
    Ok(())
}

/// Validate that selecting a structure is valid
pub fn validate_set_structure(
    session: &UnifiedSession,
    structure: &StructureRef,
) -> Result<(), CascadeError> {
    // Must have a client selected
    if session.client.is_none() {
        return Err(CascadeError::NoClientForStructure);
    }

    // If structure_type is set, the structure must match
    if let Some(expected_type) = session.structure_type {
        if structure.structure_type != expected_type {
            return Err(CascadeError::StructureTypeMismatch {
                expected: expected_type,
                actual: structure.structure_type,
            });
        }
    }

    Ok(())
}

/// Validate that selecting a case is valid
pub fn validate_set_case(session: &UnifiedSession, _case: &CaseRef) -> Result<(), CascadeError> {
    // Must have a structure selected
    if session.current_structure.is_none() {
        return Err(CascadeError::NoStructureForCase);
    }

    Ok(())
}

// =============================================================================
// CASCADE SETTERS (with automatic downstream clearing)
// =============================================================================

/// Result of a cascade update, indicating what was cleared
#[derive(Debug, Clone, Default)]
pub struct CascadeUpdateResult {
    pub client_cleared: bool,
    pub structure_type_cleared: bool,
    pub structure_cleared: bool,
    pub case_cleared: bool,
}

/// Set client and clear downstream selections
pub fn set_client(session: &mut UnifiedSession, client: Option<ClientRef>) -> CascadeUpdateResult {
    let mut result = CascadeUpdateResult::default();

    let client_changed = match (&session.client, &client) {
        (Some(old), Some(new)) => old.client_id != new.client_id,
        (None, Some(_)) => true,
        (Some(_), None) => true,
        (None, None) => false,
    };

    if client_changed {
        // Clear downstream
        if session.structure_type.is_some() {
            session.structure_type = None;
            result.structure_type_cleared = true;
        }
        if session.current_structure.is_some() {
            session.current_structure = None;
            result.structure_cleared = true;
        }
        if session.current_case.is_some() {
            session.current_case = None;
            result.case_cleared = true;
        }
    }

    session.client = client;
    result.client_cleared = client_changed && session.client.is_none();
    result
}

/// Set structure type and clear downstream selections
pub fn set_structure_type(
    session: &mut UnifiedSession,
    structure_type: Option<StructureType>,
) -> Result<CascadeUpdateResult, CascadeError> {
    if structure_type.is_some() && session.client.is_none() {
        return Err(CascadeError::NoClientForStructureType);
    }

    let mut result = CascadeUpdateResult::default();

    let type_changed = session.structure_type != structure_type;

    if type_changed {
        // Clear downstream if structure doesn't match new type
        if let (Some(structure), Some(new_type)) = (&session.current_structure, &structure_type) {
            if structure.structure_type != *new_type {
                session.current_structure = None;
                result.structure_cleared = true;

                if session.current_case.is_some() {
                    session.current_case = None;
                    result.case_cleared = true;
                }
            }
        }
    }

    session.structure_type = structure_type;
    result.structure_type_cleared = type_changed && session.structure_type.is_none();
    Ok(result)
}

/// Set current structure and clear downstream case
pub fn set_structure(
    session: &mut UnifiedSession,
    structure: Option<StructureRef>,
) -> Result<CascadeUpdateResult, CascadeError> {
    if let Some(ref s) = structure {
        validate_set_structure(session, s)?;
    }

    let mut result = CascadeUpdateResult::default();

    let structure_changed = match (&session.current_structure, &structure) {
        (Some(old), Some(new)) => old.structure_id != new.structure_id,
        (None, Some(_)) => true,
        (Some(_), None) => true,
        (None, None) => false,
    };

    if structure_changed && session.current_case.is_some() {
        session.current_case = None;
        result.case_cleared = true;
    }

    // Auto-set structure_type if not already set
    if let Some(ref s) = structure {
        if session.structure_type.is_none() {
            session.structure_type = Some(s.structure_type);
        }
    }

    session.current_structure = structure;
    result.structure_cleared = structure_changed && session.current_structure.is_none();
    Ok(result)
}

/// Set current case
pub fn set_case(
    session: &mut UnifiedSession,
    case: Option<CaseRef>,
) -> Result<CascadeUpdateResult, CascadeError> {
    if let Some(ref c) = case {
        validate_set_case(session, c)?;
    }

    let result = CascadeUpdateResult {
        case_cleared: session.current_case.is_some() && case.is_none(),
        ..Default::default()
    };
    session.current_case = case;
    Ok(result)
}

// =============================================================================
// DAG STATE UPDATES FROM CASCADE
// =============================================================================

/// Update DAG state when cascade changes
pub fn update_dag_from_cascade(session: &mut UnifiedSession) {
    // Set state flags based on current selections
    session
        .dag_state
        .set_flag("client.selected", session.client.is_some());
    session
        .dag_state
        .set_flag("structure.selected", session.current_structure.is_some());
    session
        .dag_state
        .set_flag("structure.exists", session.current_structure.is_some());
    session
        .dag_state
        .set_flag("case.selected", session.current_case.is_some());
    session
        .dag_state
        .set_flag("case.exists", session.current_case.is_some());
}

// =============================================================================
// PERSONA-BASED FILTERING
// =============================================================================

/// Filter verbs based on persona
///
/// Returns true if the verb is available for the given persona
pub fn verb_available_for_persona(verb_fqn: &str, persona: Persona) -> bool {
    // Domain-based filtering
    let domain = verb_fqn.split('.').next().unwrap_or("");

    match persona {
        Persona::Ops => {
            // Ops can use most verbs except admin-only
            !matches!(domain, "admin" | "system")
        }
        Persona::Kyc => {
            // KYC focuses on kyc, entity, ubo, document domains
            matches!(
                domain,
                "kyc" | "entity" | "ubo" | "document" | "case" | "session" | "view" | "structure"
            )
        }
        Persona::Trading => {
            // Trading focuses on trading, instruments, markets
            matches!(
                domain,
                "trading-profile"
                    | "instrument"
                    | "market"
                    | "custody"
                    | "isda"
                    | "session"
                    | "view"
                    | "mandate"
            )
        }
        Persona::Admin => {
            // Admin can use all verbs
            true
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_session() -> UnifiedSession {
        UnifiedSession::new()
    }

    fn make_client() -> ClientRef {
        ClientRef {
            client_id: Uuid::now_v7(),
            display_name: "Allianz".to_string(),
        }
    }

    fn make_structure(structure_type: StructureType) -> StructureRef {
        StructureRef {
            structure_id: Uuid::now_v7(),
            display_name: "Test Fund".to_string(),
            structure_type,
        }
    }

    fn make_case() -> CaseRef {
        CaseRef {
            case_id: Uuid::now_v7(),
            display_name: "KYC-2024-001".to_string(),
        }
    }

    #[test]
    fn test_derive_search_scope_empty() {
        let session = make_test_session();
        let scope = derive_search_scope(&session);

        assert!(scope.client_id.is_none());
        assert!(scope.structure_type.is_none());
        assert!(scope.structure_id.is_none());
        assert!(!scope.is_constrained());
    }

    #[test]
    fn test_derive_search_scope_with_client() {
        let mut session = make_test_session();
        session.client = Some(make_client());

        let scope = derive_search_scope(&session);

        assert!(scope.client_id.is_some());
        assert!(scope.is_constrained());
    }

    #[test]
    fn test_derive_search_scope_full() {
        let mut session = make_test_session();
        session.client = Some(make_client());
        session.structure_type = Some(StructureType::Pe);
        session.current_structure = Some(make_structure(StructureType::Pe));

        let scope = derive_search_scope(&session);

        assert!(scope.client_id.is_some());
        assert_eq!(scope.structure_type, Some(StructureType::Pe));
        assert!(scope.structure_id.is_some());
    }

    #[test]
    fn test_validate_structure_type_without_client() {
        let session = make_test_session();
        let result = validate_set_structure_type(&session, StructureType::Pe);

        assert!(matches!(
            result,
            Err(CascadeError::NoClientForStructureType)
        ));
    }

    #[test]
    fn test_validate_structure_without_client() {
        let session = make_test_session();
        let structure = make_structure(StructureType::Pe);
        let result = validate_set_structure(&session, &structure);

        assert!(matches!(result, Err(CascadeError::NoClientForStructure)));
    }

    #[test]
    fn test_validate_structure_type_mismatch() {
        let mut session = make_test_session();
        session.client = Some(make_client());
        session.structure_type = Some(StructureType::Pe);

        let structure = make_structure(StructureType::Sicav);
        let result = validate_set_structure(&session, &structure);

        assert!(matches!(
            result,
            Err(CascadeError::StructureTypeMismatch { .. })
        ));
    }

    #[test]
    fn test_validate_case_without_structure() {
        let mut session = make_test_session();
        session.client = Some(make_client());

        let case = make_case();
        let result = validate_set_case(&session, &case);

        assert!(matches!(result, Err(CascadeError::NoStructureForCase)));
    }

    #[test]
    fn test_set_client_clears_downstream() {
        let mut session = make_test_session();
        session.client = Some(make_client());
        session.structure_type = Some(StructureType::Pe);
        session.current_structure = Some(make_structure(StructureType::Pe));
        session.current_case = Some(make_case());

        let result = set_client(&mut session, Some(make_client()));

        assert!(result.structure_type_cleared);
        assert!(result.structure_cleared);
        assert!(result.case_cleared);
        assert!(session.structure_type.is_none());
        assert!(session.current_structure.is_none());
        assert!(session.current_case.is_none());
    }

    #[test]
    fn test_set_structure_type_clears_mismatched_structure() {
        let mut session = make_test_session();
        session.client = Some(make_client());
        session.current_structure = Some(make_structure(StructureType::Sicav));

        let result = set_structure_type(&mut session, Some(StructureType::Pe)).unwrap();

        assert!(result.structure_cleared);
        assert!(session.current_structure.is_none());
    }

    #[test]
    fn test_set_structure_auto_sets_type() {
        let mut session = make_test_session();
        session.client = Some(make_client());

        let structure = make_structure(StructureType::Hedge);
        set_structure(&mut session, Some(structure)).unwrap();

        assert_eq!(session.structure_type, Some(StructureType::Hedge));
    }

    #[test]
    fn test_update_dag_from_cascade() {
        let mut session = make_test_session();
        session.client = Some(make_client());
        session.current_structure = Some(make_structure(StructureType::Pe));

        update_dag_from_cascade(&mut session);

        assert!(session.dag_state.get_flag("client.selected"));
        assert!(session.dag_state.get_flag("structure.selected"));
        assert!(session.dag_state.get_flag("structure.exists"));
        assert!(!session.dag_state.get_flag("case.selected"));
    }

    #[test]
    fn test_verb_available_for_persona_ops() {
        assert!(verb_available_for_persona("cbu.create", Persona::Ops));
        assert!(verb_available_for_persona("entity.list", Persona::Ops));
        assert!(!verb_available_for_persona("admin.reset", Persona::Ops));
        assert!(!verb_available_for_persona("system.config", Persona::Ops));
    }

    #[test]
    fn test_verb_available_for_persona_kyc() {
        assert!(verb_available_for_persona("kyc.open-case", Persona::Kyc));
        assert!(verb_available_for_persona("entity.create", Persona::Kyc));
        assert!(verb_available_for_persona("ubo.discover", Persona::Kyc));
        assert!(!verb_available_for_persona(
            "trading-profile.create",
            Persona::Kyc
        ));
        assert!(!verb_available_for_persona("custody.link", Persona::Kyc));
    }

    #[test]
    fn test_verb_available_for_persona_trading() {
        assert!(verb_available_for_persona(
            "trading-profile.create",
            Persona::Trading
        ));
        assert!(verb_available_for_persona(
            "instrument.add",
            Persona::Trading
        ));
        assert!(verb_available_for_persona("custody.link", Persona::Trading));
        assert!(!verb_available_for_persona(
            "kyc.open-case",
            Persona::Trading
        ));
        assert!(!verb_available_for_persona(
            "ubo.discover",
            Persona::Trading
        ));
    }

    #[test]
    fn test_verb_available_for_persona_admin() {
        assert!(verb_available_for_persona("admin.reset", Persona::Admin));
        assert!(verb_available_for_persona("system.config", Persona::Admin));
        assert!(verb_available_for_persona("kyc.open-case", Persona::Admin));
        assert!(verb_available_for_persona(
            "trading-profile.create",
            Persona::Admin
        ));
    }
}
