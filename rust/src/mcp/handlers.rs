//! MCP Tool Handlers
//!
//! Implements the business logic for each MCP tool.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::dsl_v2::{compile, parse_program, registry, DslExecutor, ExecutionContext};

use super::protocol::ToolCallResult;

/// Tool handlers with database access
pub struct ToolHandlers {
    pool: PgPool,
}

impl ToolHandlers {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Handle a tool call by name
    pub async fn handle(&self, name: &str, args: Value) -> ToolCallResult {
        match self.dispatch(name, args).await {
            Ok(v) => ToolCallResult::json(&v),
            Err(e) => ToolCallResult::error(e.to_string()),
        }
    }

    async fn dispatch(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            "dsl_validate" => self.dsl_validate(args).await,
            "dsl_execute" => self.dsl_execute(args).await,
            "dsl_plan" => self.dsl_plan(args).await,
            "cbu_get" => self.cbu_get(args).await,
            "cbu_list" => self.cbu_list(args).await,
            "entity_get" => self.entity_get(args).await,
            "verbs_list" => self.verbs_list(args),
            "schema_info" => self.schema_info(args).await,
            _ => Err(anyhow!("Unknown tool: {}", name)),
        }
    }

    /// Validate DSL source code
    async fn dsl_validate(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;

        let ast = match parse_program(source) {
            Ok(ast) => ast,
            Err(e) => {
                return Ok(json!({
                    "valid": false,
                    "errors": [{"type": "parse", "message": format!("{:?}", e)}]
                }))
            }
        };

        match compile(&ast) {
            Ok(plan) => Ok(json!({
                "valid": true,
                "step_count": plan.steps.len(),
                "steps": plan.steps.iter().enumerate().map(|(i, s)| {
                    json!({
                        "index": i,
                        "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb),
                        "binding": s.bind_as
                    })
                }).collect::<Vec<_>>()
            })),
            Err(e) => Ok(json!({
                "valid": false,
                "errors": [{"type": "compile", "message": format!("{:?}", e)}]
            })),
        }
    }

    /// Execute DSL against the database
    async fn dsl_execute(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;
        let dry_run = args["dry_run"].as_bool().unwrap_or(false);

        let ast = parse_program(source).map_err(|e| anyhow!("Parse error: {:?}", e))?;
        let plan = compile(&ast).map_err(|e| anyhow!("Compile error: {:?}", e))?;

        if dry_run {
            let steps: Vec<_> = plan
                .steps
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    json!({
                        "index": i,
                        "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb),
                        "binding": s.bind_as
                    })
                })
                .collect();
            return Ok(json!({
                "success": true,
                "dry_run": true,
                "step_count": steps.len(),
                "steps": steps
            }));
        }

        let executor = DslExecutor::new(self.pool.clone());
        let mut ctx = ExecutionContext::new();

        match executor.execute_plan(&plan, &mut ctx).await {
            Ok(results) => {
                let bindings: serde_json::Map<_, _> = ctx
                    .symbols
                    .iter()
                    .map(|(k, v)| (k.clone(), json!(v.to_string())))
                    .collect();

                Ok(json!({
                    "success": true,
                    "steps_executed": results.len(),
                    "bindings": bindings
                }))
            }
            Err(e) => Ok(json!({
                "success": false,
                "error": e.to_string(),
                "completed": ctx.symbols.len()
            })),
        }
    }

    /// Show execution plan without running
    async fn dsl_plan(&self, args: Value) -> Result<Value> {
        let source = args["source"]
            .as_str()
            .ok_or_else(|| anyhow!("source required"))?;

        let ast = parse_program(source).map_err(|e| anyhow!("Parse: {:?}", e))?;
        let plan = compile(&ast).map_err(|e| anyhow!("Compile: {:?}", e))?;

        let steps: Vec<_> = plan
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                json!({
                    "index": i,
                    "verb": format!("{}.{}", s.verb_call.domain, s.verb_call.verb),
                    "binding": s.bind_as,
                    "args": s.verb_call.arguments.iter().map(|a| {
                        json!({
                            "key": a.key.canonical(),
                            "value": format!("{:?}", a.value)
                        })
                    }).collect::<Vec<_>>()
                })
            })
            .collect();

        Ok(json!({
            "valid": true,
            "step_count": plan.steps.len(),
            "steps": steps
        }))
    }

    /// Get CBU with all related data
    async fn cbu_get(&self, args: Value) -> Result<Value> {
        let cbu_id = Uuid::parse_str(
            args["cbu_id"]
                .as_str()
                .ok_or_else(|| anyhow!("cbu_id required"))?,
        )?;

        // Get CBU
        let cbu = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction, client_type
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            cbu_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("CBU not found"))?;

        // Get entities via cbu_entity_roles junction table
        let entities = sqlx::query!(
            r#"SELECT DISTINCT e.entity_id, e.name, et.type_code as entity_type
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               WHERE cer.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Get roles
        let roles = sqlx::query!(
            r#"SELECT cer.entity_id, r.name as role_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               WHERE cer.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Get documents
        let documents = sqlx::query!(
            r#"SELECT dc.doc_id, dc.document_type_code, dc.status
               FROM "ob-poc".document_catalog dc
               WHERE dc.cbu_id = $1"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Get screenings (via entities in this CBU)
        let screenings = sqlx::query!(
            r#"SELECT s.screening_id, s.entity_id, s.screening_type, s.status, s.result
               FROM "ob-poc".screenings s
               WHERE s.entity_id IN (
                   SELECT entity_id FROM "ob-poc".cbu_entity_roles WHERE cbu_id = $1
               )"#,
            cbu_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(json!({
            "cbu": {
                "cbu_id": cbu.cbu_id.to_string(),
                "name": cbu.name,
                "client_type": cbu.client_type,
                "jurisdiction": cbu.jurisdiction
            },
            "entities": entities.iter().map(|e| json!({
                "entity_id": e.entity_id.to_string(),
                "name": e.name,
                "entity_type": e.entity_type
            })).collect::<Vec<_>>(),
            "roles": roles.iter().map(|r| json!({
                "entity_id": r.entity_id.to_string(),
                "role": r.role_name
            })).collect::<Vec<_>>(),
            "documents": documents.iter().map(|d| json!({
                "doc_id": d.doc_id.to_string(),
                "document_type": d.document_type_code,
                "status": d.status
            })).collect::<Vec<_>>(),
            "screenings": screenings.iter().map(|s| json!({
                "screening_id": s.screening_id.to_string(),
                "entity_id": s.entity_id.to_string(),
                "screening_type": s.screening_type,
                "status": s.status,
                "result": s.result
            })).collect::<Vec<_>>(),
            "summary": {
                "entities": entities.len(),
                "roles": roles.len(),
                "documents": documents.len(),
                "screenings": screenings.len()
            }
        }))
    }

    /// List CBUs with filtering
    async fn cbu_list(&self, args: Value) -> Result<Value> {
        let limit = args["limit"].as_i64().unwrap_or(20);
        let search = args["search"].as_str();

        // Use a single query with optional ILIKE
        let cbus = sqlx::query!(
            r#"SELECT cbu_id, name, client_type, jurisdiction
               FROM "ob-poc".cbus
               WHERE ($1::text IS NULL OR name ILIKE '%' || $1 || '%')
               ORDER BY name
               LIMIT $2"#,
            search,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(json!({
            "cbus": cbus.iter().map(|c| json!({
                "cbu_id": c.cbu_id.to_string(),
                "name": c.name,
                "client_type": c.client_type,
                "jurisdiction": c.jurisdiction
            })).collect::<Vec<_>>(),
            "total": cbus.len()
        }))
    }

    /// Get entity details
    async fn entity_get(&self, args: Value) -> Result<Value> {
        let entity_id = Uuid::parse_str(
            args["entity_id"]
                .as_str()
                .ok_or_else(|| anyhow!("entity_id required"))?,
        )?;

        let entity = sqlx::query!(
            r#"SELECT e.entity_id, e.name, et.type_code
               FROM "ob-poc".entities e
               JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
               WHERE e.entity_id = $1"#,
            entity_id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("Entity not found"))?;

        // Get CBUs this entity belongs to via cbu_entity_roles
        let cbus = sqlx::query!(
            r#"SELECT DISTINCT cer.cbu_id, c.name as cbu_name
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".cbus c ON cer.cbu_id = c.cbu_id
               WHERE cer.entity_id = $1"#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Get roles for this entity
        let roles = sqlx::query!(
            r#"SELECT r.name as role_name, cer.cbu_id
               FROM "ob-poc".cbu_entity_roles cer
               JOIN "ob-poc".roles r ON cer.role_id = r.role_id
               WHERE cer.entity_id = $1"#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await?;

        // Get documents via document_entity_links
        let documents = sqlx::query!(
            r#"SELECT del.doc_id, dc.document_type_code, dc.status
               FROM "ob-poc".document_entity_links del
               JOIN "ob-poc".document_catalog dc ON del.doc_id = dc.doc_id
               WHERE del.entity_id = $1"#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        // Get screenings
        let screenings = sqlx::query!(
            r#"SELECT screening_id, screening_type, status, result
               FROM "ob-poc".screenings WHERE entity_id = $1"#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(json!({
            "entity": {
                "entity_id": entity.entity_id.to_string(),
                "name": entity.name,
                "entity_type": entity.type_code
            },
            "cbus": cbus.iter().map(|c| json!({
                "cbu_id": c.cbu_id.to_string(),
                "name": c.cbu_name
            })).collect::<Vec<_>>(),
            "roles": roles.iter().map(|r| json!({
                "role": r.role_name,
                "cbu_id": r.cbu_id.to_string()
            })).collect::<Vec<_>>(),
            "documents": documents.iter().map(|d| json!({
                "doc_id": d.doc_id.to_string(),
                "document_type": d.document_type_code,
                "status": d.status
            })).collect::<Vec<_>>(),
            "screenings": screenings.iter().map(|s| json!({
                "screening_id": s.screening_id.to_string(),
                "screening_type": s.screening_type,
                "status": s.status,
                "result": s.result
            })).collect::<Vec<_>>()
        }))
    }

    /// List available DSL verbs
    fn verbs_list(&self, args: Value) -> Result<Value> {
        let domain_filter = args["domain"].as_str();
        let reg = registry();

        let verbs: Vec<_> = reg
            .all_verbs()
            .filter(|v| domain_filter.is_none_or(|d| v.domain == d))
            .map(|v| {
                json!({
                    "verb": v.full_name(),
                    "domain": v.domain,
                    "description": v.description,
                    "args": v.args.iter().map(|a| json!({
                        "name": a.name,
                        "type": a.arg_type,
                        "required": a.required
                    })).collect::<Vec<_>>()
                })
            })
            .collect();

        let domains: Vec<_> = reg.domains().to_vec();

        Ok(json!({
            "domains": domains,
            "verb_count": verbs.len(),
            "verbs": verbs
        }))
    }

    /// Get entity types, roles, document types from database
    async fn schema_info(&self, args: Value) -> Result<Value> {
        let category = args["category"].as_str().unwrap_or("all");
        let mut result = json!({});

        if category == "all" || category == "entity_types" {
            let types = sqlx::query!(
                r#"SELECT type_code, name FROM "ob-poc".entity_types ORDER BY type_code"#
            )
            .fetch_all(&self.pool)
            .await?;

            result["entity_types"] = json!(types
                .iter()
                .map(|t| json!({"code": t.type_code, "name": t.name}))
                .collect::<Vec<_>>());
        }

        if category == "all" || category == "roles" {
            let roles = sqlx::query!(r#"SELECT role_id, name FROM "ob-poc".roles ORDER BY name"#)
                .fetch_all(&self.pool)
                .await?;

            result["roles"] = json!(roles
                .iter()
                .map(|r| json!({"id": r.role_id.to_string(), "name": r.name}))
                .collect::<Vec<_>>());
        }

        if category == "all" || category == "document_types" {
            let docs = sqlx::query!(
                r#"SELECT type_code, display_name FROM "ob-poc".document_types ORDER BY type_code"#
            )
            .fetch_all(&self.pool)
            .await?;

            result["document_types"] = json!(docs
                .iter()
                .map(|d| json!({"code": d.type_code, "name": d.display_name}))
                .collect::<Vec<_>>());
        }

        Ok(result)
    }
}
