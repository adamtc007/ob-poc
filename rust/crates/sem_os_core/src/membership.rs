//! Membership rule body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

/// Kind of taxonomy membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MembershipKind {
    Direct,
    Inherited,
    Conditional,
    Excluded,
}

/// A condition on a conditional membership rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipCondition {
    pub kind: String,
    pub field: String,
    #[serde(default = "default_eq")]
    pub operator: String,
    pub value: serde_json::Value,
}

fn default_eq() -> String {
    "eq".into()
}

/// Body of a `membership_rule` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipRuleBody {
    pub fqn: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub taxonomy_fqn: String,
    pub node_fqn: String,
    pub membership_kind: MembershipKind,
    pub target_type: String,
    pub target_fqn: String,
    #[serde(default)]
    pub conditions: Vec<MembershipCondition>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let val = MembershipRuleBody {
            fqn: "mem.kyc_fund".into(),
            name: "KYC Fund".into(),
            description: Some("Fund membership rule".into()),
            taxonomy_fqn: "tax.kyc".into(),
            node_fqn: "tax.kyc.fund".into(),
            membership_kind: MembershipKind::Conditional,
            target_type: "entity_type_def".into(),
            target_fqn: "entity.fund".into(),
            conditions: vec![MembershipCondition {
                kind: "attribute_equals".into(),
                field: "entity.type".into(),
                operator: "eq".into(),
                value: serde_json::json!("fund"),
            }],
        };
        let json = serde_json::to_value(&val).unwrap();
        let back: MembershipRuleBody = serde_json::from_value(json.clone()).unwrap();
        let json2 = serde_json::to_value(&back).unwrap();
        assert_eq!(json, json2);
    }

    #[test]
    fn membership_kind_snake_case() {
        assert_eq!(serde_json::to_value(MembershipKind::Direct).unwrap(), "direct");
        assert_eq!(serde_json::to_value(MembershipKind::Inherited).unwrap(), "inherited");
        assert_eq!(serde_json::to_value(MembershipKind::Conditional).unwrap(), "conditional");
        assert_eq!(serde_json::to_value(MembershipKind::Excluded).unwrap(), "excluded");
    }

    #[test]
    fn default_operator_eq() {
        let cond: MembershipCondition = serde_json::from_value(serde_json::json!({
            "kind": "attr", "field": "f", "value": true
        }))
        .unwrap();
        assert_eq!(cond.operator, "eq");
    }
}
