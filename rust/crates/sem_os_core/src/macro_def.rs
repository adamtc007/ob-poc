//! Macro definition body types — pure value types, no DB dependency.

use serde::{Deserialize, Serialize};

/// Body of a `macro_def` registry snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct MacroDefBody {
    pub(crate) fqn: String,
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) ui: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) routing: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) target: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) args: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) prereqs: Vec<serde_json::Value>,
    #[serde(default)]
    pub(crate) expands_to: Vec<serde_json::Value>,
    #[serde(default)]
    pub(crate) sets_state: Vec<serde_json::Value>,
    #[serde(default)]
    pub(crate) unlocks: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_round_trip() {
        let val = MacroDefBody {
            fqn: "case.open".into(),
            kind: "macro".into(),
            ui: Some(serde_json::json!({"label": "Open Case"})),
            routing: Some(serde_json::json!({"operator-domain": "case"})),
            target: None,
            args: None,
            prereqs: vec![],
            expands_to: vec![serde_json::json!({"verb": "kyc-case.create"})],
            sets_state: vec![],
            unlocks: vec!["case.submit".into()],
        };
        let json = serde_json::to_value(&val).unwrap();
        let back: MacroDefBody = serde_json::from_value(json.clone()).unwrap();
        let json2 = serde_json::to_value(&back).unwrap();
        assert_eq!(json, json2);
    }
}
