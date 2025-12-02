//! Parse rules from YAML configuration

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RulesConfig {
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub name: String,
    pub description: String,
    pub priority: i32,
    pub trigger: Trigger,
    pub condition: Condition,
    pub actions: Vec<Action>,

    #[serde(default)]
    pub enabled: Option<bool>,
}

impl Rule {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Trigger {
    pub event: String,
    #[serde(default)]
    pub schedule: Option<String>,
}

impl Trigger {
    pub fn is_scheduled(&self) -> bool {
        self.event == "scheduled"
    }

    pub fn matches_event(&self, event_name: &str) -> bool {
        self.event == event_name
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Condition {
    All { all: Vec<Condition> },
    Any { any: Vec<Condition> },
    Not { not: Box<Condition> },
    Leaf(LeafCondition),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeafCondition {
    pub field: String,
    #[serde(flatten)]
    pub operator: Operator,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Equals(Value),
    NotEquals(Value),
    In(Vec<Value>),
    NotIn(Vec<Value>),
    Contains(String),
    StartsWith(String),
    EndsWith(String),
    Gt(f64),
    Gte(f64),
    Lt(f64),
    Lte(f64),
    IsNull(bool),
    IsNotNull(bool),
    Matches(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Action {
    #[serde(rename = "type")]
    pub action_type: String,

    #[serde(default)]
    pub params: HashMap<String, Value>,
}

/// Load rules from a YAML file
pub fn load_rules(path: &Path) -> Result<Vec<Rule>> {
    let content = std::fs::read_to_string(path)?;
    let config: RulesConfig = serde_yaml::from_str(&content)?;

    // Filter to enabled rules only
    let rules: Vec<Rule> = config
        .rules
        .into_iter()
        .filter(|r| r.is_enabled())
        .collect();

    tracing::info!("Loaded {} rules from {:?}", rules.len(), path);

    Ok(rules)
}

/// Load rules from a string (for testing)
pub fn load_rules_from_str(yaml: &str) -> Result<Vec<Rule>> {
    let config: RulesConfig = serde_yaml::from_str(yaml)?;
    Ok(config
        .rules
        .into_iter()
        .filter(|r| r.is_enabled())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let yaml = r#"
rules:
  - name: test-rule
    description: "Test rule"
    priority: 10
    trigger:
      event: workstream.created
    condition:
      field: entity.jurisdiction
      equals: KY
    actions:
      - type: raise-red-flag
        params:
          flag-type: HIGH_RISK_JURISDICTION
          severity: ESCALATE
"#;

        let rules = load_rules_from_str(yaml).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].name, "test-rule");
    }

    #[test]
    fn test_parse_compound_condition() {
        let yaml = r#"
rules:
  - name: compound-rule
    description: "Compound condition"
    priority: 10
    trigger:
      event: workstream.created
    condition:
      all:
        - field: entity.type
          equals: trust
        - any:
            - field: entity.jurisdiction
              in: [KY, VG]
            - field: entity.name
              contains: nominee
    actions:
      - type: raise-red-flag
        params:
          flag-type: COMPLEX
          severity: ESCALATE
"#;

        let rules = load_rules_from_str(yaml).unwrap();
        assert_eq!(rules.len(), 1);

        match &rules[0].condition {
            Condition::All { all } => assert_eq!(all.len(), 2),
            _ => panic!("Expected All condition"),
        }
    }
}
