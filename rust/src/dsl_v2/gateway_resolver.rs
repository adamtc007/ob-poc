//! Gateway-backed Reference Resolver
//!
//! This module provides the same interface as `ref_resolver.rs` but uses the
//! EntityGateway gRPC service instead of direct SQL queries. This ensures
//! that validation uses the exact same search logic as LSP autocomplete.
//!
//! ## Benefits
//! - Single source of truth for entity lookups (EntityGateway)
//! - Consistent fuzzy matching behavior between LSP and validator
//! - No reconciliation risk between different search implementations

use crate::dsl_v2::ref_resolver::{RefResolver, ResolveResult, SuggestedMatch};
use crate::dsl_v2::validation::{
    Diagnostic, DiagnosticCode, RefType, Severity, SourceSpan, Suggestion,
};
use async_trait::async_trait;
use entity_gateway::proto::ob::gateway::v1::{
    entity_gateway_client::EntityGatewayClient, GetEntityConfigRequest, GetEntityConfigResponse,
    SearchMode, SearchRequest,
};
use tonic::transport::Channel;
use uuid::Uuid;

/// Default EntityGateway address
pub const DEFAULT_GATEWAY_ADDR: &str = "http://[::1]:50051";

/// Get gateway address from environment or use default
pub fn gateway_addr() -> String {
    std::env::var("ENTITY_GATEWAY_URL").unwrap_or_else(|_| DEFAULT_GATEWAY_ADDR.to_string())
}

/// Gateway-backed reference resolver
///
/// Uses EntityGateway gRPC service for all lookups, ensuring consistent
/// behavior with LSP autocomplete.
#[derive(Clone)]
pub struct GatewayRefResolver {
    client: EntityGatewayClient<Channel>,
}

impl GatewayRefResolver {
    /// Get key strength for ordering (lower = stronger = try first)
    ///
    /// Policy lives HERE in resolver, not in config crate.
    /// Config should be pure data, resolver owns resolution strategy.
    ///
    /// Strength tiers:
    /// - Tier 1 (0-9): Exact identifiers - O(1) lookup
    /// - Tier 2 (10-19): Composite with discriminators - narrowed search
    /// - Tier 3 (20+): Fuzzy name search - broad, slower
    pub fn key_strength(column: &str, has_discriminators: bool) -> u8 {
        match column {
            // Tier 1: Exact identifiers
            "id" => 0,
            "lei" => 1,
            "registration_number" | "reg_number" => 2,
            "bic" => 3,
            "isin" => 4,
            "code" => 5,
            // Tier 2: Composite with discriminators
            _ if has_discriminators => 10,
            // Tier 3: Fuzzy name-based
            "search_name" | "name" => 20,
            "alias" => 25,
            _ => 30,
        }
    }
}

impl GatewayRefResolver {
    /// Create from an existing client (preferred - reuses connection)
    ///
    /// The `EntityGatewayClient<Channel>` is cheap to clone since tonic's
    /// Channel uses Arc internally. This enables connection reuse across
    /// multiple resolve calls.
    pub fn new(client: EntityGatewayClient<Channel>) -> Self {
        Self { client }
    }

    /// Get a clone of the underlying client for sharing
    ///
    /// This is cheap because tonic Channel is Arc-based.
    pub fn client(&self) -> EntityGatewayClient<Channel> {
        self.client.clone()
    }

    /// Connect to the EntityGateway service
    pub async fn connect(addr: &str) -> Result<Self, String> {
        let client = EntityGatewayClient::connect(addr.to_string())
            .await
            .map_err(|e| format!("Failed to connect to EntityGateway at {}: {}", addr, e))?;
        Ok(Self { client })
    }

    /// Connect using default address from environment
    pub async fn connect_default() -> Result<Self, String> {
        Self::connect(&gateway_addr()).await
    }

    /// Resolve a reference by type
    pub async fn resolve(
        &mut self,
        ref_type: RefType,
        value: &str,
    ) -> Result<ResolveResult, String> {
        let nickname = ref_type_to_nickname(ref_type);

        // For UUID-based lookups, we search by ID directly if value looks like a UUID
        // Otherwise, search by name (default search key)
        let (search_value, search_key) =
            if is_uuid_lookup(ref_type) && Uuid::parse_str(value).is_ok() {
                // Value is a valid UUID - search by ID
                (value.to_string(), Some("id".to_string()))
            } else {
                // Value is a name - search by default key (name)
                (value.to_string(), None)
            };

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![search_value.clone()],
            search_key: search_key.clone(),
            mode: SearchMode::Exact as i32,
            limit: Some(5), // Get suggestions if not found
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        };

        let response = self
            .client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        let matches: Vec<_> = response.into_inner().matches;

        if matches.is_empty() {
            // No exact matches - try fuzzy search for suggestions
            let fuzzy_request = SearchRequest {
                nickname: nickname.to_string(),
                values: vec![search_value.clone()],
                search_key: search_key.clone(),
                mode: SearchMode::Fuzzy as i32,
                limit: Some(10),
                discriminators: std::collections::HashMap::new(),
                tenant_id: None,
                cbu_id: None,
            };

            let fuzzy_response = self
                .client
                .search(fuzzy_request)
                .await
                .map_err(|e| format!("EntityGateway fuzzy search failed: {}", e))?;

            let fuzzy_matches = fuzzy_response.into_inner().matches;

            if fuzzy_matches.is_empty() {
                return Ok(ResolveResult::NotFound {
                    suggestions: vec![],
                });
            }

            // Return fuzzy matches as suggestions
            let suggestions: Vec<SuggestedMatch> = fuzzy_matches
                .into_iter()
                .map(|m| SuggestedMatch {
                    value: m.token,
                    display: m.display,
                    score: m.score,
                })
                .collect();

            return Ok(ResolveResult::NotFound { suggestions });
        }

        // Check for exact match (case-insensitive)
        let value_upper = value.to_uppercase();
        for m in &matches {
            // Check if token or display matches exactly
            if m.token.to_uppercase() == value_upper || m.display.to_uppercase() == value_upper {
                // Parse token as UUID if applicable
                if let Ok(uuid) = Uuid::parse_str(&m.token) {
                    return Ok(ResolveResult::Found {
                        id: uuid,
                        display: m.display.clone(),
                    });
                } else {
                    // Token is a code (e.g., jurisdiction code, role name)
                    return Ok(ResolveResult::FoundByCode {
                        code: m.token.clone(),
                        uuid: None,
                        display: m.display.clone(),
                    });
                }
            }
        }

        // No exact match - return suggestions from fuzzy matches
        let suggestions: Vec<SuggestedMatch> = matches
            .into_iter()
            .map(|m| SuggestedMatch {
                value: m.token,
                display: m.display,
                score: m.score,
            })
            .collect();

        Ok(ResolveResult::NotFound { suggestions })
    }

    /// Batch resolve multiple values of the same type in one gRPC call
    ///
    /// This is significantly more efficient than calling `resolve` in a loop:
    /// - 30 EntityRefs with 5 types = 5 gRPC calls instead of 30
    /// - ~6x performance improvement
    pub async fn batch_resolve(
        &mut self,
        ref_type: RefType,
        values: &[String],
    ) -> Result<std::collections::HashMap<String, ResolveResult>, String> {
        use std::collections::HashMap;

        if values.is_empty() {
            return Ok(HashMap::new());
        }

        let nickname = ref_type_to_nickname(ref_type);

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: values.to_vec(),
            search_key: None,
            mode: SearchMode::Exact as i32,
            limit: None, // Return all matches
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        };

        let response = self
            .client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway batch search failed: {}", e))?;

        // Build result map: input value → ResolveResult
        let mut results = HashMap::new();
        let matches = response.into_inner().matches;

        // Index matches by their input value for quick lookup (case-insensitive)
        let match_index: HashMap<String, _> = matches
            .into_iter()
            .map(|m| (m.input.to_uppercase(), m))
            .collect();

        for value in values {
            let value_upper = value.to_uppercase();
            if let Some(m) = match_index.get(&value_upper) {
                if let Ok(uuid) = Uuid::parse_str(&m.token) {
                    results.insert(
                        value.clone(),
                        ResolveResult::Found {
                            id: uuid,
                            display: m.display.clone(),
                        },
                    );
                } else {
                    results.insert(
                        value.clone(),
                        ResolveResult::FoundByCode {
                            code: m.token.clone(),
                            uuid: None,
                            display: m.display.clone(),
                        },
                    );
                }
            } else {
                results.insert(
                    value.clone(),
                    ResolveResult::NotFound {
                        suggestions: vec![],
                    },
                );
            }
        }

        Ok(results)
    }

    /// Resolve a reference with discriminators for disambiguation
    ///
    /// This is the full-featured resolution method that uses the complete
    /// search schema from verb YAML. Discriminators are extracted from
    /// sibling arguments by the semantic validator.
    ///
    /// Example: For a person lookup with search_key "(search_name (date_of_birth :selectivity 0.95))",
    /// if the verb call has `:dob "1985-03-15"`, that value is passed as a discriminator
    /// to boost matching candidates with that DOB.
    pub async fn resolve_with_discriminators(
        &mut self,
        ref_type: RefType,
        value: &str,
        discriminators: std::collections::HashMap<String, String>,
    ) -> Result<ResolveResult, String> {
        let nickname = ref_type_to_nickname(ref_type);

        // For UUID-based lookups, we search by ID directly if value looks like a UUID
        let (search_value, search_key) =
            if is_uuid_lookup(ref_type) && Uuid::parse_str(value).is_ok() {
                (value.to_string(), Some("id".to_string()))
            } else {
                (value.to_string(), None)
            };

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![search_value.clone()],
            search_key: search_key.clone(),
            mode: SearchMode::Exact as i32,
            limit: Some(5),
            discriminators: discriminators.clone(),
            tenant_id: None,
            cbu_id: None,
        };

        let response = self
            .client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        let matches: Vec<_> = response.into_inner().matches;

        if matches.is_empty() {
            // No exact matches - try fuzzy search with discriminators for suggestions
            let fuzzy_request = SearchRequest {
                nickname: nickname.to_string(),
                values: vec![search_value.clone()],
                search_key: search_key.clone(),
                mode: SearchMode::Fuzzy as i32,
                limit: Some(10),
                discriminators,
                tenant_id: None,
                cbu_id: None,
            };

            let fuzzy_response = self
                .client
                .search(fuzzy_request)
                .await
                .map_err(|e| format!("EntityGateway fuzzy search failed: {}", e))?;

            let fuzzy_matches = fuzzy_response.into_inner().matches;

            if fuzzy_matches.is_empty() {
                return Ok(ResolveResult::NotFound {
                    suggestions: vec![],
                });
            }

            let suggestions: Vec<SuggestedMatch> = fuzzy_matches
                .into_iter()
                .map(|m| SuggestedMatch {
                    value: m.token,
                    display: m.display,
                    score: m.score,
                })
                .collect();

            return Ok(ResolveResult::NotFound { suggestions });
        }

        // Check for exact match (case-insensitive)
        let value_upper = value.to_uppercase();
        for m in &matches {
            if m.token.to_uppercase() == value_upper || m.display.to_uppercase() == value_upper {
                if let Ok(uuid) = Uuid::parse_str(&m.token) {
                    return Ok(ResolveResult::Found {
                        id: uuid,
                        display: m.display.clone(),
                    });
                } else {
                    return Ok(ResolveResult::FoundByCode {
                        code: m.token.clone(),
                        uuid: None,
                        display: m.display.clone(),
                    });
                }
            }
        }

        // No exact match - return suggestions
        let suggestions: Vec<SuggestedMatch> = matches
            .into_iter()
            .map(|m| SuggestedMatch {
                value: m.token,
                display: m.display,
                score: m.score,
            })
            .collect();

        Ok(ResolveResult::NotFound { suggestions })
    }

    /// Get entity configuration for resolution UI
    ///
    /// Returns search keys, discriminators, and resolution mode for an entity type.
    /// Used by the resolution modal to render entity-specific search fields.
    pub async fn get_entity_config(
        &mut self,
        nickname: &str,
    ) -> Result<GetEntityConfigResponse, String> {
        let request = GetEntityConfigRequest {
            nickname: nickname.to_string(),
        };

        let response = self
            .client
            .get_entity_config(request)
            .await
            .map_err(|e| format!("EntityGateway get_entity_config failed: {}", e))?;

        Ok(response.into_inner())
    }

    /// Get entity configuration by RefType
    pub async fn get_entity_config_by_ref_type(
        &mut self,
        ref_type: RefType,
    ) -> Result<GetEntityConfigResponse, String> {
        let nickname = ref_type_to_nickname(ref_type);
        self.get_entity_config(nickname).await
    }

    /// Search with fuzzy matching (for autocomplete-like scenarios)
    pub async fn search_fuzzy(
        &mut self,
        ref_type: RefType,
        prefix: &str,
        limit: usize,
    ) -> Result<Vec<SuggestedMatch>, String> {
        let nickname = ref_type_to_nickname(ref_type);

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![prefix.to_string()],
            search_key: None,
            mode: SearchMode::Fuzzy as i32,
            limit: Some(limit as i32),
            discriminators: std::collections::HashMap::new(),
            tenant_id: None,
            cbu_id: None,
        };

        let response = self
            .client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        let matches = response
            .into_inner()
            .matches
            .into_iter()
            .map(|m| SuggestedMatch {
                value: m.token,
                display: m.display,
                score: m.score,
            })
            .collect();

        Ok(matches)
    }

    /// Multi-key search with filter support
    ///
    /// This method supports:
    /// - Multiple search keys (name, jurisdiction, etc.)
    /// - Filter values to narrow results
    /// - Discriminators for scoring boost
    ///
    /// Returns (matches, total_count, was_filtered)
    pub async fn search_multi_key(
        &mut self,
        ref_type: RefType,
        query: &str,
        search_key: Option<&str>,
        filters: &std::collections::HashMap<String, String>,
        discriminators: &std::collections::HashMap<String, String>,
        limit: usize,
    ) -> Result<(Vec<SuggestedMatch>, usize, bool), String> {
        let nickname = ref_type_to_nickname(ref_type);

        // Build discriminators map with both filters and discriminators
        // Filters act as hard constraints, discriminators as soft scoring boost
        let mut all_discriminators = discriminators.clone();
        for (k, v) in filters {
            all_discriminators.insert(k.clone(), v.clone());
        }

        let request = SearchRequest {
            nickname: nickname.to_string(),
            values: vec![query.to_string()],
            search_key: search_key.map(|s| s.to_string()),
            mode: SearchMode::Fuzzy as i32,
            limit: Some(limit as i32),
            discriminators: all_discriminators,
            tenant_id: None,
            cbu_id: None,
        };

        let response = self
            .client
            .search(request)
            .await
            .map_err(|e| format!("EntityGateway search failed: {}", e))?;

        let inner = response.into_inner();
        let total = inner.matches.len();
        let was_filtered = !filters.is_empty();

        let matches = inner
            .matches
            .into_iter()
            .map(|m| SuggestedMatch {
                value: m.token,
                display: m.display,
                score: m.score,
            })
            .collect();

        Ok((matches, total, was_filtered))
    }

    /// Create a diagnostic for a failed resolution
    pub fn diagnostic_for_failure(
        &self,
        ref_type: RefType,
        value: &str,
        span: SourceSpan,
        result: &ResolveResult,
    ) -> Diagnostic {
        let ResolveResult::NotFound { suggestions } = result else {
            // This is a programming error - caller should only pass NotFound results
            debug_assert!(
                false,
                "diagnostic_for_failure called with non-failure result"
            );
            // Return a generic error diagnostic as fallback
            return Diagnostic {
                severity: Severity::Error,
                span,
                code: DiagnosticCode::InvalidValue,
                message: format!("Resolution failed for '{}' (unexpected result type)", value),
                suggestions: Vec::new(),
            };
        };

        let (code, type_name) = match ref_type {
            RefType::DocumentType => (DiagnosticCode::UnknownDocumentType, "document type"),
            RefType::Jurisdiction => (DiagnosticCode::UnknownJurisdiction, "jurisdiction"),
            RefType::Role => (DiagnosticCode::UnknownRole, "role"),
            RefType::EntityType => (DiagnosticCode::UnknownEntityType, "entity type"),
            RefType::AttributeId => (DiagnosticCode::UnknownAttributeId, "attribute"),
            RefType::Cbu => (DiagnosticCode::CbuNotFound, "CBU"),
            RefType::Entity => (DiagnosticCode::EntityNotFound, "entity"),
            RefType::Document => (DiagnosticCode::DocumentNotFound, "document"),
            RefType::ScreeningType => (DiagnosticCode::InvalidValue, "screening type"),
            RefType::Product => (DiagnosticCode::InvalidValue, "product"),
            RefType::Service => (DiagnosticCode::InvalidValue, "service"),
            RefType::Currency => (DiagnosticCode::InvalidValue, "currency"),
            RefType::ClientType => (DiagnosticCode::InvalidValue, "client type"),
        };

        Diagnostic {
            severity: Severity::Error,
            span,
            code,
            message: format!("unknown {} '{}'", type_name, value),
            suggestions: suggestions
                .iter()
                .take(3)
                .map(|s| Suggestion::new("did you mean", s.value.clone(), s.score))
                .collect(),
        }
    }
}

// Implement the RefResolver trait for GatewayRefResolver
#[async_trait]
impl RefResolver for GatewayRefResolver {
    async fn resolve(&mut self, ref_type: RefType, value: &str) -> Result<ResolveResult, String> {
        GatewayRefResolver::resolve(self, ref_type, value).await
    }

    fn diagnostic_for_failure(
        &self,
        ref_type: RefType,
        value: &str,
        span: SourceSpan,
        result: &ResolveResult,
    ) -> Diagnostic {
        GatewayRefResolver::diagnostic_for_failure(self, ref_type, value, span, result)
    }

    // GatewayRefResolver doesn't need caching - EntityGateway handles it
    fn clear_cache(&mut self) {}

    fn as_gateway_resolver(&mut self) -> Option<&mut GatewayRefResolver> {
        Some(self)
    }
}

/// Map RefType to EntityGateway nickname
/// NOTE: Nicknames are UPPERCASE to match entity_index.yaml nickname fields
fn ref_type_to_nickname(ref_type: RefType) -> &'static str {
    match ref_type {
        RefType::DocumentType => "DOCUMENT_TYPE",
        RefType::Jurisdiction => "JURISDICTION",
        RefType::Role => "ROLE",
        RefType::EntityType => "ENTITY_TYPE",
        RefType::AttributeId => "ATTRIBUTE",
        RefType::Cbu => "CBU",
        RefType::Entity => "ENTITY",
        RefType::Document => "DOCUMENT",
        RefType::ScreeningType => "SCREENING_TYPE",
        RefType::Product => "PRODUCT",
        RefType::Service => "SERVICE",
        RefType::Currency => "CURRENCY",
        RefType::ClientType => "CLIENT_TYPE",
    }
}

/// Check if this ref type uses UUID-based lookup
fn is_uuid_lookup(ref_type: RefType) -> bool {
    matches!(ref_type, RefType::Cbu | RefType::Entity | RefType::Document)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ref_type_to_nickname() {
        assert_eq!(ref_type_to_nickname(RefType::DocumentType), "DOCUMENT_TYPE");
        assert_eq!(ref_type_to_nickname(RefType::Jurisdiction), "JURISDICTION");
        assert_eq!(ref_type_to_nickname(RefType::Role), "ROLE");
        assert_eq!(ref_type_to_nickname(RefType::EntityType), "ENTITY_TYPE");
        assert_eq!(ref_type_to_nickname(RefType::AttributeId), "ATTRIBUTE");
        assert_eq!(ref_type_to_nickname(RefType::Cbu), "CBU");
        assert_eq!(ref_type_to_nickname(RefType::Entity), "ENTITY");
        assert_eq!(ref_type_to_nickname(RefType::Document), "DOCUMENT");
        assert_eq!(
            ref_type_to_nickname(RefType::ScreeningType),
            "SCREENING_TYPE"
        );
        assert_eq!(ref_type_to_nickname(RefType::Product), "PRODUCT");
        assert_eq!(ref_type_to_nickname(RefType::Service), "SERVICE");
        assert_eq!(ref_type_to_nickname(RefType::Currency), "CURRENCY");
        assert_eq!(ref_type_to_nickname(RefType::ClientType), "CLIENT_TYPE");
    }

    #[test]
    fn test_is_uuid_lookup() {
        assert!(is_uuid_lookup(RefType::Cbu));
        assert!(is_uuid_lookup(RefType::Entity));
        assert!(is_uuid_lookup(RefType::Document));
        assert!(!is_uuid_lookup(RefType::Role));
        assert!(!is_uuid_lookup(RefType::Jurisdiction));
    }

    #[test]
    fn test_gateway_addr_default() {
        // Without ENTITY_GATEWAY_URL set, should return default
        std::env::remove_var("ENTITY_GATEWAY_URL");
        assert_eq!(gateway_addr(), DEFAULT_GATEWAY_ADDR);
    }
}
