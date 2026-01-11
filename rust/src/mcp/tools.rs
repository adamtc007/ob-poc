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
            description: r#"Manage conversation session state and stage focus.

Actions:
- create: Start new session
- get: Get current state (bindings, stage focus)
- update: Add bindings
- undo: Revert last execution
- clear: Reset all bindings
- set_stage_focus: Focus on a semantic stage (filters available verbs)
- list_stages: List available stages

Stage focus enables "research mode" - set stage_code to:
- GLEIF_RESEARCH: GLEIF API lookups (search, enrich, get-parent, etc.)
- UBO_ANALYSIS: Ownership tracing and UBO registration
- ENTITY_ENRICHMENT: External registry enrichment
- GRAPH_EXPLORATION: Navigate entity relationship graph
- (or any onboarding stage like KYC_REVIEW, SETTLEMENT_INSTRUCTIONS, etc.)"#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["create", "get", "update", "undo", "clear", "set_stage_focus", "list_stages"],
                        "description": "Session action"
                    },
                    "session_id": {
                        "type": "string",
                        "description": "Session ID (required for get/update/undo/clear/set_stage_focus)"
                    },
                    "bindings": {
                        "type": "object",
                        "description": "For update action: name → uuid mappings to add"
                    },
                    "stage_code": {
                        "type": "string",
                        "description": "For set_stage_focus: stage code (e.g., 'GLEIF_RESEARCH'). Pass null to clear focus."
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
        // Resolution sub-session tools
        Tool {
            name: "resolution_start".into(),
            description: r#"Start a resolution sub-session to disambiguate entity references.

Called when DSL validation finds ambiguous entity references that need user input.
Creates a child session that inherits parent bindings and tracks resolution state.

Returns the sub-session ID and list of unresolved references with initial matches."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Parent session ID"
                    },
                    "unresolved_refs": {
                        "type": "array",
                        "description": "List of unresolved entity references",
                        "items": {
                            "type": "object",
                            "properties": {
                                "ref_id": {
                                    "type": "string",
                                    "description": "Unique identifier for this reference in the DSL"
                                },
                                "search_value": {
                                    "type": "string",
                                    "description": "The name/value that needs resolution"
                                },
                                "entity_type": {
                                    "type": "string",
                                    "description": "Expected entity type (person, company, cbu, etc.)"
                                },
                                "dsl_line": {
                                    "type": "integer",
                                    "description": "Line number in DSL source"
                                },
                                "initial_matches": {
                                    "type": "array",
                                    "description": "Initial search matches",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "value": { "type": "string", "description": "Entity UUID" },
                                            "display": { "type": "string", "description": "Display name" },
                                            "score_pct": { "type": "integer", "description": "Match score 0-100" },
                                            "detail": { "type": "string", "description": "Additional context" }
                                        }
                                    }
                                }
                            },
                            "required": ["ref_id", "search_value", "entity_type"]
                        }
                    },
                    "parent_dsl_index": {
                        "type": "integer",
                        "default": 0,
                        "description": "Index into parent's assembled_dsl that triggered resolution"
                    }
                },
                "required": ["session_id", "unresolved_refs"]
            }),
        },
        Tool {
            name: "resolution_search".into(),
            description: r#"Refine search for current unresolved reference using discriminators.

Use when user provides additional information like:
- Nationality: "the British one", "UK citizen"
- Date of birth: "born 1965", "DOB March 1980"
- Role: "the director", "who is UBO"
- Association: "at BlackRock", "works for Acme"

Returns updated match list filtered by discriminators."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subsession_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Resolution sub-session ID"
                    },
                    "discriminators": {
                        "type": "object",
                        "description": "Discriminating attributes to refine search",
                        "properties": {
                            "nationality": { "type": "string", "description": "Nationality code (e.g., 'GB', 'US')" },
                            "dob_year": { "type": "integer", "description": "Year of birth" },
                            "dob": { "type": "string", "description": "Full date of birth (YYYY-MM-DD)" },
                            "role": { "type": "string", "description": "Role (e.g., 'DIRECTOR', 'UBO')" },
                            "associated_entity": { "type": "string", "description": "Name of associated entity" },
                            "jurisdiction": { "type": "string", "description": "Jurisdiction code" }
                        }
                    },
                    "natural_language": {
                        "type": "string",
                        "description": "Natural language refinement (will be parsed for discriminators)"
                    }
                },
                "required": ["subsession_id"]
            }),
        },
        Tool {
            name: "resolution_select".into(),
            description: r#"Select a match to resolve the current entity reference.

Called when user confirms which entity they meant. Records the resolution
and advances to the next unresolved reference if any.

Returns updated state showing next reference or completion status."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subsession_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Resolution sub-session ID"
                    },
                    "selection": {
                        "type": "integer",
                        "description": "Index of selected match (0-based)"
                    },
                    "entity_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Direct entity UUID (alternative to selection index)"
                    }
                },
                "required": ["subsession_id"]
            }),
        },
        Tool {
            name: "resolution_complete".into(),
            description: r#"Complete the resolution sub-session and apply resolutions to parent.

Called when all references are resolved or user wants to finish early.
Merges resolved bindings into parent session and returns to normal flow."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subsession_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Resolution sub-session ID"
                    },
                    "apply": {
                        "type": "boolean",
                        "default": true,
                        "description": "Whether to apply resolutions to parent (false to discard)"
                    }
                },
                "required": ["subsession_id"]
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
        // Template tools
        Tool {
            name: "template_list".into(),
            description: r#"List available DSL templates, optionally filtered.

Templates are pre-built DSL patterns for common operations like:
- Adding directors, signatories, UBOs
- Running screening, reviewing hits
- Document cataloging and extraction
- KYC case management

Use to discover what operations are available."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "workflow": {
                        "type": "string",
                        "description": "Filter by workflow (e.g., kyc_onboarding)"
                    },
                    "state": {
                        "type": "string",
                        "description": "Filter by workflow state (e.g., ENTITY_COLLECTION)"
                    },
                    "blocker": {
                        "type": "string",
                        "description": "Find templates that resolve this blocker type (e.g., missing_role:DIRECTOR)"
                    },
                    "tag": {
                        "type": "string",
                        "description": "Filter by tag (e.g., director, ubo, screening)"
                    },
                    "search": {
                        "type": "string",
                        "description": "Search in name, description, tags"
                    }
                }
            }),
        },
        Tool {
            name: "template_get".into(),
            description: r#"Get full template details including:
- When to use / when not to use
- Required parameters with types and examples
- Effects and next steps
- The DSL pattern that will be generated

Use before calling template_expand to understand what a template does."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "template_id": {
                        "type": "string",
                        "description": "Template ID (e.g., onboard-director, review-screening-hit)"
                    }
                },
                "required": ["template_id"]
            }),
        },
        Tool {
            name: "template_expand".into(),
            description: r#"Expand a template to DSL source text.

Substitutes parameters and returns reviewable DSL code.
Parameters are resolved in order:
1. Explicit params you provide
2. Session context (current_cbu, current_case)
3. Default values from template

Returns:
- dsl: The expanded DSL source code
- missing_params: Any required params still needed
- prompt: Human-readable prompt for missing params

Use this to generate DSL from templates, then review/edit before execution."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "template_id": {
                        "type": "string",
                        "description": "Template ID to expand"
                    },
                    "params": {
                        "type": "object",
                        "description": "Parameter values to substitute"
                    },
                    "cbu_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Current CBU context (for session params)"
                    },
                    "case_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Current KYC case context (for session params)"
                    }
                },
                "required": ["template_id"]
            }),
        },
        // =====================================================================
        // Template Batch Execution Tools
        // Agent's working memory for template-driven bulk operations
        // =====================================================================
        Tool {
            name: "batch_start".into(),
            description: r#"Start a template batch execution session.

Call this when user wants bulk operations like "onboard all Allianz funds as CBUs".
This:
1. Sets session mode to TemplateExpansion
2. Loads template definition with entity dependencies
3. Returns the params you need to collect (batch vs shared)

Next steps:
- Use entity_search to find entities for each param
- Use batch_add_entities to add them to key sets
- Use batch_confirm_keyset when a param's entities are complete"#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "template_id": {
                        "type": "string",
                        "description": "Template to use (e.g., onboard-fund-cbu)"
                    }
                },
                "required": ["session_id", "template_id"]
            }),
        },
        Tool {
            name: "batch_add_entities".into(),
            description: r#"Add resolved entities to a template parameter's key set.

Use after searching with entity_search. Adds entities to the working set
for a specific parameter.

For batch params: Add multiple entities (these are what we iterate over)
For shared params: Add single entity (same for all batch items)"#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "param_name": {
                        "type": "string",
                        "description": "Parameter name from template (e.g., fund_entity, manco_entity)"
                    },
                    "entities": {
                        "type": "array",
                        "description": "Entities to add",
                        "items": {
                            "type": "object",
                            "properties": {
                                "entity_id": {
                                    "type": "string",
                                    "format": "uuid",
                                    "description": "Entity UUID"
                                },
                                "display_name": {
                                    "type": "string",
                                    "description": "Display name"
                                },
                                "entity_type": {
                                    "type": "string",
                                    "description": "Entity type (fund, limited_company, etc.)"
                                }
                            },
                            "required": ["entity_id", "display_name", "entity_type"]
                        }
                    },
                    "filter_description": {
                        "type": "string",
                        "description": "How these entities were found (for audit trail)"
                    }
                },
                "required": ["session_id", "param_name", "entities"]
            }),
        },
        Tool {
            name: "batch_confirm_keyset".into(),
            description: r#"Mark a parameter's key set as complete.

Call after user confirms the entity list for a parameter.
Once all required key sets are confirmed, session moves to ReviewingKeySets phase."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "param_name": {
                        "type": "string",
                        "description": "Parameter name to confirm"
                    }
                },
                "required": ["session_id", "param_name"]
            }),
        },
        Tool {
            name: "batch_set_scalar".into(),
            description: r#"Set a scalar (non-entity) parameter value.

For parameters like jurisdiction, dates, or other simple values
that aren't entity references."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "param_name": {
                        "type": "string",
                        "description": "Parameter name"
                    },
                    "value": {
                        "type": "string",
                        "description": "Parameter value"
                    }
                },
                "required": ["session_id", "param_name", "value"]
            }),
        },
        Tool {
            name: "batch_get_state".into(),
            description: r#"Get current template execution state.

Returns:
- phase: Current phase (SelectingTemplate, CollectingSharedParams, etc.)
- template_id: Template being used
- key_sets: Collected entities per parameter
- scalar_params: Scalar values set
- current_batch_index: Which item is being processed
- batch_results: Results from processed items
- progress: "5/15 complete" style summary

Use this to resume batch operations or display progress."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    }
                },
                "required": ["session_id"]
            }),
        },
        Tool {
            name: "batch_expand_current".into(),
            description: r#"Expand template for the current batch item.

Uses:
- Current batch item from the batch key set
- Shared entities from shared key sets
- Scalar params

Returns DSL source text ready for user review.
Does NOT execute - user must confirm first."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    }
                },
                "required": ["session_id"]
            }),
        },
        Tool {
            name: "batch_record_result".into(),
            description: r#"Record result after executing current batch item.

Call after DSL execution completes (success or failure).
Automatically advances to next pending item if success."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "success": {
                        "type": "boolean",
                        "description": "Whether execution succeeded"
                    },
                    "created_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "ID of created entity (e.g., new CBU)"
                    },
                    "error": {
                        "type": "string",
                        "description": "Error message if failed"
                    },
                    "executed_dsl": {
                        "type": "string",
                        "description": "The DSL that was executed"
                    }
                },
                "required": ["session_id", "success"]
            }),
        },
        Tool {
            name: "batch_skip_current".into(),
            description: r#"Skip the current batch item.

Marks current item as skipped and advances to next.
Use when user wants to skip an item without executing."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why the item was skipped"
                    }
                },
                "required": ["session_id"]
            }),
        },
        Tool {
            name: "batch_cancel".into(),
            description: r#"Cancel the batch operation.

Resets template execution context and returns to Chat mode.
Completed items remain executed, pending items are abandoned."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    }
                },
                "required": ["session_id"]
            }),
        },
        // =====================================================================
        // Research Macro Tools
        // LLM + web search for structured discovery with human review gate
        // =====================================================================
        Tool {
            name: "research_list".into(),
            description: r#"List available research macros.

Research macros use LLM + web search to discover structured information
about clients, entities, and regulatory status. Results require human
review before generating executable DSL.

Available macros include:
- client-discovery: Research institutional client corporate structure
- ubo-investigation: Investigate beneficial ownership chains
- regulatory-check: Check regulatory status and compliance concerns

Use search param to filter by name, description, or tags."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "search": {
                        "type": "string",
                        "description": "Search text to filter macros (name, description, tags)"
                    },
                    "tag": {
                        "type": "string",
                        "description": "Filter by specific tag (e.g., 'ubo', 'gleif', 'compliance')"
                    }
                }
            }),
        },
        Tool {
            name: "research_get".into(),
            description: r#"Get full research macro definition.

Returns:
- Parameters with types, defaults, and validation rules
- Expected output schema
- Review requirement level
- Suggested DSL verb template

Use before research_execute to understand what a macro does
and what parameters it needs."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "macro_name": {
                        "type": "string",
                        "description": "Research macro name (e.g., 'client-discovery', 'ubo-investigation')"
                    }
                },
                "required": ["macro_name"]
            }),
        },
        Tool {
            name: "research_execute".into(),
            description: r#"Execute a research macro with LLM + web search.

This initiates an LLM-powered research session that:
1. Renders the prompt template with your parameters
2. Allows the LLM to perform web searches iteratively
3. Extracts structured JSON matching the output schema
4. Validates JSON against schema
5. Optionally validates LEIs against GLEIF API
6. Returns results in PendingReview state

The result is stored in session.research.pending and requires
human review before generating DSL verbs.

Use research_status to check progress, research_approve to
approve results, or research_reject to discard."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID for state persistence"
                    },
                    "macro_name": {
                        "type": "string",
                        "description": "Research macro to execute (e.g., 'client-discovery')"
                    },
                    "params": {
                        "type": "object",
                        "description": "Parameters for the macro. Use research_get to see required params.",
                        "additionalProperties": true
                    },
                    "validate_leis": {
                        "type": "boolean",
                        "default": true,
                        "description": "Validate discovered LEIs against GLEIF API"
                    }
                },
                "required": ["session_id", "macro_name", "params"]
            }),
        },
        Tool {
            name: "research_approve".into(),
            description: r#"Approve research results and generate suggested DSL verbs.

Call after reviewing research results from research_execute.
Optionally provide edits to correct any issues before approval.

After approval:
- Results move to session.research.approved
- Suggested DSL verbs are generated from the template
- State changes to VerbsReady

Use research_status to see the generated verbs, then
review/edit before executing with dsl_execute."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "edits": {
                        "type": "object",
                        "description": "Optional JSON edits to apply before approval. Use JSON Merge Patch semantics - keys set to null are removed.",
                        "additionalProperties": true
                    },
                    "reviewer_notes": {
                        "type": "string",
                        "description": "Optional notes from the human reviewer"
                    }
                },
                "required": ["session_id"]
            }),
        },
        Tool {
            name: "research_reject".into(),
            description: r#"Reject research results.

Discards the pending research results and resets state to Idle.
Use when results are too low quality, contain hallucinated data,
or the research direction was wrong.

After rejection, you can start a new research_execute with
different parameters."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Reason for rejection (for audit trail)"
                    }
                },
                "required": ["session_id"]
            }),
        },
        Tool {
            name: "research_status".into(),
            description: r#"Get current research state for a session.

Returns:
- state: Current research state (Idle, PendingReview, VerbsReady, Executed)
- pending: If PendingReview, the research results awaiting review
- approved: Map of approved research by ID
- generated_verbs: If VerbsReady, the suggested DSL code

Use to:
- Check if there are pending results to review
- See what was approved and what verbs were generated
- Track the research workflow state"#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    }
                },
                "required": ["session_id"]
            }),
        },
        // =====================================================================
        // Taxonomy Navigation Tools
        // Entity type hierarchy browsing and navigation
        // =====================================================================
        Tool {
            name: "taxonomy_get".into(),
            description: r#"Get the entity type taxonomy tree.

Returns the full entity type hierarchy starting from root.
Each node includes:
- node_id: Unique identifier
- label: Display name (e.g., "SHELL", "LIMITED_COMPANY")
- node_type: Category (e.g., "entity_type", "category")
- children: Child nodes in the hierarchy
- entity_count: Number of entities of this type (if applicable)

Use this to understand the entity type structure before drilling in."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID (optional - for state persistence)"
                    },
                    "include_counts": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include entity counts per type"
                    }
                }
            }),
        },
        Tool {
            name: "taxonomy_drill_in".into(),
            description: r#"Drill into a taxonomy node to see its children.

Navigates deeper into the entity type hierarchy.
The new level becomes the current view in the session's taxonomy stack.

Use node_label to specify which node to drill into (e.g., "SHELL", "FUND").
Returns the children of that node."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "node_label": {
                        "type": "string",
                        "description": "Label of node to drill into (e.g., 'SHELL', 'PERSON', 'LIMITED_COMPANY')"
                    }
                },
                "required": ["session_id", "node_label"]
            }),
        },
        Tool {
            name: "taxonomy_zoom_out".into(),
            description: r#"Zoom out one level in the taxonomy hierarchy.

Returns to the parent level in the taxonomy stack.
If already at root, returns an error."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    }
                },
                "required": ["session_id"]
            }),
        },
        Tool {
            name: "taxonomy_reset".into(),
            description: r#"Reset taxonomy navigation to root level.

Clears the taxonomy stack and returns to the top of the hierarchy."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    }
                },
                "required": ["session_id"]
            }),
        },
        Tool {
            name: "taxonomy_position".into(),
            description: r#"Get current position in taxonomy navigation.

Returns:
- breadcrumbs: Path from root to current position
- depth: Current depth in hierarchy
- current_node: Details of current node
- can_zoom_out: Whether zoom out is available
- can_drill_in: Whether drilling in is available"#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    }
                },
                "required": ["session_id"]
            }),
        },
        Tool {
            name: "taxonomy_entities".into(),
            description: r#"List entities of the currently focused type.

When drilled into a specific entity type (e.g., LIMITED_COMPANY),
this returns actual entities of that type.

Supports pagination and filtering by name."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "Session ID"
                    },
                    "search": {
                        "type": "string",
                        "description": "Filter entities by name"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 20,
                        "description": "Max results to return"
                    },
                    "offset": {
                        "type": "integer",
                        "default": 0,
                        "description": "Offset for pagination"
                    }
                },
                "required": ["session_id"]
            }),
        },
        // =====================================================================
        // Trading Matrix Tools
        // =====================================================================
        Tool {
            name: "trading_matrix_get".into(),
            description: r#"Get trading matrix summary and status for a CBU.

Returns a summary of the CBU's trading configuration:
- Trading profile status (DRAFT, VALIDATED, ACTIVE, etc.)
- Universe entries count with instrument classes and markets
- SSI count and booking rules count
- Settlement chains and ISDA/CSA agreements
- Completeness indicator

Use this to understand what trading configuration exists before
using trading-profile.* verbs to create, edit, or validate.

For the full hierarchical tree, use the REST endpoint returned
in the response."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "cbu_id": {
                        "type": "string",
                        "format": "uuid",
                        "description": "CBU UUID to get trading matrix for"
                    }
                },
                "required": ["cbu_id"]
            }),
        },
        // =====================================================================
        // Feedback Inspector Tools
        // On-demand failure analysis, repro generation, and TODO creation
        // =====================================================================
        Tool {
            name: "feedback_analyze".into(),
            description: r#"Analyze failures from the event store.

Scans failure events, classifies them, computes fingerprints for
deduplication, and stores in the feedback database.

Returns:
- total_failures: Number of failure events processed
- unique_issues: Deduplicated issue count
- resolution_rate: Percentage resolved at runtime
- by_error_type: Breakdown by error classification
- top_verbs: Most failing verbs

Use since_hours to limit analysis window (default 24h)."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "since_hours": {
                        "type": "integer",
                        "default": 24,
                        "description": "Analyze failures from last N hours"
                    }
                }
            }),
        },
        Tool {
            name: "feedback_list".into(),
            description: r#"List issues with optional filtering.

Returns issue summaries with:
- fingerprint: Unique issue identifier
- error_type: Classification (TIMEOUT, ENUM_DRIFT, HANDLER_PANIC, etc.)
- status: Lifecycle state (NEW, REPRO_VERIFIED, TODO_CREATED, etc.)
- verb: The DSL verb that failed
- occurrence_count: How many times seen
- first_seen/last_seen: Temporal bounds

Filter by status, error_type, verb, or source."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["NEW", "REPRO_GENERATED", "REPRO_VERIFIED", "TODO_CREATED", "IN_PROGRESS", "RESOLVED", "WONT_FIX"],
                        "description": "Filter by issue status"
                    },
                    "error_type": {
                        "type": "string",
                        "enum": ["TIMEOUT", "RATE_LIMITED", "ENUM_DRIFT", "SCHEMA_DRIFT", "HANDLER_PANIC", "HANDLER_ERROR", "PARSE_ERROR"],
                        "description": "Filter by error type"
                    },
                    "verb": {
                        "type": "string",
                        "description": "Filter by verb name (partial match)"
                    },
                    "source": {
                        "type": "string",
                        "description": "Filter by source (gleif, bods, etc.)"
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
            name: "feedback_get".into(),
            description: r#"Get full details for a specific issue.

Returns:
- failure: Complete failure record with context
- occurrences: List of individual occurrences
- audit_trail: Full audit history

Use the fingerprint from feedback_list or feedback_analyze."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "fingerprint": {
                        "type": "string",
                        "description": "Issue fingerprint (e.g., v1:HANDLER_PANIC:gleif.fetch:...)"
                    }
                },
                "required": ["fingerprint"]
            }),
        },
        Tool {
            name: "feedback_repro".into(),
            description: r#"Generate and verify a repro test for an issue.

Creates a test file that reproduces the failure:
- GoldenJson: Expected vs actual JSON comparison
- DslScenario: DSL script that triggers the error
- UnitTest: Rust unit test

Verifies the test fails as expected, then updates issue status.

Returns:
- repro_type: Type of test generated
- path: File path of generated test
- passes: Whether verification passed (test fails = good)"#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "fingerprint": {
                        "type": "string",
                        "description": "Issue fingerprint"
                    }
                },
                "required": ["fingerprint"]
            }),
        },
        Tool {
            name: "feedback_todo".into(),
            description: r#"Generate a TODO document for an issue.

REQUIRES verified repro first - call feedback_repro before this.

Creates a structured TODO markdown file with:
- Issue summary and classification
- Repro test reference
- Suggested fix approach
- Acceptance criteria

Returns:
- todo_number: Assigned TODO number
- path: File path of generated TODO
- content: The TODO document content"#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "fingerprint": {
                        "type": "string",
                        "description": "Issue fingerprint (must have verified repro)"
                    },
                    "todo_number": {
                        "type": "integer",
                        "description": "TODO number to assign (e.g., 27 for TODO-027)"
                    }
                },
                "required": ["fingerprint", "todo_number"]
            }),
        },
        Tool {
            name: "feedback_audit".into(),
            description: r#"Get audit trail for an issue.

Returns chronological list of all actions taken:
- CAPTURED: Initial capture from event
- CLASSIFIED: Error type determined
- REPRO_GENERATED: Test file created
- REPRO_VERIFIED_FAILS: Test confirmed to fail
- TODO_CREATED: TODO document generated
- FIX_COMMITTED: Fix merged
- RESOLVED: Issue closed

Each entry includes actor, timestamp, and details."#.into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "fingerprint": {
                        "type": "string",
                        "description": "Issue fingerprint"
                    }
                },
                "required": ["fingerprint"]
            }),
        },
    ]
}
