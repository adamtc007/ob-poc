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
                        "enum": ["cbu", "entity", "document", "product", "service", "kyc_case", "attribute"],
                        "description": "Type of record to look up. Use 'attribute' for attribute IDs (e.g., attr.identity.first_name)"
                    },
                    "search": {
                        "type": "string",
                        "description": "Text search on name/id/display (case-insensitive)"
                    },
                    "filters": {
                        "type": "object",
                        "description": "Filter criteria varies by type. For attribute: category, value_type, domain. For document: document_type, cbu_id. For entity: entity_type. For cbu: jurisdiction, client_type."
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
        Tool {
            name: "dsl_generate".into(),
            description: "Generate DSL from natural language. Extracts structured intent and assembles valid DSL code.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "instruction": {
                        "type": "string",
                        "description": "Natural language description of what to create/do (e.g., 'Create a fund in Luxembourg called Apex Capital')"
                    },
                    "domain": {
                        "type": "string",
                        "description": "Optional domain hint to focus generation (e.g., 'cbu', 'entity', 'kyc')"
                    },
                    "execute": {
                        "type": "boolean",
                        "default": false,
                        "description": "If true, execute the generated DSL immediately"
                    }
                },
                "required": ["instruction"]
            }),
        },
        Tool {
            name: "session_context".into(),
            description: "Manage conversation session state. Create session at start, tracks bindings across DSL executions. Use 'create' at conversation start, then pass session_id to dsl_execute.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["create", "get", "update", "undo", "clear"],
                        "description": "Session action: create (new session), get (current state), update (add bindings), undo (revert last execution), clear (reset all)"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session ID (required for get/update/undo/clear)"
                    },
                    "bindings": {
                        "type": "object",
                        "description": "For update action: name → uuid mappings to add"
                    }
                },
                "required": ["action"]
            }),
        },
        Tool {
            name: "entity_search".into(),
            description: r#"Search for entities with rich context for smart disambiguation.

Returns matches enriched with:
- Context (nationality, DOB, roles, ownership, jurisdiction)
- Disambiguation labels for display
- Resolution confidence and suggested action

Use conversation_hints to enable context-aware auto-resolution:
- If user mentioned "director", matches with DIRECTOR role are preferred
- If user mentioned a CBU name, entities linked to it are preferred
- If user mentioned nationality (e.g., "British"), matches are filtered

Suggested actions:
- auto_resolve: Single clear match, use it directly
- ask_user: Multiple similar matches, show disambiguation prompt
- suggest_create: No good matches, offer to create new entity"#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query (name or partial name)"
                    },
                    "entity_type": {
                        "type": "string",
                        "enum": ["cbu", "entity", "person", "company", "document", "product", "service"],
                        "description": "Filter by entity type"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 10,
                        "description": "Max results to return"
                    },
                    "conversation_hints": {
                        "type": "object",
                        "description": "Context from conversation to improve resolution",
                        "properties": {
                            "mentioned_roles": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Roles mentioned in conversation (e.g., ['DIRECTOR', 'UBO'])"
                            },
                            "mentioned_cbu": {
                                "type": "string",
                                "description": "CBU name mentioned in conversation"
                            },
                            "mentioned_nationality": {
                                "type": "string",
                                "description": "Nationality mentioned (e.g., 'US', 'GB', 'British')"
                            },
                            "mentioned_jurisdiction": {
                                "type": "string",
                                "description": "Jurisdiction mentioned (e.g., 'LU', 'Luxembourg')"
                            },
                            "current_cbu_id": {
                                "type": "string",
                                "format": "uuid",
                                "description": "Currently active CBU in the session"
                            }
                        }
                    }
                },
                "required": ["query"]
            }),
        },
        // Workflow orchestration tools
        Tool {
            name: "workflow_status".into(),
            description: "Get current workflow status, blockers, and available actions for a subject.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subject_type": {
                        "type": "string",
                        "enum": ["cbu", "entity", "case"],
                        "description": "Type of subject (cbu, entity, case)"
                    },
                    "subject_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the subject"
                    },
                    "workflow_id": {
                        "type": "string",
                        "default": "kyc_onboarding",
                        "description": "Workflow ID (default: kyc_onboarding)"
                    }
                },
                "required": ["subject_type", "subject_id"]
            }),
        },
        Tool {
            name: "workflow_advance".into(),
            description: "Try to advance workflow to next state (evaluates guards and auto-transitions).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subject_type": {
                        "type": "string",
                        "enum": ["cbu", "entity", "case"],
                        "description": "Type of subject"
                    },
                    "subject_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the subject"
                    },
                    "workflow_id": {
                        "type": "string",
                        "default": "kyc_onboarding",
                        "description": "Workflow ID"
                    }
                },
                "required": ["subject_type", "subject_id"]
            }),
        },
        Tool {
            name: "workflow_transition".into(),
            description: "Manually transition to a specific state.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subject_type": {
                        "type": "string",
                        "description": "Type of subject"
                    },
                    "subject_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the subject"
                    },
                    "workflow_id": {
                        "type": "string",
                        "default": "kyc_onboarding",
                        "description": "Workflow ID"
                    },
                    "to_state": {
                        "type": "string",
                        "description": "Target state to transition to"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Reason for the transition"
                    }
                },
                "required": ["subject_type", "subject_id", "to_state"]
            }),
        },
        Tool {
            name: "workflow_start".into(),
            description: "Start a new workflow for a subject.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workflow_id": {
                        "type": "string",
                        "default": "kyc_onboarding",
                        "description": "Workflow ID to start"
                    },
                    "subject_type": {
                        "type": "string",
                        "enum": ["cbu", "entity", "case"],
                        "description": "Type of subject"
                    },
                    "subject_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "UUID of the subject"
                    }
                },
                "required": ["workflow_id", "subject_type", "subject_id"]
            }),
        },
        Tool {
            name: "resolve_blocker".into(),
            description: "Get DSL template to resolve a specific blocker type.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "blocker_type": {
                        "type": "string",
                        "enum": ["missing_role", "missing_document", "pending_screening", "unresolved_alert", "incomplete_ownership", "unverified_ubo"],
                        "description": "Type of blocker to resolve"
                    },
                    "context": {
                        "type": "object",
                        "description": "Blocker-specific context (e.g., role name, entity_id)"
                    }
                },
                "required": ["blocker_type"]
            }),
        },
    ]
}
