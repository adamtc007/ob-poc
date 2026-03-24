use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use dsl_core::config::loader::ConfigLoader;
use sem_os_obpoc_adapter::{
    scanner::generate_seed_domain_macro_calls, seeds::core_derivation_specs,
};

use sem_os_core::derivation_spec::DerivationExpression;

pub(crate) fn try_expand_attribute_seed_macro(
    macro_fqn: &str,
    args: &BTreeMap<String, String>,
) -> Option<Result<Vec<String>>> {
    match macro_fqn {
        "attribute.seed-domain" => Some(expand_seed_domain(args)),
        "attribute.seed-derived" => Some(expand_seed_derived(args)),
        _ => None,
    }
}

fn expand_seed_domain(args: &BTreeMap<String, String>) -> Result<Vec<String>> {
    let domain = args
        .get("domain")
        .map(String::as_str)
        .ok_or_else(|| anyhow!("attribute.seed-domain requires :domain"))?;
    let verbs = ConfigLoader::from_env()
        .load_verbs()
        .context("Failed to load verb config for attribute.seed-domain")?;
    Ok(generate_seed_domain_macro_calls(&verbs, domain))
}

fn expand_seed_derived(args: &BTreeMap<String, String>) -> Result<Vec<String>> {
    let domain = args
        .get("domain")
        .map(String::as_str)
        .ok_or_else(|| anyhow!("attribute.seed-derived requires :domain"))?;

    let mut calls = Vec::new();
    for spec in core_derivation_specs()
        .into_iter()
        .filter(|spec| spec.fqn.starts_with(&format!("{domain}.")))
    {
        let function_name = match &spec.expression {
            DerivationExpression::FunctionRef { ref_name } => ref_name.as_str(),
        };
        let category = inferred_category_for_domain(domain);
        let value_type = infer_value_type(&spec.output_attribute_fqn);
        let inputs = serde_json::to_string(&spec.inputs)?;
        let escaped_name = escape_dsl_string(&spec.name);
        let escaped_json = escape_dsl_string(&inputs);
        let evidence_grade = spec.evidence_grade.to_string();
        let description = escape_dsl_string(&spec.description);

        let mut statement = format!(
            "(attribute.define-derived :id \"{}\" :display-name \"{}\" :category \"{}\" :value-type \"{}\" :domain \"{}\" :semos-description \"{}\" :derivation-function \"{}\" :derivation-inputs \"{}\" :evidence-grade \"{}\"",
            spec.fqn,
            escaped_name,
            category,
            value_type,
            domain,
            description,
            function_name,
            escaped_json,
            evidence_grade
        );

        if let Some(rule) = spec.freshness_rule {
            statement.push_str(&format!(" :freshness-seconds {}", rule.max_age_seconds));
        }
        statement.push(')');
        calls.push(statement);
    }

    Ok(calls)
}

fn infer_value_type(output_attribute_fqn: &str) -> &'static str {
    if output_attribute_fqn.ends_with("_flag") {
        "boolean"
    } else if output_attribute_fqn.ends_with("_pct")
        || output_attribute_fqn.ends_with("_percentage")
        || output_attribute_fqn.ends_with("_value")
    {
        "number"
    } else {
        "string"
    }
}

fn inferred_category_for_domain(domain: &str) -> &'static str {
    match domain {
        "ubo" => "ubo",
        "risk" => "risk",
        "trading" => "financial",
        "kyc" => "compliance",
        "entity" => "entity",
        "fund" => "fund",
        _ => "compliance",
    }
}

fn escape_dsl_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
