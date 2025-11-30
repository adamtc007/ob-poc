//! Reference Type Resolver - DB-connected validation for DSL arguments
//!
//! This module validates that DSL argument values actually exist in the database.
//! It's not just type checking - it's instance checking.
//!
//! Example:
//! - `:document-type "PASSPORT_GBR"` → must exist in document_types.type_code
//! - `:entity-id @company` → symbol must resolve to existing entities.entity_id
//! - `:jurisdiction "UK"` → must exist in master_jurisdictions.iso_code
//! - `:attribute-id "full_legal_name"` → must exist in attribute_registry.id

use crate::dsl_v2::validation::{
    Diagnostic, DiagnosticCode, RefType, Severity, SourceSpan, Suggestion,
};
use sqlx::PgPool;
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

/// Reference resolver with DB connection
pub struct RefTypeResolver {
    pool: PgPool,
    /// Cache for repeated lookups (cleared per validation batch)
    cache: RefResolverCache,
}

/// Cache to avoid repeated DB lookups within a validation batch
#[derive(Default)]
struct RefResolverCache {
    document_types: Option<Vec<CachedRef>>,
    jurisdictions: Option<Vec<CachedRef>>,
    roles: Option<Vec<CachedRef>>,
    entity_types: Option<Vec<CachedRef>>,
    attributes: Option<Vec<CachedRef>>,
}

#[derive(Debug, Clone)]
struct CachedRef {
    code: String,
    uuid: Option<Uuid>,
    display: String,
}

impl RefTypeResolver {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            cache: RefResolverCache::default(),
        }
    }

    /// Clear cache (call between validation batches)
    pub fn clear_cache(&mut self) {
        self.cache = RefResolverCache::default();
    }

    /// Resolve a reference by type
    pub async fn resolve(
        &mut self,
        ref_type: RefType,
        value: &str,
    ) -> Result<ResolveResult, String> {
        match ref_type {
            RefType::DocumentType => self.resolve_document_type(value).await,
            RefType::Jurisdiction => self.resolve_jurisdiction(value).await,
            RefType::Role => self.resolve_role(value).await,
            RefType::EntityType => self.resolve_entity_type(value).await,
            RefType::AttributeId => self.resolve_attribute(value).await,
            RefType::Cbu => self.resolve_cbu(value).await,
            RefType::Entity => self.resolve_entity(value).await,
            RefType::Document => self.resolve_document(value).await,
            RefType::ScreeningType => self.resolve_screening_type(value).await,
        }
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
            panic!("diagnostic_for_failure called with non-failure result");
        };

        let (code, table_name) = match ref_type {
            RefType::DocumentType => (DiagnosticCode::UnknownDocumentType, "document_types"),
            RefType::Jurisdiction => (DiagnosticCode::UnknownJurisdiction, "master_jurisdictions"),
            RefType::Role => (DiagnosticCode::UnknownRole, "roles"),
            RefType::EntityType => (DiagnosticCode::UnknownEntityType, "entity_types"),
            RefType::AttributeId => (DiagnosticCode::UnknownAttributeId, "attribute_registry"),
            RefType::Cbu => (DiagnosticCode::CbuNotFound, "cbus"),
            RefType::Entity => (DiagnosticCode::EntityNotFound, "entities"),
            RefType::Document => (DiagnosticCode::DocumentNotFound, "document_catalog"),
            RefType::ScreeningType => (DiagnosticCode::InvalidValue, "screening_types"),
        };

        Diagnostic {
            severity: Severity::Error,
            span,
            code,
            message: format!(
                "unknown {} '{}' - not found in {}",
                ref_type_name(ref_type),
                value,
                table_name
            ),
            suggestions: suggestions
                .iter()
                .take(3) // Top 3 suggestions
                .map(|s| s.clone().into_suggestion("did you mean"))
                .collect(),
        }
    }

    // =========================================================================
    // LOOKUP METHODS
    // =========================================================================

    async fn resolve_document_type(&mut self, code: &str) -> Result<ResolveResult, String> {
        // Load cache if needed
        if self.cache.document_types.is_none() {
            let rows = sqlx::query!(
                r#"SELECT type_id, type_code, display_name
                   FROM "ob-poc".document_types
                   ORDER BY type_code"#
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("DB error loading document_types: {}", e))?;

            self.cache.document_types = Some(
                rows.into_iter()
                    .map(|r| CachedRef {
                        code: r.type_code,
                        uuid: Some(r.type_id),
                        display: r.display_name,
                    })
                    .collect(),
            );
        }

        self.lookup_in_cache(&self.cache.document_types, code)
    }

    async fn resolve_jurisdiction(&mut self, code: &str) -> Result<ResolveResult, String> {
        if self.cache.jurisdictions.is_none() {
            let rows = sqlx::query!(
                r#"SELECT jurisdiction_code, jurisdiction_name
                   FROM "ob-poc".master_jurisdictions
                   ORDER BY jurisdiction_code"#
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("DB error loading jurisdictions: {}", e))?;

            self.cache.jurisdictions = Some(
                rows.into_iter()
                    .map(|r| CachedRef {
                        code: r.jurisdiction_code,
                        uuid: None, // jurisdiction_code is text PK
                        display: r.jurisdiction_name,
                    })
                    .collect(),
            );
        }

        self.lookup_in_cache(&self.cache.jurisdictions, code)
    }

    async fn resolve_role(&mut self, code: &str) -> Result<ResolveResult, String> {
        if self.cache.roles.is_none() {
            let rows = sqlx::query!(
                r#"SELECT role_id, name
                   FROM "ob-poc".roles
                   ORDER BY name"#
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("DB error loading roles: {}", e))?;

            self.cache.roles = Some(
                rows.into_iter()
                    .map(|r| CachedRef {
                        code: r.name.clone(),
                        uuid: Some(r.role_id),
                        display: r.name,
                    })
                    .collect(),
            );
        }

        self.lookup_in_cache(&self.cache.roles, code)
    }

    async fn resolve_entity_type(&mut self, code: &str) -> Result<ResolveResult, String> {
        if self.cache.entity_types.is_none() {
            let rows = sqlx::query!(
                r#"SELECT entity_type_id, name
                   FROM "ob-poc".entity_types
                   ORDER BY name"#
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("DB error loading entity_types: {}", e))?;

            self.cache.entity_types = Some(
                rows.into_iter()
                    .map(|r| CachedRef {
                        code: r.name.clone(),
                        uuid: Some(r.entity_type_id),
                        display: r.name,
                    })
                    .collect(),
            );
        }

        self.lookup_in_cache(&self.cache.entity_types, code)
    }

    async fn resolve_attribute(&mut self, code: &str) -> Result<ResolveResult, String> {
        if self.cache.attributes.is_none() {
            let rows = sqlx::query!(
                r#"SELECT id, uuid, display_name
                   FROM "ob-poc".attribute_registry
                   ORDER BY id"#
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| format!("DB error loading attribute_registry: {}", e))?;

            self.cache.attributes = Some(
                rows.into_iter()
                    .map(|r| CachedRef {
                        code: r.id,
                        uuid: Some(r.uuid),
                        display: r.display_name,
                    })
                    .collect(),
            );
        }

        self.lookup_in_cache(&self.cache.attributes, code)
    }

    /// Resolve CBU by UUID (for runtime symbol resolution)
    async fn resolve_cbu(&mut self, value: &str) -> Result<ResolveResult, String> {
        // CBU IDs are UUIDs, parse and lookup directly
        let uuid =
            Uuid::parse_str(value).map_err(|_| format!("Invalid UUID for cbu-id: {}", value))?;

        let row = sqlx::query!(
            r#"SELECT cbu_id, name FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            uuid
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        match row {
            Some(r) => Ok(ResolveResult::Found {
                id: r.cbu_id,
                display: r.name,
            }),
            None => Ok(ResolveResult::NotFound {
                suggestions: vec![], // No fuzzy match for UUIDs
            }),
        }
    }

    /// Resolve entity by UUID
    async fn resolve_entity(&mut self, value: &str) -> Result<ResolveResult, String> {
        let uuid =
            Uuid::parse_str(value).map_err(|_| format!("Invalid UUID for entity-id: {}", value))?;

        let row = sqlx::query!(
            r#"SELECT entity_id, name FROM "ob-poc".entities WHERE entity_id = $1"#,
            uuid
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        match row {
            Some(r) => Ok(ResolveResult::Found {
                id: r.entity_id,
                display: r.name,
            }),
            None => Ok(ResolveResult::NotFound {
                suggestions: vec![],
            }),
        }
    }

    /// Resolve document by UUID
    async fn resolve_document(&mut self, value: &str) -> Result<ResolveResult, String> {
        let uuid = Uuid::parse_str(value)
            .map_err(|_| format!("Invalid UUID for document-id: {}", value))?;

        let row = sqlx::query!(
            r#"SELECT doc_id, document_name FROM "ob-poc".document_catalog WHERE doc_id = $1"#,
            uuid
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        match row {
            Some(r) => Ok(ResolveResult::Found {
                id: r.doc_id,
                display: r.document_name.unwrap_or_else(|| "unnamed".to_string()),
            }),
            None => Ok(ResolveResult::NotFound {
                suggestions: vec![],
            }),
        }
    }

    /// Resolve screening type (hardcoded enum for now)
    async fn resolve_screening_type(&mut self, value: &str) -> Result<ResolveResult, String> {
        let valid_types = ["PEP", "SANCTIONS", "ADVERSE_MEDIA", "WATCHLIST"];
        let upper = value.to_uppercase();

        if valid_types.contains(&upper.as_str()) {
            Ok(ResolveResult::FoundByCode {
                code: upper,
                uuid: None,
                display: value.to_string(),
            })
        } else {
            let suggestions = fuzzy_match(value, &valid_types, 3);
            Ok(ResolveResult::NotFound { suggestions })
        }
    }

    // =========================================================================
    // CACHE LOOKUP WITH FUZZY MATCHING
    // =========================================================================

    fn lookup_in_cache(
        &self,
        cache: &Option<Vec<CachedRef>>,
        code: &str,
    ) -> Result<ResolveResult, String> {
        let entries = cache.as_ref().ok_or("Cache not loaded")?;

        // Exact match (case-insensitive)
        let code_upper = code.to_uppercase();
        for entry in entries {
            if entry.code.to_uppercase() == code_upper {
                return Ok(match entry.uuid {
                    Some(uuid) => ResolveResult::Found {
                        id: uuid,
                        display: entry.display.clone(),
                    },
                    None => ResolveResult::FoundByCode {
                        code: entry.code.clone(),
                        uuid: None,
                        display: entry.display.clone(),
                    },
                });
            }
        }

        // No exact match - fuzzy match for suggestions
        let candidates: Vec<&str> = entries.iter().map(|e| e.code.as_str()).collect();
        let suggestions = fuzzy_match(code, &candidates, 5);

        Ok(ResolveResult::NotFound { suggestions })
    }
}

// =============================================================================
// FUZZY MATCHING
// =============================================================================

/// Simple fuzzy matching using Levenshtein distance
fn fuzzy_match(input: &str, candidates: &[&str], max_results: usize) -> Vec<SuggestedMatch> {
    let input_upper = input.to_uppercase();

    let mut scored: Vec<(f32, &str)> = candidates
        .iter()
        .map(|&candidate| {
            let score = similarity(&input_upper, &candidate.to_uppercase());
            (score, candidate)
        })
        .filter(|(score, _)| *score > 0.3) // Min threshold
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .into_iter()
        .take(max_results)
        .map(|(score, candidate)| SuggestedMatch {
            value: candidate.to_string(),
            display: candidate.to_string(),
            score,
        })
        .collect()
}

/// Calculate similarity score (0.0 - 1.0) using normalized Levenshtein distance
fn similarity(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let distance = levenshtein_distance(a, b);
    let max_len = a.len().max(b.len()) as f32;

    1.0 - (distance as f32 / max_len)
}

/// Levenshtein distance implementation
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for (i, row) in matrix.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in matrix[0].iter_mut().enumerate() {
        *val = j;
    }

    for (i, a_char) in a_chars.iter().enumerate() {
        for (j, b_char) in b_chars.iter().enumerate() {
            let cost = if a_char == b_char { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }

    matrix[a_len][b_len]
}

/// Get human-readable name for ref type
fn ref_type_name(ref_type: RefType) -> &'static str {
    match ref_type {
        RefType::DocumentType => "document type",
        RefType::Jurisdiction => "jurisdiction",
        RefType::Role => "role",
        RefType::EntityType => "entity type",
        RefType::AttributeId => "attribute",
        RefType::Cbu => "CBU",
        RefType::Entity => "entity",
        RefType::Document => "document",
        RefType::ScreeningType => "screening type",
    }
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
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("PASSPORT", "PASPORT"), 1);
    }

    #[test]
    fn test_similarity() {
        assert_eq!(similarity("abc", "abc"), 1.0);
        assert!(similarity("PASSPORT_GBR", "PASSPORT_GB") > 0.9);
        assert!(similarity("PASSPORT_GBR", "DRIVERS_LICENSE") < 0.5);
    }

    #[test]
    fn test_fuzzy_match() {
        let candidates = [
            "PASSPORT_GBR",
            "PASSPORT_USA",
            "DRIVERS_LICENSE_GBR",
            "CERT_OF_INCORPORATION",
        ];

        let results = fuzzy_match("PASSPORT_GB", &candidates, 3);
        assert!(!results.is_empty());
        assert_eq!(results[0].value, "PASSPORT_GBR"); // Best match

        let results2 = fuzzy_match("PASPORT_GBR", &candidates, 3);
        assert!(!results2.is_empty());
        assert_eq!(results2[0].value, "PASSPORT_GBR"); // Typo correction
    }

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
