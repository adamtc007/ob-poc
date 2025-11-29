//! MCP Tool Definitions
//!
//! Defines all available tools for the DSL MCP server.

use super::protocol::Tool;
use serde_json::json;

/// Get all available MCP tools
pub fn get_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "dsl_validate".into(),
            description: "Validate DSL source code. Parses and runs CSG linting.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "DSL source code to validate"
                    }
                },
                "required": ["source"]
            }),
        },
        Tool {
            name: "dsl_execute".into(),
            description: "Execute DSL against the database.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "DSL source code to execute"
                    },
                    "dry_run": {
                        "type": "boolean",
                        "default": false,
                        "description": "If true, show plan without executing"
                    }
                },
                "required": ["source"]
            }),
        },
        Tool {
            name: "dsl_plan".into(),
            description: "Show execution plan without running.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "DSL source code"
                    }
                },
                "required": ["source"]
            }),
        },
        Tool {
            name: "cbu_get".into(),
            description: "Get CBU with entities, roles, documents, screenings.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "cbu_id": {
                        "type": "string",
                        "description": "CBU UUID"
                    }
                },
                "required": ["cbu_id"]
            }),
        },
        Tool {
            name: "cbu_list".into(),
            description: "List CBUs with filtering.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["active", "pending", "closed", "deleted"],
                        "description": "Filter by status"
                    },
                    "client_type": {
                        "type": "string",
                        "description": "Filter by client type"
                    },
                    "search": {
                        "type": "string",
                        "description": "Search by name"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 20,
                        "description": "Max results to return"
                    }
                }
            }),
        },
        Tool {
            name: "entity_get".into(),
            description: "Get entity details with roles, documents, screenings.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "entity_id": {
                        "type": "string",
                        "description": "Entity UUID"
                    }
                },
                "required": ["entity_id"]
            }),
        },
        Tool {
            name: "verbs_list".into(),
            description: "List available DSL verbs.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "domain": {
                        "type": "string",
                        "description": "Filter by domain (e.g., cbu, entity, document)"
                    }
                }
            }),
        },
        Tool {
            name: "schema_info".into(),
            description: "Get entity types, roles, document types from database.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "enum": ["entity_types", "roles", "document_types", "all"],
                        "default": "all",
                        "description": "Category to retrieve"
                    }
                }
            }),
        },
    ]
}
