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
        Tool {
            name: "dsl_lookup".into(),
            description: "Look up real database IDs. ALWAYS use this instead of guessing UUIDs. Returns matching records with their IDs.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "lookup_type": {
                        "type": "string",
                        "enum": ["cbu", "entity", "document", "product", "service", "kyc_case"],
                        "description": "Type of record to look up"
                    },
                    "search": {
                        "type": "string",
                        "description": "Text search on name/display (case-insensitive)"
                    },
                    "filters": {
                        "type": "object",
                        "description": "Filter criteria: document_type, entity_type, status, jurisdiction, client_type, cbu_id"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 10,
                        "description": "Max results to return"
                    }
                },
                "required": ["lookup_type"]
            }),
        },
        Tool {
            name: "dsl_complete".into(),
            description: "Get completions for verbs or attributes. Use before generating DSL to get correct names.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "completion_type": {
                        "type": "string",
                        "enum": ["verb", "domain", "product", "role"],
                        "description": "What to complete"
                    },
                    "prefix": {
                        "type": "string",
                        "description": "Partial text to match (optional)"
                    },
                    "domain": {
                        "type": "string",
                        "description": "For verb completion - filter by domain"
                    }
                },
                "required": ["completion_type"]
            }),
        },
        Tool {
            name: "dsl_signature".into(),
            description: "Get verb signature - parameters, types, and requirements. Use to understand what arguments a verb needs.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "verb": {
                        "type": "string",
                        "description": "Full verb name (e.g., 'cbu.add-product', 'entity.create-proper-person')"
                    }
                },
                "required": ["verb"]
            }),
        },
    ]
}
