//! Extension trait for ExpansionContext integration with main crate session types
//!
//! This provides the `from_repl_session()` and `from_session_context()` methods
//! that were moved out of the ob-templates crate to avoid circular dependencies.

use ob_templates::ExpansionContext;

use crate::api::session::SessionContext;
use crate::dsl_v2::repl_session::ReplSession;

/// Extension trait for creating ExpansionContext from main crate session types
pub trait ExpansionContextExt {
    /// Create context from a ReplSession
    ///
    /// Automatically populates:
    /// - All executed bindings (name → UUID)
    /// - Entity types for each binding
    /// - Extracts current_cbu from bindings named "cbu" or "fund"
    /// - Extracts current_case from bindings named "case" or "kyc_case"
    fn from_repl_session(session: &ReplSession) -> Self;

    /// Create context from an AgentSession's SessionContext
    ///
    /// Uses the session's bound entities and active CBU/case.
    fn from_session_context(session_ctx: &SessionContext) -> Self;
}

impl ExpansionContextExt for ExpansionContext {
    fn from_repl_session(session: &ReplSession) -> Self {
        let mut ctx = Self::new();

        // Populate bindings from session
        for name in session.binding_names() {
            if let Some(pk) = session.get_binding(name) {
                ctx.bindings.insert(name.to_string(), pk.to_string());

                // Also track entity type
                if let Some(ty) = session.get_binding_type(name) {
                    ctx.binding_types.insert(name.to_string(), ty.to_string());
                }

                // Extract current_cbu from common binding names
                match name {
                    "cbu" | "fund" | "active_cbu" => {
                        if ctx.current_cbu.is_none() {
                            ctx.current_cbu = Some(pk);
                        }
                    }
                    "case" | "kyc_case" | "active_case" => {
                        if ctx.current_case.is_none() {
                            ctx.current_case = Some(pk);
                        }
                    }
                    _ => {}
                }
            }
        }

        ctx
    }

    fn from_session_context(session_ctx: &SessionContext) -> Self {
        let mut ctx = Self::new();

        // Set active CBU if present
        if let Some(ref active_cbu) = session_ctx.active_cbu {
            ctx.current_cbu = Some(active_cbu.id);
            ctx.bindings
                .insert("cbu".to_string(), active_cbu.id.to_string());
            ctx.binding_types
                .insert("cbu".to_string(), "cbu".to_string());
        }

        // Populate from bindings
        for (name, bound) in &session_ctx.bindings {
            ctx.bindings.insert(name.clone(), bound.id.to_string());
            ctx.binding_types
                .insert(name.clone(), bound.entity_type.clone());
        }

        // Also include named_refs for backward compat
        for (name, pk) in &session_ctx.named_refs {
            if !ctx.bindings.contains_key(name) {
                ctx.bindings.insert(name.clone(), pk.to_string());
            }
        }

        // Set current_case from primary keys if available
        if let Some(case_id) = session_ctx.primary_keys.kyc_case_id {
            ctx.current_case = Some(case_id);
        }

        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn test_from_repl_session_extracts_cbu() {
        use crate::dsl_v2::ast::Program;

        let mut session = ReplSession::new();
        let cbu_id = Uuid::new_v4();

        // Simulate binding a CBU via append_executed
        let program = Program { statements: vec![] };
        let mut bindings = HashMap::new();
        bindings.insert("cbu".to_string(), cbu_id);
        let mut types = HashMap::new();
        types.insert("cbu".to_string(), "cbu".to_string());

        session.append_executed(program, bindings, types);

        let ctx = ExpansionContext::from_repl_session(&session);

        assert_eq!(ctx.current_cbu, Some(cbu_id));
        assert_eq!(ctx.bindings.get("cbu"), Some(&cbu_id.to_string()));
        assert_eq!(ctx.binding_types.get("cbu"), Some(&"cbu".to_string()));
    }

    #[test]
    fn test_from_session_context_extracts_active_cbu() {
        use crate::api::session::BoundEntity;

        let mut session_ctx = SessionContext::default();
        let cbu_id = Uuid::new_v4();

        session_ctx.active_cbu = Some(BoundEntity {
            id: cbu_id,
            display_name: "Test CBU".to_string(),
            entity_type: "cbu".to_string(),
        });

        let ctx = ExpansionContext::from_session_context(&session_ctx);

        assert_eq!(ctx.current_cbu, Some(cbu_id));
        assert_eq!(ctx.bindings.get("cbu"), Some(&cbu_id.to_string()));
    }
}
