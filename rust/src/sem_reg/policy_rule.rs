//! Policy rule types for the semantic registry.
//!
//! A policy rule defines an enforceable constraint that governs
//! registry object behaviour. Rules consist of predicates (conditions)
//! and actions (enforcement outcomes).
//!
//! Examples: "PEP entities require enhanced due diligence",
//! "Proof-class attributes must have governed-tier evidence".

use serde::{Deserialize, Serialize};

/// Body for a policy rule snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRuleBody {
    /// Fully qualified name, e.g. `"kyc.pep-enhanced-dd"`
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description
    pub description: String,
    /// Owning domain
    pub domain: String,
    /// Scope: `global`, `domain`, `entity_type`
    #[serde(default)]
    pub scope: Option<String>,
    /// Priority (lower = higher priority, for conflict resolution)
    #[serde(default = "default_priority")]
    pub priority: i32,
    /// Predicates (all must match for the rule to fire)
    #[serde(default)]
    pub predicates: Vec<PolicyPredicate>,
    /// Actions to take when rule fires
    #[serde(default)]
    pub actions: Vec<PolicyAction>,
    /// Whether this rule is currently enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_priority() -> i32 {
    100
}

fn default_true() -> bool {
    true
}

/// A predicate (condition) in a policy rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyPredicate {
    /// Predicate kind: `attribute_value`, `governance_tier`, `trust_class`,
    /// `entity_type`, `taxonomy_membership`, `security_label`
    pub kind: String,
    /// The field or path being tested
    pub field: String,
    /// Operator: `eq`, `ne`, `in`, `not_in`, `gt`, `lt`, `contains`, `exists`
    pub operator: String,
    /// Expected value
    pub value: serde_json::Value,
}

/// An action to take when a policy rule fires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAction {
    /// Action kind: `require_evidence`, `block_publish`, `flag_review`,
    /// `set_trust_class`, `require_approval`, `restrict_access`
    pub kind: String,
    /// Action-specific parameters
    #[serde(default)]
    pub params: serde_json::Value,
    /// Human-readable description of this action
    #[serde(default)]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_rule_serde() {
        let body = PolicyRuleBody {
            fqn: "kyc.pep-enhanced-dd".into(),
            name: "PEP Enhanced Due Diligence".into(),
            description: "Politically exposed persons require enhanced checks".into(),
            domain: "kyc".into(),
            scope: Some("entity_type".into()),
            priority: 10,
            predicates: vec![PolicyPredicate {
                kind: "attribute_value".into(),
                field: "entity.pep-status".into(),
                operator: "eq".into(),
                value: serde_json::json!("active"),
            }],
            actions: vec![PolicyAction {
                kind: "require_evidence".into(),
                params: serde_json::json!({"evidence_fqn": "kyc.pep-enhanced-evidence"}),
                description: Some("Require enhanced evidence pack".into()),
            }],
            enabled: true,
        };
        let json = serde_json::to_value(&body).unwrap();
        let round: PolicyRuleBody = serde_json::from_value(json).unwrap();
        assert_eq!(round.fqn, "kyc.pep-enhanced-dd");
        assert_eq!(round.priority, 10);
        assert_eq!(round.predicates.len(), 1);
        assert_eq!(round.actions.len(), 1);
    }

    #[test]
    fn test_policy_rule_defaults() {
        let json = serde_json::json!({
            "fqn": "test.rule",
            "name": "Test",
            "description": "A test rule",
            "domain": "test"
        });
        let body: PolicyRuleBody = serde_json::from_value(json).unwrap();
        assert_eq!(body.priority, 100); // default
        assert!(body.enabled); // default true
        assert!(body.predicates.is_empty());
        assert!(body.actions.is_empty());
    }
}
