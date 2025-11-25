//! LLM context export for verb schemas.
//!
//! Generates documentation suitable for RAG systems and LLM prompts.

use crate::forth_engine::schema::types::*;
use crate::forth_engine::schema::registry::VerbRegistry;

impl VerbDef {
    /// Export verb definition for LLM context.
    pub fn to_llm_context(&self) -> String {
        let mut out = format!("## {}\n\n", self.name);
        out += &format!("{}\n\n", self.description);

        out += "### Arguments\n\n";
        for arg in self.args {
            let req = match &arg.required {
                RequiredRule::Always => "**required**".to_string(),
                RequiredRule::Never => "optional".to_string(),
                RequiredRule::UnlessProvided(other) => 
                    format!("required unless `{}` provided", other),
                RequiredRule::IfEquals { arg, value } => 
                    format!("required if `{} = \"{}\"`", arg, value),
                RequiredRule::IfProvided(other) => 
                    format!("required if `{}` provided", other),
            };

            let type_str = arg.sem_type.type_name();

            out += &format!("- `{}` ({}) [{}]\n", arg.name, type_str, req);
            out += &format!("  - {}\n", arg.description);

            if let Some(default) = &arg.default {
                let default_str = match default {
                    DefaultValue::Str(s) => format!("\"{}\"", s),
                    DefaultValue::Int(i) => i.to_string(),
                    DefaultValue::Decimal(d) => d.to_string(),
                    DefaultValue::Bool(b) => b.to_string(),
                    DefaultValue::FromContext(k) => format!("from context: {}", k.env_field()),
                };
                out += &format!("  - Default: {}\n", default_str);
            }
        }

        if !self.constraints.is_empty() {
            out += "\n### Constraints\n\n";
            for c in self.constraints {
                let constraint_str = match c {
                    CrossConstraint::ExactlyOne(args) => 
                        format!("Exactly one of {:?} must be provided", args),
                    CrossConstraint::AtLeastOne(args) => 
                        format!("At least one of {:?} must be provided", args),
                    CrossConstraint::Requires { if_present, then_require } => 
                        format!("If `{}` is provided, `{}` is required", if_present, then_require),
                    CrossConstraint::Excludes { if_present, then_forbid } => 
                        format!("`{}` and `{}` cannot both be provided", if_present, then_forbid),
                    CrossConstraint::ConditionalRequired { if_arg, equals, then_require } => 
                        format!("If `{} = \"{}\"`, then `{}` is required", if_arg, equals, then_require),
                    CrossConstraint::LessThan { lesser, greater } => 
                        format!("`{}` must be less than `{}`", lesser, greater),
                };
                out += &format!("- {}\n", constraint_str);
            }
        }

        if let Some(produces) = &self.produces {
            out += "\n### Produces\n\n";
            out += &format!("- Captures: `{}` ({})\n", produces.capture_as.env_field(), produces.description);
        }

        out += "\n### Examples\n\n```clojure\n";
        for ex in self.examples {
            out += &format!("{}\n", ex);
        }
        out += "```\n";

        out
    }

    /// Export as JSON for structured context.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "domain": self.domain,
            "description": self.description,
            "crud_asset": self.crud_asset,
            "args": self.args.iter().map(|a| {
                serde_json::json!({
                    "name": a.name,
                    "type": a.sem_type.type_name(),
                    "required": match &a.required {
                        RequiredRule::Always => "always",
                        RequiredRule::Never => "never",
                        _ => "conditional",
                    },
                    "description": a.description,
                })
            }).collect::<Vec<_>>(),
            "examples": self.examples,
        })
    }
}

impl VerbRegistry {
    /// Export all verbs for LLM context.
    pub fn to_llm_context(&self) -> String {
        let mut out = String::from("# DSL Verb Reference\n\n");

        let mut domains: Vec<_> = self.domains().collect();
        domains.sort();

        for domain in domains {
            out += &format!("# {} Domain\n\n", domain.to_uppercase());

            for verb in self.get_by_domain(domain) {
                out += &verb.to_llm_context();
                out += "\n---\n\n";
            }
        }

        out
    }

    /// Export all verbs as JSON.
    pub fn to_json(&self) -> serde_json::Value {
        let mut domains_json = serde_json::Map::new();

        for domain in self.domains() {
            let verbs: Vec<_> = self.get_by_domain(domain)
                .iter()
                .map(|v| v.to_json())
                .collect();
            domains_json.insert(domain.to_string(), serde_json::Value::Array(verbs));
        }

        serde_json::json!({
            "version": "1.0",
            "domains": domains_json,
            "total_verbs": self.count(),
        })
    }

    /// Export a compact summary for system prompts.
    pub fn to_summary(&self) -> String {
        let mut out = String::from("Available DSL verbs:\n\n");

        let mut domains: Vec<_> = self.domains().collect();
        domains.sort();

        for domain in domains {
            out += &format!("**{}**: ", domain);
            let verbs: Vec<_> = self.get_by_domain(domain)
                .iter()
                .map(|v| v.name)
                .collect();
            out += &verbs.join(", ");
            out += "\n";
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forth_engine::schema::registry::VERB_REGISTRY;

    #[test]
    fn test_verb_to_llm_context() {
        let verb = VERB_REGISTRY.get("cbu.ensure").unwrap();
        let context = verb.to_llm_context();
        
        assert!(context.contains("cbu.ensure"));
        assert!(context.contains(":cbu-name"));
        assert!(context.contains("**required**"));
    }

    #[test]
    fn test_verb_to_json() {
        let verb = VERB_REGISTRY.get("cbu.ensure").unwrap();
        let json = verb.to_json();
        
        assert_eq!(json["name"], "cbu.ensure");
        assert_eq!(json["domain"], "cbu");
    }

    #[test]
    fn test_registry_to_summary() {
        let summary = VERB_REGISTRY.to_summary();
        
        assert!(summary.contains("cbu"));
        assert!(summary.contains("entity"));
        assert!(summary.contains("cbu.ensure"));
    }
}
