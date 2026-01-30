//! Document Bundle Type Definitions
//!
//! Core types for document bundles, loaded from YAML and stored in database.

use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A document bundle definition (from YAML)
///
/// Bundles define sets of required documents for fund structures.
/// They support inheritance via the `extends` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocsBundleDef {
    /// Unique identifier (e.g., "docs.bundle.ucits-baseline")
    pub id: String,

    /// Human-readable display name
    #[serde(rename = "display-name")]
    pub display_name: String,

    /// Description of what this bundle is for
    #[serde(default)]
    pub description: Option<String>,

    /// Version string (e.g., "2024-03")
    pub version: String,

    /// When this bundle version becomes effective
    #[serde(rename = "effective-from")]
    pub effective_from: NaiveDate,

    /// When this bundle version expires (None = currently active)
    #[serde(rename = "effective-to", default)]
    pub effective_to: Option<NaiveDate>,

    /// Parent bundle ID for inheritance
    #[serde(default)]
    pub extends: Option<String>,

    /// Documents required by this bundle
    #[serde(default)]
    pub documents: Vec<BundleDocumentDef>,
}

impl DocsBundleDef {
    /// Check if this bundle is currently effective
    pub fn is_effective(&self) -> bool {
        let today = Utc::now().date_naive();
        self.effective_from <= today && self.effective_to.map_or(true, |end| end > today)
    }

    /// Check if this bundle was effective on a given date
    pub fn is_effective_on(&self, date: NaiveDate) -> bool {
        self.effective_from <= date && self.effective_to.map_or(true, |end| end > date)
    }
}

/// A document requirement within a bundle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleDocumentDef {
    /// Document identifier (e.g., "prospectus", "kiid")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Description of the document
    #[serde(default)]
    pub description: Option<String>,

    /// Whether this document is required (default: true)
    #[serde(default = "default_required")]
    pub required: bool,

    /// Condition for when this document is required
    /// e.g., "has-prime-broker", "umbrella == true"
    #[serde(rename = "required-if", default)]
    pub required_if: Option<String>,

    /// Reference to document template (optional)
    #[serde(rename = "template-ref", default)]
    pub template_ref: Option<String>,

    /// Display order within the bundle
    #[serde(rename = "sort-order", default)]
    pub sort_order: i32,
}

fn default_required() -> bool {
    true
}

/// A resolved document from a bundle (after inheritance resolution)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedBundleDocument {
    /// The root bundle this was resolved for
    pub bundle_id: String,

    /// Document identifier
    pub document_id: String,

    /// Human-readable name
    pub document_name: String,

    /// Description
    pub description: Option<String>,

    /// Whether required
    pub required: bool,

    /// Conditional requirement
    pub required_if: Option<String>,

    /// Template reference
    pub template_ref: Option<String>,

    /// Display order
    pub sort_order: i32,

    /// Which bundle this document came from (may differ due to inheritance)
    pub source_bundle_id: String,
}

/// Record of a bundle applied to a CBU
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AppliedBundle {
    /// Unique ID
    pub applied_id: Uuid,

    /// CBU this bundle was applied to
    pub cbu_id: Uuid,

    /// Bundle identifier
    pub bundle_id: String,

    /// Version at time of application
    pub bundle_version: String,

    /// Macro that applied this bundle (if any)
    pub macro_id: Option<String>,

    /// When it was applied
    pub applied_at: chrono::DateTime<Utc>,

    /// Who/what applied it
    pub applied_by: Option<String>,
}

/// Result of applying a bundle to a CBU
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyBundleResult {
    /// Applied bundle record
    pub applied_bundle: AppliedBundle,

    /// Document requirements created
    pub requirements: Vec<CreatedRequirement>,
}

/// A document requirement created from bundle application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatedRequirement {
    /// Requirement ID
    pub requirement_id: Uuid,

    /// Document type
    pub document_id: String,

    /// Document name
    pub document_name: String,

    /// Whether required
    pub required: bool,

    /// Current status
    pub status: String,
}

/// Context for evaluating required_if conditions
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BundleContext {
    /// Boolean flags (e.g., "has-prime-broker" => true)
    #[serde(default)]
    pub flags: std::collections::HashMap<String, bool>,

    /// String values (e.g., "wrapper" => "sicav")
    #[serde(default)]
    pub values: std::collections::HashMap<String, String>,
}

impl BundleContext {
    /// Create empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a boolean flag
    pub fn with_flag(mut self, key: impl Into<String>, value: bool) -> Self {
        self.flags.insert(key.into(), value);
        self
    }

    /// Set a string value
    pub fn with_value(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.values.insert(key.into(), value.into());
        self
    }

    /// Evaluate a required_if condition
    ///
    /// Supports simple conditions:
    /// - "flag-name" → checks if flag exists and is true
    /// - "key == value" → checks if key equals value
    pub fn evaluate(&self, condition: &str) -> bool {
        let condition = condition.trim();

        // Check for equality expression: "key == value"
        if let Some((key, value)) = condition.split_once("==") {
            let key = key.trim();
            let value = value.trim().trim_matches('"');

            // Check flags first
            if let Some(&flag_value) = self.flags.get(key) {
                return flag_value.to_string() == value;
            }

            // Check values
            if let Some(actual_value) = self.values.get(key) {
                return actual_value == value;
            }

            return false;
        }

        // Simple flag check
        self.flags.get(condition).copied().unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bundle_context_flag() {
        let ctx = BundleContext::new()
            .with_flag("has-prime-broker", true)
            .with_flag("umbrella", false);

        assert!(ctx.evaluate("has-prime-broker"));
        assert!(!ctx.evaluate("umbrella"));
        assert!(!ctx.evaluate("unknown-flag"));
    }

    #[test]
    fn test_bundle_context_equality() {
        let ctx = BundleContext::new()
            .with_value("wrapper", "sicav")
            .with_flag("umbrella", true);

        assert!(ctx.evaluate("wrapper == sicav"));
        assert!(ctx.evaluate("wrapper == \"sicav\""));
        assert!(!ctx.evaluate("wrapper == raif"));
        assert!(ctx.evaluate("umbrella == true"));
    }
}
