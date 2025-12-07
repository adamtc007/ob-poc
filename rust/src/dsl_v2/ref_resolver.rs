//! Reference Type Resolver - Traits and types for DSL argument validation
//!
//! This module defines the interface for validating that DSL argument values
//! actually exist in the database. It's not just type checking - it's instance checking.
//!
//! Example:
//! - `:document-type "PASSPORT_GBR"` → must exist in document_types.type_code
//! - `:entity-id @company` → symbol must resolve to existing entities.entity_id
//! - `:jurisdiction "UK"` → must exist in master_jurisdictions.iso_code
//! - `:attribute-id "full_legal_name"` → must exist in attribute_registry.id
//!
//! The `GatewayRefResolver` in `gateway_resolver.rs` implements the `RefResolver` trait
//! using the EntityGateway gRPC service for all lookups.

use crate::dsl_v2::validation::{Diagnostic, RefType, SourceSpan, Suggestion};
use async_trait::async_trait;
use uuid::Uuid;

/// Result of resolving a reference
#[derive(Debug, Clone)]
pub enum ResolveResult {
    /// Found - includes the resolved ID and display name
    Found { id: Uuid, display: String },

    /// Found by code/string key (for text PKs like attribute_registry.id)
    FoundByCode {
        code: String,
        uuid: Option<Uuid>,
        display: String,
    },

    /// Not found - includes fuzzy match suggestions
    NotFound { suggestions: Vec<SuggestedMatch> },
}

/// A suggested match from fuzzy matching
#[derive(Debug, Clone)]
pub struct SuggestedMatch {
    pub value: String,
    pub display: String,
    pub score: f32, // 0.0 - 1.0, higher is better match
}

impl SuggestedMatch {
    pub fn into_suggestion(self, message: &str) -> Suggestion {
        Suggestion::new(message, self.value, self.score)
    }
}

// =============================================================================
// RESOLVER TRAIT
// =============================================================================

/// Trait for reference resolution - implemented by GatewayRefResolver
#[async_trait]
pub trait RefResolver: Send + Sync {
    /// Resolve a reference by type
    async fn resolve(&mut self, ref_type: RefType, value: &str) -> Result<ResolveResult, String>;

    /// Create a diagnostic for a failed resolution
    fn diagnostic_for_failure(
        &self,
        ref_type: RefType,
        value: &str,
        span: SourceSpan,
        result: &ResolveResult,
    ) -> Diagnostic;

    /// Clear any internal cache (optional, default no-op)
    fn clear_cache(&mut self) {}
}

// =============================================================================
// ARG TYPE MAPPING
// =============================================================================

/// Map DSL argument keys to their expected reference types
/// Returns None if the argument doesn't need DB validation
pub fn arg_to_ref_type(verb: &str, arg_key: &str) -> Option<RefType> {
    match arg_key {
        // Document type references
        ":document-type" | ":doc-type" => Some(RefType::DocumentType),

        // Jurisdiction references
        ":jurisdiction" => Some(RefType::Jurisdiction),

        // Role references
        ":role" => Some(RefType::Role),

        // Entity type references
        ":entity-type" | ":type" if verb.starts_with("entity.") => Some(RefType::EntityType),

        // Attribute references
        ":attribute-id" | ":attribute" => Some(RefType::AttributeId),

        // ID references (these resolve symbols, not literals)
        ":cbu-id" => Some(RefType::Cbu),
        ":entity-id" => Some(RefType::Entity),
        ":document-id" | ":doc-id" => Some(RefType::Document),

        // Screening type
        ":screening-type" | ":check-type" => Some(RefType::ScreeningType),

        // Not a DB reference
        _ => None,
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arg_to_ref_type() {
        assert_eq!(
            arg_to_ref_type("document.catalog", ":document-type"),
            Some(RefType::DocumentType)
        );
        assert_eq!(
            arg_to_ref_type("cbu.create", ":jurisdiction"),
            Some(RefType::Jurisdiction)
        );
        assert_eq!(
            arg_to_ref_type("cbu.assign-role", ":role"),
            Some(RefType::Role)
        );
        assert_eq!(
            arg_to_ref_type("entity.create", ":type"),
            Some(RefType::EntityType)
        );
        assert_eq!(arg_to_ref_type("cbu.create", ":name"), None); // Not a ref type
    }
}
