use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookSpec {
    pub id: String,
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub slots: HashMap<String, SlotSpec>,
    pub steps: Vec<StepSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotSpec {
    #[serde(rename = "type")]
    pub slot_type: String,
    pub required: bool,
    #[serde(default)]
    pub default: Option<serde_yaml::Value>,
    pub autofill_from: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepSpec {
    pub id: String,
    pub verb: String,
    #[serde(default)]
    pub args: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub after: Vec<String>,
}
