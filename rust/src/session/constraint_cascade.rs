//! Constraint Cascade Logic
//!
//! Derives search scopes from session context.
//! The cascade flows: client → structure_type → current_structure → case
//!
//! Example:
//! - Client "Allianz" selected → entity search narrows from 10,000 to ~500
//! - Structure type "PE" selected → narrows to ~50
//! - Current structure selected → narrows to ~5-10 related entities

use super::unified::{SearchScope, UnifiedSession};

// =============================================================================
// SEARCH SCOPE DERIVATION
// =============================================================================

/// Derive the current search scope from session state
///
/// The search scope is used to narrow entity queries based on the
/// constraint cascade. Each level in the cascade further constrains
/// the search space.
pub(crate) fn derive_search_scope(session: &UnifiedSession) -> SearchScope {
    SearchScope {
        client_id: session.client.as_ref().map(|c| c.client_id),
        structure_type: session.structure_type,
        structure_id: session.current_structure.as_ref().map(|s| s.structure_id),
    }
}

// =============================================================================
// DAG STATE UPDATES FROM CASCADE
// =============================================================================

/// Update DAG state when cascade changes
pub(crate) fn update_dag_from_cascade(session: &mut UnifiedSession) {
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
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::unified::{ClientRef, StructureRef, StructureType};
    use uuid::Uuid;

    fn make_test_session() -> UnifiedSession {
        UnifiedSession::new()
    }

    fn make_client() -> ClientRef {
        ClientRef {
            client_id: Uuid::new_v4(),
            display_name: "Allianz".to_string(),
        }
    }

    fn make_structure(structure_type: StructureType) -> StructureRef {
        StructureRef {
            structure_id: Uuid::new_v4(),
            display_name: "Test Fund".to_string(),
            structure_type,
        }
    }

    #[test]
    fn test_derive_search_scope_empty() {
        let session = make_test_session();
        let scope = derive_search_scope(&session);

        assert!(scope.client_id.is_none());
        assert!(scope.structure_type.is_none());
        assert!(scope.structure_id.is_none());
    }

    #[test]
    fn test_derive_search_scope_with_client() {
        let mut session = make_test_session();
        session.client = Some(make_client());

        let scope = derive_search_scope(&session);

        assert!(scope.client_id.is_some());
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
    fn test_update_dag_from_cascade() {
        let mut session = make_test_session();
        session.client = Some(make_client());
        session.current_structure = Some(make_structure(StructureType::Pe));

        update_dag_from_cascade(&mut session);

        assert_eq!(session.dag_state.state_flags.get("client.selected"), Some(&true));
        assert_eq!(session.dag_state.state_flags.get("structure.selected"), Some(&true));
        assert_eq!(session.dag_state.state_flags.get("structure.exists"), Some(&true));
        assert_eq!(session.dag_state.state_flags.get("case.selected"), Some(&false));
    }
}
