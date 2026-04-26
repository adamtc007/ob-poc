//! Argument assembly from `OutcomeStep` into DSL-ready structured intents.

use anyhow::{anyhow, Result};
use dsl_core::config::types::{ArgType, VerbConfig};

use crate::mcp::intent_pipeline::{
    assemble_dsl_string, IntentArgValue, IntentArgument, StructuredIntent,
};

use super::outcome::OutcomeStep;

/// Assemble a DSL invocation from one `OutcomeStep` and a target verb config.
///
/// # Examples
/// ```ignore
/// use dsl_core::config::loader::ConfigLoader;
/// use ob_poc::sage::{OutcomeAction, OutcomeStep};
/// use ob_poc::sage::arg_assembly::assemble_args_from_step;
/// use std::collections::HashMap;
///
/// let config = ConfigLoader::from_env().load_verbs()?;
/// let verb = &config.domains["cbu"].verbs["create"];
/// let step = OutcomeStep {
///     action: OutcomeAction::Create,
///     target: "cbu".to_string(),
///     params: HashMap::from([
///         ("name".to_string(), "Apex Fund".to_string()),
///         ("jurisdiction".to_string(), "LU".to_string()),
///     ]),
///     notes: None,
/// };
/// let dsl = assemble_args_from_step("cbu.create", &step, verb)?;
/// assert!(dsl.starts_with("(cbu.create"));
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn assemble_args_from_step(
    verb_fqn: &str,
    step: &OutcomeStep,
    config: &VerbConfig,
) -> Result<String> {
    let intent = structured_intent_from_step(verb_fqn, step, config)?;
    assemble_dsl_string(&intent)
}

/// Assemble a structured intent from one `OutcomeStep` and a target verb config.
///
/// # Examples
/// ```ignore
/// use dsl_core::config::loader::ConfigLoader;
/// use ob_poc::sage::{OutcomeAction, OutcomeStep};
/// use ob_poc::sage::arg_assembly::structured_intent_from_step;
/// use std::collections::HashMap;
///
/// let config = ConfigLoader::from_env().load_verbs()?;
/// let verb = &config.domains["cbu"].verbs["create"];
/// let step = OutcomeStep {
///     action: OutcomeAction::Create,
///     target: "cbu".to_string(),
///     params: HashMap::from([("name".to_string(), "Apex Fund".to_string())]),
///     notes: None,
/// };
/// let intent = structured_intent_from_step("cbu.create", &step, verb)?;
/// assert_eq!(intent.verb, "cbu.create");
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn structured_intent_from_step(
    verb_fqn: &str,
    step: &OutcomeStep,
    config: &VerbConfig,
) -> Result<StructuredIntent> {
    let arguments = config
        .args
        .iter()
        .map(|arg| {
            let value = step
                .params
                .iter()
                .find_map(|(key, value)| {
                    matches_arg_name(key, &arg.name)
                        .then(|| coerce_arg_value(value, arg.arg_type, arg.lookup.is_some()))
                })
                .transpose()?
                .unwrap_or_else(|| IntentArgValue::Missing {
                    arg_name: arg.name.clone(),
                });

            Ok(IntentArgument {
                name: arg.name.clone(),
                resolved: !matches!(
                    value,
                    IntentArgValue::Unresolved { .. } | IntentArgValue::Missing { .. }
                ),
                value,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(StructuredIntent {
        verb: verb_fqn.to_string(),
        arguments,
        confidence: 1.0,
        notes: step.notes.clone().into_iter().collect(),
    })
}

fn matches_arg_name(candidate: &str, arg_name: &str) -> bool {
    candidate == arg_name || normalize_key(candidate) == normalize_key(arg_name)
}

fn normalize_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .map(|ch| ch.to_ascii_lowercase())
        .collect()
}

fn coerce_arg_value(raw: &str, arg_type: ArgType, has_lookup: bool) -> Result<IntentArgValue> {
    if let Some(reference) = raw.strip_prefix('@') {
        return Ok(IntentArgValue::Reference(reference.to_string()));
    }

    match arg_type {
        ArgType::Uuid => {
            if uuid::Uuid::parse_str(raw).is_ok() {
                Ok(IntentArgValue::Uuid(raw.to_string()))
            } else if has_lookup {
                Ok(IntentArgValue::Unresolved {
                    value: raw.to_string(),
                    entity_type: None,
                })
            } else {
                Err(anyhow!("expected uuid, got '{raw}'"))
            }
        }
        ArgType::Boolean => raw
            .parse::<bool>()
            .map(IntentArgValue::Boolean)
            .map_err(|_| anyhow!("expected boolean, got '{raw}'")),
        ArgType::Integer | ArgType::Decimal => raw
            .parse::<f64>()
            .map(IntentArgValue::Number)
            .map_err(|_| anyhow!("expected number, got '{raw}'")),
        ArgType::StringList => Ok(IntentArgValue::List(
            raw.split(',')
                .map(|item| IntentArgValue::String(item.trim().to_string()))
                .collect(),
        )),
        ArgType::UuidArray | ArgType::UuidList => Ok(IntentArgValue::List(
            raw.split(',')
                .map(|item| IntentArgValue::Uuid(item.trim().to_string()))
                .collect(),
        )),
        ArgType::Lookup if has_lookup => Ok(IntentArgValue::Unresolved {
            value: raw.to_string(),
            entity_type: None,
        }),
        _ => Ok(IntentArgValue::String(raw.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use dsl_core::config::types::{ArgConfig, VerbBehavior};

    use super::*;
    use crate::sage::OutcomeAction;

    fn sample_step() -> OutcomeStep {
        OutcomeStep {
            action: OutcomeAction::Create,
            target: "cbu".to_string(),
            params: HashMap::from([
                ("name".to_string(), "Apex Fund".to_string()),
                ("jurisdiction_code".to_string(), "LU".to_string()),
                (
                    "client-id".to_string(),
                    "123e4567-e89b-12d3-a456-426614174000".to_string(),
                ),
            ]),
            notes: Some("test".to_string()),
        }
    }

    fn sample_config() -> VerbConfig {
        VerbConfig {
            description: "Create a CBU".to_string(),
            behavior: VerbBehavior::Plugin,
            crud: None,
            handler: Some("cbu.create".to_string()),
            graph_query: None,
            durable: None,
            args: vec![
                ArgConfig {
                    name: "name".to_string(),
                    arg_type: ArgType::String,
                    required: true,
                    maps_to: None,
                    lookup: None,
                    valid_values: None,
                    default: None,
                    description: None,
                    validation: None,
                    fuzzy_check: None,
                    slot_type: None,
                    preferred_roles: vec![],
                },
                ArgConfig {
                    name: "jurisdiction-code".to_string(),
                    arg_type: ArgType::String,
                    required: true,
                    maps_to: None,
                    lookup: None,
                    valid_values: None,
                    default: None,
                    description: None,
                    validation: None,
                    fuzzy_check: None,
                    slot_type: None,
                    preferred_roles: vec![],
                },
                ArgConfig {
                    name: "client-id".to_string(),
                    arg_type: ArgType::Uuid,
                    required: false,
                    maps_to: None,
                    lookup: None,
                    valid_values: None,
                    default: None,
                    description: None,
                    validation: None,
                    fuzzy_check: None,
                    slot_type: None,
                    preferred_roles: vec![],
                },
            ],
            returns: None,
            produces: None,
            consumes: vec![],
            lifecycle: None,
            metadata: None,
            invocation_phrases: vec![],
            policy: None,
            sentences: None,
            confirm_policy: None,
            outputs: vec![],
            three_axis: None,
            transition_args: None,
        }
    }

    #[test]
    fn assemble_args_maps_exact_and_fuzzy_names() {
        let dsl = assemble_args_from_step("cbu.create", &sample_step(), &sample_config()).unwrap();
        assert_eq!(
            dsl,
            "(cbu.create :name \"Apex Fund\" :jurisdiction-code \"LU\" :client-id \"123e4567-e89b-12d3-a456-426614174000\")"
        );
    }

    #[test]
    fn assemble_args_skips_missing_optional_values() {
        let mut step = sample_step();
        step.params.remove("client-id");
        let dsl = assemble_args_from_step("cbu.create", &step, &sample_config()).unwrap();
        assert!(!dsl.contains(":client-id"));
    }

    #[test]
    fn assemble_args_errors_on_invalid_uuid_without_lookup() {
        let mut step = sample_step();
        step.params
            .insert("client-id".to_string(), "not-a-uuid".to_string());
        let error = assemble_args_from_step("cbu.create", &step, &sample_config()).unwrap_err();
        assert!(error.to_string().contains("expected uuid"));
    }
}
