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
