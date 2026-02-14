//! MCP tool definitions for the Semantic Registry.
//!
//! Bridges the internal `SemRegToolSpec` definitions to MCP `protocol::Tool` format,
//! making all sem_reg tools available through the MCP `tools/list` surface.

use serde_json::json;

use super::protocol::Tool;
use crate::sem_reg::agent::mcp_tools::{all_tool_specs, SemRegToolSpec};

/// Convert a single `SemRegToolSpec` into an MCP `Tool`.
/// Map internal parameter types to valid JSON Schema types.
fn map_param_schema(param_type: &str, description: &str) -> serde_json::Value {
    let mut schema = match param_type {
        "uuid" => json!({"type": "string", "format": "uuid"}),
        "string" => json!({"type": "string"}),
        "int" | "integer" => json!({"type": "integer"}),
        "number" => json!({"type": "number"}),
        "bool" | "boolean" => json!({"type": "boolean"}),
        "json" => json!({"type": "object"}),
        t if t.starts_with("array") => json!({"type": "array", "items": {"type": "string"}}),
        _ => json!({"type": "string"}), // safe fallback
    };
    if let Some(obj) = schema.as_object_mut() {
        obj.insert("description".into(), json!(description));
    }
    schema
}

fn spec_to_mcp_tool(spec: &SemRegToolSpec) -> Tool {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for param in &spec.parameters {
        let schema = map_param_schema(&param.param_type, &param.description);
        properties.insert(param.name.clone(), schema);
        if param.required {
            required.push(json!(param.name));
        }
    }

    Tool {
        name: spec.name.clone(),
        description: spec.description.clone(),
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required,
        }),
    }
}

/// Returns all Semantic Registry tools as MCP `Tool` definitions.
pub fn sem_reg_tools() -> Vec<Tool> {
    all_tool_specs().iter().map(spec_to_mcp_tool).collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sem_reg_tools_not_empty() {
        let tools = sem_reg_tools();
        assert!(!tools.is_empty(), "sem_reg_tools() should return at least one tool");
    }

    #[test]
    fn test_sem_reg_tools_have_valid_schemas() {
        let tools = sem_reg_tools();
        for tool in &tools {
            assert!(!tool.name.is_empty(), "Tool name should not be empty");
            assert!(!tool.description.is_empty(), "Tool {} has empty description", tool.name);
            assert!(
                tool.input_schema.is_object(),
                "Tool {} has non-object input schema",
                tool.name
            );
            let schema = tool.input_schema.as_object().unwrap();
            assert_eq!(
                schema.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "Tool {} schema type should be object",
                tool.name
            );
        }
    }

    #[test]
    fn test_sem_reg_tools_include_describe_verb() {
        let tools = sem_reg_tools();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(
            names.contains(&"sem_reg_describe_verb"),
            "Should include sem_reg_describe_verb, got: {:?}",
            names
        );
    }

    #[test]
    fn test_sem_reg_tools_no_invalid_schema_types() {
        let valid_types = ["string", "integer", "number", "boolean", "object", "array"];
        let tools = sem_reg_tools();
        for tool in &tools {
            if let Some(props) = tool.input_schema.get("properties").and_then(|p| p.as_object()) {
                for (param_name, param_schema) in props {
                    if let Some(t) = param_schema.get("type").and_then(|v| v.as_str()) {
                        assert!(
                            valid_types.contains(&t),
                            "Tool {} param {} has invalid JSON Schema type: {}",
                            tool.name, param_name, t
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_sem_reg_tools_count() {
        let tools = sem_reg_tools();
        // Should have ~29 tools from all_tool_specs()
        assert!(tools.len() >= 20, "Expected at least 20 tools, got {}", tools.len());
    }
}
