//! Policy rule body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

fn default_priority() -> i32 {
    100
}

fn default_true() -> bool {
    true
}

/// Body of a `policy_rule` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRuleBody {
    pub fqn: String,
    pub name: String,
    pub description: String,
    pub domain: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default)]
    pub predicates: Vec<PolicyPredicate>,
    #[serde(default)]
    pub actions: Vec<PolicyAction>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A predicate that determines when a policy applies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyPredicate {
    pub kind: String,
    pub field: String,
    pub operator: String,
    pub value: serde_json::Value,
}

/// An action taken when a policy's predicates match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyAction {
    pub kind: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub description: Option<String>,
}
