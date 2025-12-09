//! Agent Context Builder
//!
//! Builds dynamic context for LLM-based DSL generation, incorporating:
//! - Executed bindings from session history
//! - Bootstrap hints when no CBU exists
//! - Dataflow-aware suggestions
//!
//! This replaces hardcoded examples in prompts with context-aware guidance.

use std::collections::HashMap;
use uuid::Uuid;

use crate::dsl_v2::BindingContext;

/// Context for agent DSL generation
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Executed bindings available for reference
    pub bindings: Vec<BindingDisplay>,
    /// Whether this is a fresh session with no CBU
    pub needs_bootstrap: bool,
    /// The active CBU ID if one exists
    pub cbu_id: Option<Uuid>,
    /// Suggested next actions based on current state
    pub suggestions: Vec<String>,
}

/// A binding formatted for LLM display
#[derive(Debug, Clone)]
pub struct BindingDisplay {
    /// The binding name (without @)
    pub name: String,
    /// The type: "cbu", "entity", "case", etc.
    pub binding_type: String,
    /// Optional subtype: "proper_person", "limited_company"
    pub subtype: Option<String>,
    /// Human-readable display name
    pub display_name: Option<String>,
    /// The UUID if resolved
    pub uuid: Option<Uuid>,
}

impl BindingDisplay {
    /// Format for LLM context: "@fund (cbu)" or "@john (entity/proper_person: John Smith)"
    pub fn format_for_llm(&self) -> String {
        let type_str = match &self.subtype {
            Some(sub) => format!("{}/{}", self.binding_type, sub),
            None => self.binding_type.clone(),
        };

        match &self.display_name {
            Some(name) => format!("@{} ({}: {})", self.name, type_str, name),
            None => format!("@{} ({})", self.name, type_str),
        }
    }
}

/// Builder for agent context
pub struct AgentContextBuilder {
    bindings: Vec<BindingDisplay>,
    cbu_id: Option<Uuid>,
}

impl AgentContextBuilder {
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
            cbu_id: None,
        }
    }

    /// Add bindings from a BindingContext (from topo_sort/validation)
    pub fn with_binding_context(mut self, ctx: &BindingContext) -> Self {
        for info in ctx.all() {
            self.bindings.push(BindingDisplay {
                name: info.name.clone(),
                binding_type: info.produced_type.clone(),
                subtype: info.subtype.clone(),
                display_name: None,
                uuid: if info.entity_pk != Uuid::nil() {
                    Some(info.entity_pk)
                } else {
                    None
                },
            });

            // Track CBU binding
            if info.produced_type == "cbu" && info.entity_pk != Uuid::nil() {
                self.cbu_id = Some(info.entity_pk);
            }
        }
        self
    }

    /// Add bindings from a simple name->UUID map (from session)
    pub fn with_bindings_map(mut self, bindings: &HashMap<String, Uuid>) -> Self {
        for (name, uuid) in bindings {
            // Infer type from name patterns
            let (binding_type, subtype) = infer_binding_type(name);

            self.bindings.push(BindingDisplay {
                name: name.clone(),
                binding_type: binding_type.clone(),
                subtype,
                display_name: None,
                uuid: Some(*uuid),
            });

            if binding_type == "cbu" {
                self.cbu_id = Some(*uuid);
            }
        }
        self
    }

    /// Set the active CBU ID explicitly
    pub fn with_cbu_id(mut self, cbu_id: Option<Uuid>) -> Self {
        self.cbu_id = cbu_id;
        self
    }

    /// Build the final context
    pub fn build(self) -> AgentContext {
        let has_cbu =
            self.cbu_id.is_some() || self.bindings.iter().any(|b| b.binding_type == "cbu");

        let needs_bootstrap = !has_cbu;

        let suggestions = self.generate_suggestions(needs_bootstrap);

        AgentContext {
            bindings: self.bindings,
            needs_bootstrap,
            cbu_id: self.cbu_id,
            suggestions,
        }
    }

    fn generate_suggestions(&self, needs_bootstrap: bool) -> Vec<String> {
        let mut suggestions = Vec::new();

        if needs_bootstrap {
            suggestions.push(
                "Start with: (cbu.ensure :name \"...\" :jurisdiction \"XX\" :as @fund)".to_string(),
            );
            suggestions.push("All other commands require a CBU binding first.".to_string());
        } else {
            // Suggest based on what bindings exist
            let has_entity = self.bindings.iter().any(|b| b.binding_type == "entity");
            let has_case = self.bindings.iter().any(|b| b.binding_type == "case");

            if !has_entity {
                suggestions.push(
                    "Add entities: entity.create-proper-person or entity.create-limited-company"
                        .to_string(),
                );
            }

            if has_entity && !has_case {
                suggestions
                    .push("Start KYC: kyc-case.create then entity-workstream.create".to_string());
            }
        }

        suggestions
    }
}

impl Default for AgentContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentContext {
    /// Format bindings for LLM prompt injection
    pub fn format_bindings_for_llm(&self) -> String {
        if self.bindings.is_empty() {
            return String::new();
        }

        let formatted: Vec<String> = self.bindings.iter().map(|b| b.format_for_llm()).collect();

        format!(
            "[SESSION CONTEXT: Available references: {}. Use these exact @names when referring to these entities.]",
            formatted.join(", ")
        )
    }

    /// Generate bootstrap instruction for new CBU mode
    pub fn format_bootstrap_hint(&self) -> Option<String> {
        if !self.needs_bootstrap {
            return None;
        }

        Some(
            r#"## NEW CBU MODE

No CBU exists yet. You MUST start with:
```
(cbu.ensure :name "Fund Name" :jurisdiction "XX" :as @fund)
```

All other commands depend on having a CBU binding. Do NOT reference @cbu or any other binding until it is created."#
                .to_string(),
        )
    }

    /// Get full context string for agent prompt
    pub fn to_prompt_context(&self) -> String {
        let mut parts = Vec::new();

        // Bootstrap hint if needed
        if let Some(hint) = self.format_bootstrap_hint() {
            parts.push(hint);
        }

        // Available bindings
        let bindings_str = self.format_bindings_for_llm();
        if !bindings_str.is_empty() {
            parts.push(bindings_str);
        }

        // Suggestions
        if !self.suggestions.is_empty() {
            let suggestions_str = format!("[SUGGESTIONS: {}]", self.suggestions.join(" | "));
            parts.push(suggestions_str);
        }

        parts.join("\n\n")
    }
}

/// Infer binding type from common naming patterns
fn infer_binding_type(name: &str) -> (String, Option<String>) {
    let lower = name.to_lowercase();

    // CBU patterns
    if lower.contains("fund")
        || lower.contains("cbu")
        || lower == "client"
        || lower.ends_with("_cbu")
    {
        return ("cbu".to_string(), None);
    }

    // Case patterns
    if lower.contains("case") || lower.starts_with("kyc") {
        return ("case".to_string(), None);
    }

    // Workstream patterns
    if lower.contains("workstream") || lower.starts_with("ws") {
        return ("workstream".to_string(), None);
    }

    // Person patterns
    if lower.contains("person")
        || lower.contains("john")
        || lower.contains("jane")
        || lower.contains("ubo")
        || lower.contains("director")
    {
        return ("entity".to_string(), Some("proper_person".to_string()));
    }

    // Company patterns
    if lower.contains("company")
        || lower.contains("corp")
        || lower.contains("ltd")
        || lower.contains("holdings")
    {
        return ("entity".to_string(), Some("limited_company".to_string()));
    }

    // Default to entity
    ("entity".to_string(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_context_needs_bootstrap() {
        let ctx = AgentContextBuilder::new().build();
        assert!(ctx.needs_bootstrap);
        assert!(ctx.format_bootstrap_hint().is_some());
    }

    #[test]
    fn test_context_with_cbu_no_bootstrap() {
        let mut bindings = HashMap::new();
        bindings.insert("fund".to_string(), Uuid::new_v4());

        let ctx = AgentContextBuilder::new()
            .with_bindings_map(&bindings)
            .build();

        assert!(!ctx.needs_bootstrap);
        assert!(ctx.format_bootstrap_hint().is_none());
    }

    #[test]
    fn test_binding_display_format() {
        let binding = BindingDisplay {
            name: "john".to_string(),
            binding_type: "entity".to_string(),
            subtype: Some("proper_person".to_string()),
            display_name: Some("John Smith".to_string()),
            uuid: Some(Uuid::new_v4()),
        };

        let formatted = binding.format_for_llm();
        assert!(formatted.contains("@john"));
        assert!(formatted.contains("entity/proper_person"));
        assert!(formatted.contains("John Smith"));
    }

    #[test]
    fn test_infer_binding_type() {
        assert_eq!(infer_binding_type("fund"), ("cbu".to_string(), None));
        assert_eq!(infer_binding_type("my_cbu"), ("cbu".to_string(), None));
        assert_eq!(infer_binding_type("kyc_case"), ("case".to_string(), None));
        assert_eq!(
            infer_binding_type("john"),
            ("entity".to_string(), Some("proper_person".to_string()))
        );
        assert_eq!(
            infer_binding_type("acme_corp"),
            ("entity".to_string(), Some("limited_company".to_string()))
        );
    }
}
