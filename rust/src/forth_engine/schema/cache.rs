//! Schema cache for lookup table validation.

use std::collections::HashMap;
use crate::forth_engine::schema::types::RefType;

/// Cached lookup tables for validation and LSP completions.
#[derive(Debug, Clone, Default)]
pub struct SchemaCache {
    /// Document types: type_code -> DisplayInfo
    pub document_types: HashMap<String, LookupEntry>,
    /// Attributes: attr_id -> DisplayInfo
    pub attributes: HashMap<String, LookupEntry>,
    /// Roles: role_name -> DisplayInfo
    pub roles: HashMap<String, LookupEntry>,
    /// Entity types: type_code -> DisplayInfo
    pub entity_types: HashMap<String, LookupEntry>,
    /// Jurisdictions: iso_code -> DisplayInfo
    pub jurisdictions: HashMap<String, LookupEntry>,
    /// Screening lists: list_code -> DisplayInfo
    pub screening_lists: HashMap<String, LookupEntry>,
    /// Currencies: iso_code -> DisplayInfo
    pub currencies: HashMap<String, LookupEntry>,
}

/// Entry for LSP completion display.
#[derive(Debug, Clone)]
pub struct LookupEntry {
    /// Code to insert into DSL
    pub code: String,
    /// Human-readable display name
    pub display_name: String,
    /// Category for grouping
    pub category: Option<String>,
    /// Description for hover/docs
    pub description: Option<String>,
    /// Related attributes (for document types)
    pub extractable_attributes: Option<Vec<String>>,
}

impl SchemaCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create cache with default/static values for testing.
    pub fn with_defaults() -> Self {
        let mut cache = Self::new();
        
        // Default jurisdictions
        for (code, name) in &[
            ("GB", "United Kingdom"), ("US", "United States"), ("LU", "Luxembourg"),
            ("IE", "Ireland"), ("SG", "Singapore"), ("HK", "Hong Kong"),
            ("CH", "Switzerland"), ("DE", "Germany"), ("FR", "France"),
            ("KY", "Cayman Islands"), ("BVI", "British Virgin Islands"),
            ("JE", "Jersey"), ("GG", "Guernsey"),
        ] {
            cache.jurisdictions.insert(code.to_string(), LookupEntry {
                code: code.to_string(),
                display_name: name.to_string(),
                category: None,
                description: None,
                extractable_attributes: None,
            });
        }

        // Default currencies
        for (code, name) in &[
            ("USD", "US Dollar"), ("EUR", "Euro"), ("GBP", "British Pound"),
            ("CHF", "Swiss Franc"), ("SGD", "Singapore Dollar"), ("HKD", "Hong Kong Dollar"),
            ("JPY", "Japanese Yen"), ("CNY", "Chinese Yuan"),
        ] {
            cache.currencies.insert(code.to_string(), LookupEntry {
                code: code.to_string(),
                display_name: name.to_string(),
                category: None,
                description: None,
                extractable_attributes: None,
            });
        }

        // Default roles
        for (code, name, desc) in &[
            ("InvestmentManager", "Investment Manager", "Manages investments for the fund"),
            ("BeneficialOwner", "Beneficial Owner", "Ultimate beneficial owner (>25% ownership)"),
            ("Director", "Director", "Member of board of directors"),
            ("Custodian", "Custodian", "Holds assets in custody"),
            ("Administrator", "Administrator", "Fund administrator"),
            ("Auditor", "Auditor", "External auditor"),
            ("LegalCounsel", "Legal Counsel", "Legal advisor"),
            ("AuthorizedSignatory", "Authorized Signatory", "Can sign on behalf of entity"),
            ("ComplianceOfficer", "Compliance Officer", "Responsible for compliance"),
            ("MLRO", "MLRO", "Money Laundering Reporting Officer"),
        ] {
            cache.roles.insert(code.to_string(), LookupEntry {
                code: code.to_string(),
                display_name: name.to_string(),
                category: Some("Role".to_string()),
                description: Some(desc.to_string()),
                extractable_attributes: None,
            });
        }

        // Default document types
        for (code, name, category) in &[
            ("CERT_OF_INCORP", "Certificate of Incorporation", "Corporate"),
            ("CERT_GOOD_STANDING", "Certificate of Good Standing", "Corporate"),
            ("ARTICLES_OF_ASSOC", "Articles of Association", "Corporate"),
            ("MEMORANDUM_OF_ASSOC", "Memorandum of Association", "Corporate"),
            ("PASSPORT", "Passport", "Identity"),
            ("NATIONAL_ID", "National ID Card", "Identity"),
            ("DRIVING_LICENSE", "Driving License", "Identity"),
            ("UTILITY_BILL", "Utility Bill", "Address"),
            ("BANK_STATEMENT", "Bank Statement", "Financial"),
            ("AUDITED_ACCOUNTS", "Audited Accounts", "Financial"),
            ("TAX_RETURN", "Tax Return", "Financial"),
            ("TRUST_DEED", "Trust Deed", "Legal"),
            ("PARTNERSHIP_AGREEMENT", "Partnership Agreement", "Legal"),
        ] {
            cache.document_types.insert(code.to_string(), LookupEntry {
                code: code.to_string(),
                display_name: name.to_string(),
                category: Some(category.to_string()),
                description: None,
                extractable_attributes: None,
            });
        }

        // Default entity types
        for (code, name) in &[
            ("LIMITED_COMPANY", "Limited Company"),
            ("PROPER_PERSON", "Natural Person"),
            ("PARTNERSHIP", "Partnership"),
            ("TRUST", "Trust"),
            ("FOUNDATION", "Foundation"),
            ("GOVERNMENT_BODY", "Government Body"),
        ] {
            cache.entity_types.insert(code.to_string(), LookupEntry {
                code: code.to_string(),
                display_name: name.to_string(),
                category: None,
                description: None,
                extractable_attributes: None,
            });
        }

        cache
    }

    /// Check if a code exists for a RefType.
    pub fn exists(&self, ref_type: &RefType, code: &str) -> bool {
        match ref_type {
            RefType::DocumentType => self.document_types.contains_key(code),
            RefType::Attribute => self.attributes.contains_key(code),
            RefType::Role => self.roles.contains_key(code),
            RefType::EntityType => self.entity_types.contains_key(code),
            RefType::Jurisdiction => self.jurisdictions.contains_key(code),
            RefType::ScreeningList => self.screening_lists.contains_key(code),
            RefType::Currency => self.currencies.contains_key(code),
        }
    }

    /// Get entry for a RefType code.
    pub fn get(&self, ref_type: &RefType, code: &str) -> Option<&LookupEntry> {
        match ref_type {
            RefType::DocumentType => self.document_types.get(code),
            RefType::Attribute => self.attributes.get(code),
            RefType::Role => self.roles.get(code),
            RefType::EntityType => self.entity_types.get(code),
            RefType::Jurisdiction => self.jurisdictions.get(code),
            RefType::ScreeningList => self.screening_lists.get(code),
            RefType::Currency => self.currencies.get(code),
        }
    }

    /// Get suggestions for typo correction.
    pub fn suggest(&self, ref_type: &RefType, typo: &str) -> Vec<String> {
        let entries = self.get_map(ref_type);
        
        let mut suggestions: Vec<_> = entries.keys()
            .filter(|k| {
                levenshtein_distance(k, typo) <= 3
                    || k.to_lowercase().contains(&typo.to_lowercase())
                    || typo.to_lowercase().contains(&k.to_lowercase())
            })
            .cloned()
            .collect();

        suggestions.sort_by_key(|k| levenshtein_distance(k, typo));
        suggestions.truncate(5);
        suggestions
    }

    /// Get all entries for LSP completion.
    pub fn get_completions(&self, ref_type: &RefType) -> Vec<&LookupEntry> {
        self.get_map(ref_type).values().collect()
    }

    /// Filter completions by prefix.
    pub fn get_filtered_completions(&self, ref_type: &RefType, prefix: &str) -> Vec<&LookupEntry> {
        self.get_completions(ref_type)
            .into_iter()
            .filter(|e| {
                e.code.to_lowercase().starts_with(&prefix.to_lowercase())
                    || e.display_name.to_lowercase().contains(&prefix.to_lowercase())
            })
            .collect()
    }

    /// Get the map for a RefType.
    fn get_map(&self, ref_type: &RefType) -> &HashMap<String, LookupEntry> {
        match ref_type {
            RefType::DocumentType => &self.document_types,
            RefType::Attribute => &self.attributes,
            RefType::Role => &self.roles,
            RefType::EntityType => &self.entity_types,
            RefType::Jurisdiction => &self.jurisdictions,
            RefType::ScreeningList => &self.screening_lists,
            RefType::Currency => &self.currencies,
        }
    }
}

/// Calculate Levenshtein distance.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    if a_len == 0 { return b_len; }
    if b_len == 0 { return a_len; }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for i in 0..=a_len { matrix[i][0] = i; }
    for j in 0..=b_len { matrix[0][j] = j; }

    for (i, ca) in a.chars().enumerate() {
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca.to_lowercase().eq(cb.to_lowercase()) { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }

    matrix[a_len][b_len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exists() {
        let cache = SchemaCache::with_defaults();
        
        assert!(cache.exists(&RefType::Jurisdiction, "GB"));
        assert!(cache.exists(&RefType::Jurisdiction, "US"));
        assert!(!cache.exists(&RefType::Jurisdiction, "XX"));
        
        assert!(cache.exists(&RefType::Role, "InvestmentManager"));
        assert!(!cache.exists(&RefType::Role, "UnknownRole"));
    }

    #[test]
    fn test_suggest() {
        let cache = SchemaCache::with_defaults();
        
        let suggestions = cache.suggest(&RefType::Role, "Investmanager");
        assert!(suggestions.contains(&"InvestmentManager".to_string()));
        
        let suggestions = cache.suggest(&RefType::Jurisdiction, "UK");
        // Should suggest GB since UK is often used for GB
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_completions() {
        let cache = SchemaCache::with_defaults();
        
        let completions = cache.get_completions(&RefType::Currency);
        assert!(!completions.is_empty());
        
        let filtered = cache.get_filtered_completions(&RefType::Currency, "US");
        assert!(filtered.iter().any(|e| e.code == "USD"));
    }
}
