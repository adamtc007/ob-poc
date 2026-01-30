//! Role Cardinality Validation
//!
//! Validates that expanded macro output respects role cardinality constraints.
//! For example, UCITS funds must have exactly one depositary.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// Cardinality constraint for a role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Cardinality {
    /// Exactly one required
    One,
    /// Optional, at most one
    ZeroOrOne,
    /// At least one required
    OneOrMore,
    /// Any number (no constraint)
    ZeroOrMore,
}

impl Cardinality {
    /// Check if a count satisfies this cardinality
    pub fn is_satisfied(&self, count: usize) -> bool {
        match self {
            Cardinality::One => count == 1,
            Cardinality::ZeroOrOne => count <= 1,
            Cardinality::OneOrMore => count >= 1,
            Cardinality::ZeroOrMore => true,
        }
    }

    /// Get the minimum required count
    pub fn min_count(&self) -> usize {
        match self {
            Cardinality::One | Cardinality::OneOrMore => 1,
            Cardinality::ZeroOrOne | Cardinality::ZeroOrMore => 0,
        }
    }

    /// Get the maximum allowed count (None = unlimited)
    pub fn max_count(&self) -> Option<usize> {
        match self {
            Cardinality::One | Cardinality::ZeroOrOne => Some(1),
            Cardinality::OneOrMore | Cardinality::ZeroOrMore => None,
        }
    }

    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Cardinality::One => "exactly one",
            Cardinality::ZeroOrOne => "at most one",
            Cardinality::OneOrMore => "at least one",
            Cardinality::ZeroOrMore => "any number",
        }
    }
}

/// Configuration for a single role's cardinality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleCardinalityDef {
    pub cardinality: Cardinality,
    /// Structure types where this constraint applies (None = all)
    #[serde(default)]
    pub context: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Full cardinality configuration from YAML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardinalityConfig {
    pub roles: HashMap<String, RoleCardinalityDef>,
    #[serde(default)]
    pub structure_aliases: HashMap<String, Vec<String>>,
}

impl CardinalityConfig {
    /// Load configuration from a YAML file
    pub fn from_yaml_file(path: impl AsRef<Path>) -> Result<Self, CardinalityError> {
        let content = fs::read_to_string(path.as_ref()).map_err(|e| CardinalityError::Io {
            path: path.as_ref().to_string_lossy().to_string(),
            source: e,
        })?;
        Self::from_yaml(&content)
    }

    /// Parse configuration from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self, CardinalityError> {
        serde_yaml::from_str(yaml).map_err(CardinalityError::Parse)
    }

    /// Expand structure aliases to get all matching types
    pub fn expand_structure_type<'a>(&'a self, structure_type: &'a str) -> Vec<&'a str> {
        let mut result = vec![structure_type];

        // Check if this type is aliased
        for (alias, types) in &self.structure_aliases {
            if types.iter().any(|t| t == structure_type) {
                result.push(alias.as_str());
            }
        }

        result
    }

    /// Check if a role's cardinality applies to a given structure type
    pub fn applies_to_structure(&self, role: &str, structure_type: Option<&str>) -> bool {
        let Some(def) = self.roles.get(role) else {
            return false;
        };

        let Some(context) = &def.context else {
            // No context restriction = applies to all
            return true;
        };

        let Some(st) = structure_type else {
            // No structure type specified = assume it applies
            return true;
        };

        // Check direct match
        if context.iter().any(|c| c == st) {
            return true;
        }

        // Check via aliases (structure_type might match an alias that's in context)
        let expanded = self.expand_structure_type(st);
        for alias in expanded {
            if context.iter().any(|c| c == alias) {
                return true;
            }
        }

        false
    }
}

/// Registry holding cardinality rules
#[derive(Debug, Clone)]
pub struct CardinalityRegistry {
    config: CardinalityConfig,
}

impl CardinalityRegistry {
    /// Create a new registry from config
    pub fn new(config: CardinalityConfig) -> Self {
        Self { config }
    }

    /// Load from the default config path
    pub fn load_default() -> Result<Self, CardinalityError> {
        // Try multiple potential paths
        let paths = [
            "config/role_cardinality.yaml",
            "rust/config/role_cardinality.yaml",
            "../config/role_cardinality.yaml",
        ];

        for path in paths {
            if Path::new(path).exists() {
                return Ok(Self::new(CardinalityConfig::from_yaml_file(path)?));
            }
        }

        Err(CardinalityError::NotFound {
            searched: paths.iter().map(|s| s.to_string()).collect(),
        })
    }

    /// Get cardinality for a role, optionally filtered by structure type
    pub fn get_cardinality(&self, role: &str, structure_type: Option<&str>) -> Option<Cardinality> {
        let def = self.config.roles.get(role)?;

        // Check if context applies
        if !self.config.applies_to_structure(role, structure_type) {
            return None;
        }

        Some(def.cardinality)
    }

    /// Get all roles with cardinality constraints for a structure type
    pub fn roles_for_structure(&self, structure_type: Option<&str>) -> Vec<(&str, Cardinality)> {
        self.config
            .roles
            .iter()
            .filter_map(|(role, def)| {
                if self.config.applies_to_structure(role, structure_type) {
                    Some((role.as_str(), def.cardinality))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get required roles (cardinality = one or one-or-more) for a structure type
    pub fn required_roles(&self, structure_type: Option<&str>) -> Vec<&str> {
        self.roles_for_structure(structure_type)
            .into_iter()
            .filter(|(_, c)| c.min_count() > 0)
            .map(|(r, _)| r)
            .collect()
    }

    /// Access the underlying config
    pub fn config(&self) -> &CardinalityConfig {
        &self.config
    }
}

/// Validator that checks role assignments against cardinality rules
#[derive(Debug)]
pub struct CardinalityValidator {
    registry: CardinalityRegistry,
}

impl CardinalityValidator {
    /// Create a new validator
    pub fn new(registry: CardinalityRegistry) -> Self {
        Self { registry }
    }

    /// Validate role assignments for a structure
    ///
    /// # Arguments
    /// * `role_counts` - Map of role name to count of entities in that role
    /// * `structure_type` - Optional structure type for context filtering
    ///
    /// # Returns
    /// Vector of diagnostics (empty if valid)
    pub fn validate(
        &self,
        role_counts: &HashMap<String, usize>,
        structure_type: Option<&str>,
    ) -> Vec<CardinalityDiagnostic> {
        let mut diagnostics = Vec::new();

        // Check roles that are present
        for (role, &count) in role_counts {
            if let Some(cardinality) = self.registry.get_cardinality(role, structure_type) {
                if !cardinality.is_satisfied(count) {
                    diagnostics.push(CardinalityDiagnostic {
                        role: role.clone(),
                        expected: cardinality,
                        actual: count,
                        structure_type: structure_type.map(String::from),
                    });
                }
            }
        }

        // Check required roles that are missing
        let present_roles: HashSet<_> = role_counts.keys().collect();
        for required_role in self.registry.required_roles(structure_type) {
            if !present_roles.contains(&required_role.to_string()) {
                let cardinality = self
                    .registry
                    .get_cardinality(required_role, structure_type)
                    .unwrap_or(Cardinality::One);

                diagnostics.push(CardinalityDiagnostic {
                    role: required_role.to_string(),
                    expected: cardinality,
                    actual: 0,
                    structure_type: structure_type.map(String::from),
                });
            }
        }

        diagnostics
    }

    /// Validate a list of role assignments (role names only, count = 1 each)
    pub fn validate_roles(
        &self,
        roles: &[&str],
        structure_type: Option<&str>,
    ) -> Vec<CardinalityDiagnostic> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for role in roles {
            *counts.entry(role.to_string()).or_default() += 1;
        }
        self.validate(&counts, structure_type)
    }

    /// Check if a specific role assignment is valid
    pub fn is_valid_assignment(
        &self,
        role: &str,
        current_count: usize,
        adding: usize,
        structure_type: Option<&str>,
    ) -> bool {
        let Some(cardinality) = self.registry.get_cardinality(role, structure_type) else {
            // Unknown role = no constraint
            return true;
        };

        let new_count = current_count + adding;

        // Check max constraint
        if let Some(max) = cardinality.max_count() {
            if new_count > max {
                return false;
            }
        }

        true
    }

    /// Access the registry
    pub fn registry(&self) -> &CardinalityRegistry {
        &self.registry
    }
}

/// Diagnostic for cardinality violation
#[derive(Debug, Clone, Serialize)]
pub struct CardinalityDiagnostic {
    pub role: String,
    pub expected: Cardinality,
    pub actual: usize,
    pub structure_type: Option<String>,
}

impl CardinalityDiagnostic {
    /// Format as human-readable message
    pub fn message(&self) -> String {
        let context = self
            .structure_type
            .as_ref()
            .map(|s| format!(" for {} structures", s))
            .unwrap_or_default();

        match self.expected {
            Cardinality::One => {
                format!(
                    "Role '{}' requires exactly one assignment{}, found {}",
                    self.role, context, self.actual
                )
            }
            Cardinality::ZeroOrOne if self.actual > 1 => {
                format!(
                    "Role '{}' allows at most one assignment{}, found {}",
                    self.role, context, self.actual
                )
            }
            Cardinality::OneOrMore if self.actual == 0 => {
                format!(
                    "Role '{}' requires at least one assignment{}",
                    self.role, context
                )
            }
            _ => {
                format!(
                    "Role '{}' has invalid count: expected {}, found {}{}",
                    self.role,
                    self.expected.description(),
                    self.actual,
                    context
                )
            }
        }
    }

    /// Is this a missing required role?
    pub fn is_missing(&self) -> bool {
        self.actual == 0 && self.expected.min_count() > 0
    }

    /// Is this a duplicate violation?
    pub fn is_duplicate(&self) -> bool {
        self.expected
            .max_count()
            .map_or(false, |max| self.actual > max)
    }
}

/// Errors that can occur during cardinality operations
#[derive(Debug)]
pub enum CardinalityError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Parse(serde_yaml::Error),
    NotFound {
        searched: Vec<String>,
    },
}

impl std::fmt::Display for CardinalityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardinalityError::Io { path, source } => {
                write!(
                    f,
                    "Failed to read cardinality config '{}': {}",
                    path, source
                )
            }
            CardinalityError::Parse(e) => {
                write!(f, "Failed to parse cardinality config: {}", e)
            }
            CardinalityError::NotFound { searched } => {
                write!(
                    f,
                    "Cardinality config not found. Searched: {}",
                    searched.join(", ")
                )
            }
        }
    }
}

impl std::error::Error for CardinalityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CardinalityError::Io { source, .. } => Some(source),
            CardinalityError::Parse(e) => Some(e),
            CardinalityError::NotFound { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CardinalityConfig {
        let yaml = r#"
roles:
  fund-vehicle:
    cardinality: one
    description: "The fund entity"
  depositary:
    cardinality: one
    context: [ucits, aif]
  custodian:
    cardinality: one-or-more
  prime-broker:
    cardinality: zero-or-more
    context: [hedge]
  general-partner:
    cardinality: one
    context: [pe]
  auditor:
    cardinality: one

structure_aliases:
  ucits: [sicav, icav-ucits]
  aif: [raif, qiaif]
  pe: [scsp, lp]
  hedge: [qiaif-hedge]
"#;
        CardinalityConfig::from_yaml(yaml).unwrap()
    }

    #[test]
    fn test_cardinality_is_satisfied() {
        assert!(Cardinality::One.is_satisfied(1));
        assert!(!Cardinality::One.is_satisfied(0));
        assert!(!Cardinality::One.is_satisfied(2));

        assert!(Cardinality::ZeroOrOne.is_satisfied(0));
        assert!(Cardinality::ZeroOrOne.is_satisfied(1));
        assert!(!Cardinality::ZeroOrOne.is_satisfied(2));

        assert!(!Cardinality::OneOrMore.is_satisfied(0));
        assert!(Cardinality::OneOrMore.is_satisfied(1));
        assert!(Cardinality::OneOrMore.is_satisfied(5));

        assert!(Cardinality::ZeroOrMore.is_satisfied(0));
        assert!(Cardinality::ZeroOrMore.is_satisfied(100));
    }

    #[test]
    fn test_config_parsing() {
        let config = test_config();
        assert_eq!(config.roles.len(), 6);
        assert_eq!(config.roles["fund-vehicle"].cardinality, Cardinality::One);
        assert_eq!(
            config.roles["custodian"].cardinality,
            Cardinality::OneOrMore
        );
    }

    #[test]
    fn test_structure_alias_expansion() {
        let config = test_config();

        let expanded = config.expand_structure_type("sicav");
        assert!(expanded.contains(&"sicav"));
        assert!(expanded.contains(&"ucits"));

        let expanded = config.expand_structure_type("raif");
        assert!(expanded.contains(&"raif"));
        assert!(expanded.contains(&"aif"));
    }

    #[test]
    fn test_context_filtering() {
        let config = test_config();

        // depositary applies to ucits but not pe
        assert!(config.applies_to_structure("depositary", Some("ucits")));
        assert!(config.applies_to_structure("depositary", Some("sicav"))); // via alias
        assert!(!config.applies_to_structure("depositary", Some("pe")));

        // fund-vehicle applies to all (no context)
        assert!(config.applies_to_structure("fund-vehicle", Some("ucits")));
        assert!(config.applies_to_structure("fund-vehicle", Some("pe")));
        assert!(config.applies_to_structure("fund-vehicle", None));
    }

    #[test]
    fn test_registry_get_cardinality() {
        let registry = CardinalityRegistry::new(test_config());

        // fund-vehicle: one (no context)
        assert_eq!(
            registry.get_cardinality("fund-vehicle", None),
            Some(Cardinality::One)
        );
        assert_eq!(
            registry.get_cardinality("fund-vehicle", Some("ucits")),
            Some(Cardinality::One)
        );

        // depositary: one (ucits/aif context)
        assert_eq!(
            registry.get_cardinality("depositary", Some("ucits")),
            Some(Cardinality::One)
        );
        assert_eq!(
            registry.get_cardinality("depositary", Some("sicav")),
            Some(Cardinality::One)
        ); // via alias
        assert_eq!(registry.get_cardinality("depositary", Some("pe")), None); // not applicable

        // general-partner: one (pe context)
        assert_eq!(
            registry.get_cardinality("general-partner", Some("pe")),
            Some(Cardinality::One)
        );
        assert_eq!(
            registry.get_cardinality("general-partner", Some("scsp")),
            Some(Cardinality::One)
        ); // via alias
        assert_eq!(
            registry.get_cardinality("general-partner", Some("ucits")),
            None
        );
    }

    #[test]
    fn test_validator_valid_structure() {
        let validator = CardinalityValidator::new(CardinalityRegistry::new(test_config()));

        let mut counts = HashMap::new();
        counts.insert("fund-vehicle".to_string(), 1);
        counts.insert("depositary".to_string(), 1);
        counts.insert("custodian".to_string(), 2);
        counts.insert("auditor".to_string(), 1);

        let diagnostics = validator.validate(&counts, Some("ucits"));
        assert!(
            diagnostics.is_empty(),
            "Expected valid, got: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_validator_missing_required() {
        let validator = CardinalityValidator::new(CardinalityRegistry::new(test_config()));

        let mut counts = HashMap::new();
        counts.insert("fund-vehicle".to_string(), 1);
        // Missing: depositary, custodian, auditor

        let diagnostics = validator.validate(&counts, Some("ucits"));
        assert!(!diagnostics.is_empty());

        let missing_roles: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.is_missing())
            .map(|d| d.role.as_str())
            .collect();

        assert!(missing_roles.contains(&"depositary"));
        assert!(missing_roles.contains(&"custodian"));
        assert!(missing_roles.contains(&"auditor"));
    }

    #[test]
    fn test_validator_duplicate_violation() {
        let validator = CardinalityValidator::new(CardinalityRegistry::new(test_config()));

        let mut counts = HashMap::new();
        counts.insert("fund-vehicle".to_string(), 2); // Should be exactly 1
        counts.insert("depositary".to_string(), 1);
        counts.insert("custodian".to_string(), 1);
        counts.insert("auditor".to_string(), 1);

        let diagnostics = validator.validate(&counts, Some("ucits"));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].role, "fund-vehicle");
        assert!(diagnostics[0].is_duplicate());
    }

    #[test]
    fn test_validator_pe_structure() {
        let validator = CardinalityValidator::new(CardinalityRegistry::new(test_config()));

        let mut counts = HashMap::new();
        counts.insert("fund-vehicle".to_string(), 1);
        counts.insert("general-partner".to_string(), 1);
        counts.insert("custodian".to_string(), 1);
        counts.insert("auditor".to_string(), 1);
        // Note: depositary NOT required for PE

        let diagnostics = validator.validate(&counts, Some("pe"));
        assert!(
            diagnostics.is_empty(),
            "Expected valid PE, got: {:?}",
            diagnostics
        );
    }

    #[test]
    fn test_is_valid_assignment() {
        let validator = CardinalityValidator::new(CardinalityRegistry::new(test_config()));

        // fund-vehicle: can only have 1
        assert!(validator.is_valid_assignment("fund-vehicle", 0, 1, None));
        assert!(!validator.is_valid_assignment("fund-vehicle", 1, 1, None));

        // custodian: one-or-more
        assert!(validator.is_valid_assignment("custodian", 0, 1, None));
        assert!(validator.is_valid_assignment("custodian", 5, 3, None));

        // unknown role: no constraint
        assert!(validator.is_valid_assignment("unknown-role", 0, 10, None));
    }

    #[test]
    fn test_diagnostic_messages() {
        let diag = CardinalityDiagnostic {
            role: "depositary".to_string(),
            expected: Cardinality::One,
            actual: 0,
            structure_type: Some("ucits".to_string()),
        };
        assert!(diag.message().contains("exactly one"));
        assert!(diag.message().contains("ucits"));

        let diag = CardinalityDiagnostic {
            role: "fund-vehicle".to_string(),
            expected: Cardinality::One,
            actual: 2,
            structure_type: None,
        };
        assert!(diag.message().contains("exactly one"));
        assert!(diag.message().contains("found 2"));
    }
}
