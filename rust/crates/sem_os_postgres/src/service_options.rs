//! Service-options framework: the v1 pure planner (`ResourceFanoutPlanner`)
//! plus the small set of value types it produces and consumes. The
//! repository + insert-row scaffolding that originally lived alongside
//! (~800 LOC, `ServiceOptionsRepository` + 7 `NewXxx` insert helpers +
//! `OptionResolver` + `CoverageValidator` + the 6 read-side `FromRow`
//! structs) was deleted 2026-05-14 — see git history — once the
//! dead-code sweep confirmed the only live consumer was
//! `ops/service_options.rs`, which talks directly to sqlx through the
//! `SemOsVerbOp` boundary rather than the repository pattern.

use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

/// Canonical source-kind values for service options and resource attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceKind {
    Derived,
    CbuProfile,
    InstrumentMatrix,
    LegalEntity,
    Document,
    ProductOption,
    Manual,
    OptionBinding,
}

impl TryFrom<&str> for SourceKind {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "derived" => Ok(SourceKind::Derived),
            "cbu_profile" => Ok(SourceKind::CbuProfile),
            "instrument_matrix" => Ok(SourceKind::InstrumentMatrix),
            "legal_entity" => Ok(SourceKind::LegalEntity),
            "document" => Ok(SourceKind::Document),
            "product_option" => Ok(SourceKind::ProductOption),
            "manual" => Ok(SourceKind::Manual),
            "option_binding" => Ok(SourceKind::OptionBinding),
            other => bail!("unknown source kind: {other}"),
        }
    }
}

/// Axes along which an option can drive resource fan-out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FanoutAxis {
    None,
    Market,
    Currency,
    Counterparty,
    Account,
    Fund,
    ShareClass,
    LegalEntity,
    InstructionChannel,
    Jurisdiction,
    BookingPrincipal,
}

impl FanoutAxis {
    /// Return the database representation for this fan-out axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use sem_os_postgres::service_options::FanoutAxis;
    ///
    /// assert_eq!(FanoutAxis::Market.as_db_str(), "market");
    /// ```
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            FanoutAxis::None => "none",
            FanoutAxis::Market => "market",
            FanoutAxis::Currency => "currency",
            FanoutAxis::Counterparty => "counterparty",
            FanoutAxis::Account => "account",
            FanoutAxis::Fund => "fund",
            FanoutAxis::ShareClass => "share_class",
            FanoutAxis::LegalEntity => "legal_entity",
            FanoutAxis::InstructionChannel => "instruction_channel",
            FanoutAxis::Jurisdiction => "jurisdiction",
            FanoutAxis::BookingPrincipal => "booking_principal",
        }
    }
}

impl TryFrom<&str> for FanoutAxis {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "none" => Ok(FanoutAxis::None),
            "market" => Ok(FanoutAxis::Market),
            "currency" => Ok(FanoutAxis::Currency),
            "counterparty" => Ok(FanoutAxis::Counterparty),
            "account" => Ok(FanoutAxis::Account),
            "fund" => Ok(FanoutAxis::Fund),
            "share_class" => Ok(FanoutAxis::ShareClass),
            "legal_entity" => Ok(FanoutAxis::LegalEntity),
            "instruction_channel" => Ok(FanoutAxis::InstructionChannel),
            "jurisdiction" => Ok(FanoutAxis::Jurisdiction),
            "booking_principal" => Ok(FanoutAxis::BookingPrincipal),
            other => bail!("unknown fanout axis: {other}"),
        }
    }
}

/// Resource fan-out materialisation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FanoutMode {
    PerValue,
    Shared,
    Grouped,
    Conditional,
}

impl TryFrom<&str> for FanoutMode {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "per_value" => Ok(FanoutMode::PerValue),
            "shared" => Ok(FanoutMode::Shared),
            "grouped" => Ok(FanoutMode::Grouped),
            "conditional" => Ok(FanoutMode::Conditional),
            other => bail!("unknown fanout mode: {other}"),
        }
    }
}

/// Resource fan-out rule row. Drives the `ResourceFanoutPlanner`.
///
/// Fields `group_by_policy` and `priority` are loaded from the database
/// (so the `FromRow` shape matches the schema) but not yet read by the
/// v1 pure planner — Grouped/Conditional fan-out modes that consume
/// them are explicitly out of scope for v1.
#[derive(Debug, Clone, FromRow)]
pub(crate) struct ResourceFanoutRuleRow {
    pub(crate) fanout_rule_id: Uuid,
    pub(crate) service_id: Uuid,
    pub(crate) resource_id: Uuid,
    pub(crate) service_option_def_id: Option<Uuid>,
    pub(crate) fanout_axis: String,
    pub(crate) fanout_mode: String,
    #[allow(dead_code)]
    pub(crate) group_by_policy: Value,
    pub(crate) shared_when_null: bool,
    #[allow(dead_code)]
    pub(crate) priority: i32,
    pub(crate) is_active: bool,
}

/// Resolved option value ready to become a binding row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedOptionValue {
    pub(crate) service_option_def_id: Uuid,
    pub(crate) option_key: String,
    pub(crate) value: Value,
    pub(crate) source_kind: SourceKind,
    pub(crate) source_ref: Option<Value>,
    pub(crate) source_version: Option<String>,
    pub(crate) value_hash: String,
}

/// Planned resource instance before materialisation through `service-resource.provision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedResourceInstance {
    pub(crate) service_id: Uuid,
    pub(crate) resource_id: Uuid,
    pub(crate) fanout_axis: FanoutAxis,
    pub(crate) fanout_value: Option<Value>,
}

/// Pure resource fan-out planner.
#[derive(Debug, Default, Clone)]
pub(crate) struct ResourceFanoutPlanner;

impl ResourceFanoutPlanner {
    /// Create a fan-out planner.
    ///
    /// # Examples
    ///
    /// ```
    /// let _planner = sem_os_postgres::service_options::ResourceFanoutPlanner::new();
    /// ```
    pub(crate) fn new() -> Self {
        Self
    }

    /// Build planned resource instances from fan-out rules and resolved option bindings.
    ///
    /// # Examples
    ///
    /// ```
    /// let planner = sem_os_postgres::service_options::ResourceFanoutPlanner::new();
    /// let planned = planner.plan(&[], &[]).unwrap();
    /// assert!(planned.is_empty());
    /// ```
    pub(crate) fn plan(
        &self,
        rules: &[ResourceFanoutRuleRow],
        bindings: &[ResolvedOptionValue],
    ) -> Result<Vec<PlannedResourceInstance>> {
        let mut planned = Vec::new();

        for rule in rules.iter().filter(|rule| rule.is_active) {
            let axis = FanoutAxis::try_from(rule.fanout_axis.as_str())?;
            let mode = FanoutMode::try_from(rule.fanout_mode.as_str())?;

            match mode {
                FanoutMode::Shared => {
                    planned.push(PlannedResourceInstance {
                        service_id: rule.service_id,
                        resource_id: rule.resource_id,
                        fanout_axis: axis,
                        fanout_value: None,
                    });
                }
                FanoutMode::PerValue => {
                    let Some(option_def_id) = rule.service_option_def_id else {
                        bail!("per_value fanout rule requires service_option_def_id");
                    };
                    let binding = bindings
                        .iter()
                        .find(|binding| binding.service_option_def_id == option_def_id)
                        .ok_or_else(|| {
                            anyhow!("missing binding for fanout rule {}", rule.fanout_rule_id)
                        })?;
                    for value in fanout_values(&binding.value, rule.shared_when_null) {
                        planned.push(PlannedResourceInstance {
                            service_id: rule.service_id,
                            resource_id: rule.resource_id,
                            fanout_axis: axis,
                            fanout_value: value,
                        });
                    }
                }
                FanoutMode::Grouped | FanoutMode::Conditional => {
                    bail!(
                        "{} fanout requires policy execution and is not implemented in the pure v1 planner",
                        rule.fanout_mode
                    );
                }
            }
        }

        Ok(planned)
    }
}

/// Compute SHA-256 over canonical JSON.
///
/// # Examples
///
/// ```
/// use sem_os_postgres::service_options::hash_canonical_json;
/// use serde_json::json;
///
/// let left = json!({"b": 2, "a": 1});
/// let right = json!({"a": 1, "b": 2});
/// assert_eq!(hash_canonical_json(&left), hash_canonical_json(&right));
/// ```
pub(crate) fn hash_canonical_json(value: &Value) -> String {
    let canonical = canonical_json(value);
    let digest = Sha256::digest(canonical.as_bytes());
    hex::encode(digest)
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).expect("string serialization"),
        Value::Array(values) => {
            let rendered: Vec<String> = values.iter().map(canonical_json).collect();
            format!("[{}]", rendered.join(","))
        }
        Value::Object(map) => canonical_object(map),
    }
}

fn canonical_object(map: &Map<String, Value>) -> String {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    let fields: Vec<String> = keys
        .into_iter()
        .map(|key| {
            let key_json = serde_json::to_string(key).expect("object key serialization");
            let value_json = canonical_json(&map[key]);
            format!("{key_json}:{value_json}")
        })
        .collect();
    format!("{{{}}}", fields.join(","))
}

fn fanout_values(value: &Value, shared_when_null: bool) -> Vec<Option<Value>> {
    match value {
        Value::Null if shared_when_null => vec![None],
        Value::Null => Vec::new(),
        Value::Array(values) => values.iter().cloned().map(Some).collect(),
        other => vec![Some(other.clone())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fanout_rule(option_id: Uuid, mode: &str) -> ResourceFanoutRuleRow {
        ResourceFanoutRuleRow {
            fanout_rule_id: Uuid::new_v4(),
            service_id: Uuid::new_v4(),
            resource_id: Uuid::new_v4(),
            service_option_def_id: Some(option_id),
            fanout_axis: "market".to_string(),
            fanout_mode: mode.to_string(),
            group_by_policy: json!({}),
            shared_when_null: true,
            priority: 100,
            is_active: true,
        }
    }

    #[test]
    fn canonical_hash_is_key_order_stable() {
        let left = json!({"b": 2, "a": {"d": 4, "c": 3}});
        let right = json!({"a": {"c": 3, "d": 4}, "b": 2});
        assert_eq!(hash_canonical_json(&left), hash_canonical_json(&right));
    }

    #[test]
    fn canonical_hash_changes_with_value() {
        let left = json!({"a": 1});
        let right = json!({"a": 2});
        assert_ne!(hash_canonical_json(&left), hash_canonical_json(&right));
    }

    #[test]
    fn fanout_planner_expands_array_values() {
        let option_id = Uuid::new_v4();
        let binding = ResolvedOptionValue {
            service_option_def_id: option_id,
            option_key: "markets".to_string(),
            value: json!(["US_EQUITY", "EU_EQUITY"]),
            source_kind: SourceKind::CbuProfile,
            source_ref: None,
            source_version: None,
            value_hash: hash_canonical_json(&json!(["US_EQUITY", "EU_EQUITY"])),
        };
        let rule = fanout_rule(option_id, "per_value");

        let planned = ResourceFanoutPlanner::new()
            .plan(&[rule], &[binding])
            .expect("plans fanout");

        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].fanout_axis, FanoutAxis::Market);
        assert_eq!(planned[0].fanout_value, Some(json!("US_EQUITY")));
        assert_eq!(planned[1].fanout_value, Some(json!("EU_EQUITY")));
    }

    #[test]
    fn shared_fanout_emits_single_shared_instance() {
        let rule = ResourceFanoutRuleRow {
            fanout_mode: "shared".to_string(),
            service_option_def_id: None,
            ..fanout_rule(Uuid::new_v4(), "shared")
        };

        let planned = ResourceFanoutPlanner::new()
            .plan(&[rule], &[])
            .expect("plans shared resource");

        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].fanout_value, None);
    }
}
