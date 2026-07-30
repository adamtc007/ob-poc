//! Applicability Rules
//!
//! Loads and evaluates business rules from database metadata.
//! Used by the CSG Linter to validate document/attribute applicability.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// RULE STRUCTURES
// =============================================================================

/// All loaded applicability rules
#[derive(Debug, Default)]
pub(crate) struct ApplicabilityRules {
    pub document_rules: HashMap<String, DocumentApplicability>,
}

/// Applicability rules for a document type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct DocumentApplicability {
    #[serde(default)]
    pub entity_types: Vec<String>,

    #[serde(default)]
    pub jurisdictions: Vec<String>,

    #[serde(default)]
    pub client_types: Vec<String>,

    #[serde(default)]
    pub required_for: Vec<String>,

    #[serde(default)]
    pub excludes: Vec<String>,

    #[serde(default)]
    pub requires: Vec<String>,

    #[serde(default)]
    pub category: Option<String>,
}

impl DocumentApplicability {
    /// Check if document applies to given entity type (supports wildcards)
    pub(crate) fn applies_to_entity_type(&self, entity_type: &str) -> bool {
        if self.entity_types.is_empty() {
            return true; // No restriction
        }

        self.entity_types.iter().any(|allowed| {
            if allowed.ends_with('*') {
                let prefix = &allowed[..allowed.len() - 1];
                entity_type.starts_with(prefix)
            } else {
                allowed == entity_type || entity_type.starts_with(&format!("{}_", allowed))
            }
        })
    }

    /// Check if document applies to given jurisdiction
    pub(crate) fn applies_to_jurisdiction(&self, jurisdiction: &str) -> bool {
        if self.jurisdictions.is_empty() {
            return true;
        }
        self.jurisdictions.iter().any(|j| j == jurisdiction)
    }

    /// Check if document applies to given client type
    pub(crate) fn applies_to_client_type(&self, client_type: &str) -> bool {
        if self.client_types.is_empty() {
            return true;
        }
        self.client_types.iter().any(|c| c == client_type)
    }

}

// =============================================================================
// RULE LOADING
// =============================================================================

impl ApplicabilityRules {
    /// Load all rules from database
    #[cfg(feature = "database")]
    pub(crate) async fn load(pool: &PgPool) -> Result<Self, String> {
        Ok(Self {
            document_rules: Self::load_document_rules(pool).await?,
        })
    }

    #[cfg(feature = "database")]
    async fn load_document_rules(
        pool: &PgPool,
    ) -> Result<HashMap<String, DocumentApplicability>, String> {
        let rows = sqlx::query!(
            r#"SELECT type_code, applicability
               FROM "ob-poc".document_types
               WHERE applicability IS NOT NULL AND applicability != '{}'::jsonb"#
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to load document rules: {}", e))?;

        let mut rules = HashMap::new();
        for row in rows {
            let applicability = row
                .applicability
                .and_then(|v| serde_json::from_value::<DocumentApplicability>(v).ok())
                .unwrap_or_default();
            rules.insert(row.type_code, applicability);
        }

        Ok(rules)
    }

    /// Find valid documents for an entity type
    pub(crate) fn valid_documents_for_entity(&self, entity_type: &str) -> Vec<&str> {
        self.document_rules
            .iter()
            .filter(|(_, rule)| rule.applies_to_entity_type(entity_type))
            .map(|(code, _)| code.as_str())
            .collect()
    }


}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_applicability_empty() {
        let rule = DocumentApplicability::default();
        assert!(rule.applies_to_entity_type("ANY_TYPE"));
        assert!(rule.applies_to_jurisdiction("ANY"));
    }

    #[test]
    fn test_document_applicability_exact_match() {
        let rule = DocumentApplicability {
            entity_types: vec!["PROPER_PERSON_NATURAL".to_string()],
            ..Default::default()
        };
        assert!(rule.applies_to_entity_type("PROPER_PERSON_NATURAL"));
        assert!(!rule.applies_to_entity_type("LIMITED_COMPANY_PRIVATE"));
    }

    #[test]
    fn test_document_applicability_wildcard() {
        let rule = DocumentApplicability {
            entity_types: vec!["LIMITED_COMPANY_*".to_string()],
            ..Default::default()
        };
        assert!(rule.applies_to_entity_type("LIMITED_COMPANY_PRIVATE"));
        assert!(rule.applies_to_entity_type("LIMITED_COMPANY_PUBLIC"));
        assert!(!rule.applies_to_entity_type("LIMITED_COMPANY"));
        assert!(!rule.applies_to_entity_type("PROPER_PERSON_NATURAL"));
    }

    #[test]
    fn test_document_applicability_jurisdiction() {
        let rule = DocumentApplicability {
            jurisdictions: vec!["GB".to_string(), "US".to_string()],
            ..Default::default()
        };
        assert!(rule.applies_to_jurisdiction("GB"));
        assert!(rule.applies_to_jurisdiction("US"));
        assert!(!rule.applies_to_jurisdiction("DE"));
    }

    #[test]
    fn test_document_applicability_parent_match() {
        // Test that PROPER_PERSON matches PROPER_PERSON_NATURAL via prefix
        let rule = DocumentApplicability {
            entity_types: vec!["PROPER_PERSON".to_string()],
            ..Default::default()
        };
        assert!(rule.applies_to_entity_type("PROPER_PERSON"));
        assert!(rule.applies_to_entity_type("PROPER_PERSON_NATURAL"));
        assert!(rule.applies_to_entity_type("PROPER_PERSON_BENEFICIAL_OWNER"));
        assert!(!rule.applies_to_entity_type("LIMITED_COMPANY_PRIVATE"));
    }
}
