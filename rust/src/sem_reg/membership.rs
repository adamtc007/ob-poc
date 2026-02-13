//! Membership rules for the semantic registry.
//!
//! A membership rule binds a registry object (attribute, verb, entity type)
//! to a taxonomy node. This enables classification queries such as
//! "which attributes belong to KYC High Risk?".

use serde::{Deserialize, Serialize};

/// The kind of membership relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipKind {
    /// Object is directly classified under this node
    Direct,
    /// Object inherits membership through a parent relationship
    Inherited,
    /// Object is conditionally classified (rule must be evaluated)
    Conditional,
    /// Object is excluded from this node (negative membership)
    Excluded,
}

/// A condition that must hold for conditional membership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipCondition {
    /// Type of condition: `attribute_equals`, `entity_has_role`, etc.
    pub kind: String,
    /// The field or attribute to check
    pub field: String,
    /// Operator: `eq`, `ne`, `in`, `not_in`, `gt`, `lt`
    #[serde(default = "default_eq")]
    pub operator: String,
    /// Expected value(s)
    pub value: serde_json::Value,
}

fn default_eq() -> String {
    "eq".into()
}

/// Body for a membership rule snapshot.
///
/// Links a target registry object to a taxonomy node with a membership kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipRuleBody {
    /// Fully qualified name for this rule
    pub fqn: String,
    /// Human-readable name
    pub name: String,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// FQN of the taxonomy this rule operates within
    pub taxonomy_fqn: String,
    /// FQN of the specific taxonomy node
    pub node_fqn: String,
    /// Kind of membership
    pub membership_kind: MembershipKind,
    /// What type of registry object is being classified
    /// (`attribute_def`, `verb_contract`, `entity_type_def`)
    pub target_type: String,
    /// FQN of the target registry object
    pub target_fqn: String,
    /// Conditions (for `MembershipKind::Conditional`)
    #[serde(default)]
    pub conditions: Vec<MembershipCondition>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_membership_rule_serde() {
        let body = MembershipRuleBody {
            fqn: "risk.kyc-tier.high.entity.proper-person".into(),
            name: "Proper persons in High Risk".into(),
            description: Some("Natural persons classified as high risk".into()),
            taxonomy_fqn: "risk.kyc-tier".into(),
            node_fqn: "risk.kyc-tier.high".into(),
            membership_kind: MembershipKind::Direct,
            target_type: "entity_type_def".into(),
            target_fqn: "entity.proper-person".into(),
            conditions: vec![],
        };
        let json = serde_json::to_value(&body).unwrap();
        let round: MembershipRuleBody = serde_json::from_value(json).unwrap();
        assert_eq!(round.fqn, "risk.kyc-tier.high.entity.proper-person");
        assert_eq!(round.membership_kind, MembershipKind::Direct);
    }

    #[test]
    fn test_conditional_membership() {
        let body = MembershipRuleBody {
            fqn: "risk.kyc-tier.high.attr.pep-status".into(),
            name: "PEP status triggers high risk".into(),
            description: None,
            taxonomy_fqn: "risk.kyc-tier".into(),
            node_fqn: "risk.kyc-tier.high".into(),
            membership_kind: MembershipKind::Conditional,
            target_type: "attribute_def".into(),
            target_fqn: "entity.pep-status".into(),
            conditions: vec![MembershipCondition {
                kind: "attribute_equals".into(),
                field: "pep-status".into(),
                operator: "eq".into(),
                value: serde_json::json!("active"),
            }],
        };
        assert_eq!(body.membership_kind, MembershipKind::Conditional);
        assert_eq!(body.conditions.len(), 1);
    }

    #[test]
    fn test_membership_kind_variants() {
        let kinds = [
            MembershipKind::Direct,
            MembershipKind::Inherited,
            MembershipKind::Conditional,
            MembershipKind::Excluded,
        ];
        for kind in &kinds {
            let json = serde_json::to_value(kind).unwrap();
            let round: MembershipKind = serde_json::from_value(json).unwrap();
            assert_eq!(&round, kind);
        }
    }
}
