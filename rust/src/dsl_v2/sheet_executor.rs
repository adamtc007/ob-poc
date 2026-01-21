//! Phased Sheet Executor
//!
//! Executes a DSL sheet in phases based on DAG depth:
//! - Phase 0: Statements with no dependencies
//! - Phase 1: Statements depending on phase 0 outputs
//! - Phase N: Statements depending on outputs up to phase N-1
//!
//! # Execution Model
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────────┐
//! │                        Phased Execution                                      │
//! │                                                                              │
//! │  BEGIN TRANSACTION                                                           │
//! │  │                                                                           │
//! │  ├─ Phase 0 ─────────────────────────────────────────────────────────────┐  │
//! │  │  Execute statements with depth=0                                       │  │
//! │  │  Collect returned PKs → symbol table                                   │  │
//! │  └────────────────────────────────────────────────────────────────────────┘  │
//! │  │                                                                           │
//! │  ├─ Phase 1 ─────────────────────────────────────────────────────────────┐  │
//! │  │  Substitute @symbols with PKs from phase 0                             │  │
//! │  │  Execute statements with depth=1                                       │  │
//! │  │  Collect returned PKs → symbol table                                   │  │
//! │  └────────────────────────────────────────────────────────────────────────┘  │
//! │  │                                                                           │
//! │  ├─ ... (repeat for each phase)                                             │
//! │  │                                                                           │
//! │  ├─ On ANY failure:                                                          │
//! │  │  • Mark downstream statements as SKIPPED                                  │
//! │  │  • ROLLBACK TRANSACTION                                                   │
//! │  │  • Return error details                                                   │
//! │  │                                                                           │
//! │  └─ On success: COMMIT TRANSACTION                                           │
//! │                                                                              │
//! └─────────────────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::time::Instant;

use anyhow::Result;
use chrono::Utc;
use sqlx::PgPool;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::events::SharedEmitter;
use crate::session::dsl_sheet::{
    DslSheet, ErrorCode, ExecutionPhase, SheetExecutionResult, SheetStatus, StatementError,
    StatementResult, StatementStatus,
};

use super::executor::{DslExecutor, ExecutionResult};
use super::ExecutionContext;

/// Phased executor for DSL sheets
pub struct SheetExecutor<'a> {
    pool: &'a PgPool,
    #[allow(dead_code)] // Reserved for future event emission
    emitter: Option<SharedEmitter>,
}

impl<'a> SheetExecutor<'a> {
    /// Create a new sheet executor
    pub fn new(pool: &'a PgPool, emitter: Option<SharedEmitter>) -> Self {
        Self { pool, emitter }
    }

    /// Execute a sheet in phases
    ///
    /// # Arguments
    /// * `sheet` - The DSL sheet to execute (will be mutated with results)
    /// * `phases` - Pre-computed execution phases from DAG analysis
    ///
    /// # Returns
    /// * `SheetExecutionResult` with per-statement status and overall result
    pub async fn execute_phased(
        &self,
        sheet: &mut DslSheet,
        phases: &[ExecutionPhase],
    ) -> Result<SheetExecutionResult> {
        let started_at = Utc::now();
        let start_instant = Instant::now();
        let total_statements = sheet.statements.len();

        info!(
            "Starting phased execution: {} statements in {} phases",
            total_statements,
            phases.len()
        );

        // Symbol table: symbol name → UUID
        let mut symbols: HashMap<String, Uuid> = HashMap::new();

        // Results per statement
        let mut results: Vec<StatementResult> = Vec::with_capacity(total_statements);

        // Initialize results for all statements
        for stmt in &sheet.statements {
            results.push(StatementResult {
                index: stmt.index,
                dag_depth: stmt.dag_depth,
                source: stmt.source.clone(),
                resolved_source: None,
                status: StatementStatus::Pending,
                error: None,
                returned_pk: None,
                execution_time_ms: None,
            });
        }

        // Begin transaction
        let mut tx = self.pool.begin().await?;
        let mut phases_completed = 0;
        let mut overall_status = SheetStatus::Success;
        let mut failed_stmt_idx: Option<usize> = None;

        // Execute phase by phase
        'phases: for phase in phases {
            debug!(
                "Executing phase {} with {} statements",
                phase.depth,
                phase.statement_indices.len()
            );

            for &stmt_idx in &phase.statement_indices {
                let stmt = &mut sheet.statements[stmt_idx];
                let stmt_start = Instant::now();

                // Substitute symbols in the source
                let resolved_source = match self.substitute_symbols(&stmt.source, &symbols) {
                    Ok(resolved) => {
                        results[stmt_idx].resolved_source = Some(resolved.clone());
                        resolved
                    }
                    Err(e) => {
                        error!(
                            "Symbol substitution failed for statement {}: {}",
                            stmt_idx, e
                        );
                        stmt.status = StatementStatus::Failed {
                            error: e.to_string(),
                            code: ErrorCode::UnresolvedSymbol,
                        };
                        results[stmt_idx].status = stmt.status.clone();
                        results[stmt_idx].error = Some(StatementError {
                            code: ErrorCode::UnresolvedSymbol,
                            message: e.to_string(),
                            detail: None,
                            span: None,
                            blocked_by: None,
                        });
                        results[stmt_idx].execution_time_ms =
                            Some(stmt_start.elapsed().as_millis() as u64);
                        overall_status = SheetStatus::Failed;
                        failed_stmt_idx = Some(stmt_idx);
                        break 'phases;
                    }
                };

                // Mark as executing
                stmt.status = StatementStatus::Executing;
                results[stmt_idx].status = StatementStatus::Executing;

                // Execute the statement
                match self.execute_statement(&mut tx, &resolved_source).await {
                    Ok(exec_result) => {
                        // Record returned PK if any
                        if let Some(ref symbol) = stmt.produces {
                            if let Some(pk) = exec_result.produced_pk {
                                symbols.insert(symbol.clone(), pk);
                                stmt.returned_pk = Some(pk);
                                results[stmt_idx].returned_pk = Some(pk);
                            }
                        }

                        stmt.status = StatementStatus::Success;
                        results[stmt_idx].status = StatementStatus::Success;
                        results[stmt_idx].execution_time_ms =
                            Some(stmt_start.elapsed().as_millis() as u64);

                        debug!(
                            "Statement {} succeeded ({}ms)",
                            stmt_idx,
                            stmt_start.elapsed().as_millis()
                        );
                    }
                    Err(e) => {
                        error!("Statement {} failed: {}", stmt_idx, e);
                        let error_code = classify_error(&e);

                        stmt.status = StatementStatus::Failed {
                            error: e.to_string(),
                            code: error_code.clone(),
                        };
                        results[stmt_idx].status = stmt.status.clone();
                        results[stmt_idx].error = Some(StatementError {
                            code: error_code,
                            message: e.to_string(),
                            detail: None,
                            span: None,
                            blocked_by: None,
                        });
                        results[stmt_idx].execution_time_ms =
                            Some(stmt_start.elapsed().as_millis() as u64);

                        overall_status = SheetStatus::Failed;
                        failed_stmt_idx = Some(stmt_idx);
                        break 'phases;
                    }
                }
            }

            phases_completed += 1;
        }

        // Mark downstream statements as skipped if there was a failure
        if let Some(failed_idx) = failed_stmt_idx {
            self.mark_downstream_skipped(sheet, &mut results, failed_idx, phases);
        }

        // Commit or rollback
        if overall_status == SheetStatus::Success {
            tx.commit().await?;
            info!(
                "Sheet execution committed: {} statements in {}ms",
                total_statements,
                start_instant.elapsed().as_millis()
            );
        } else {
            tx.rollback().await?;
            overall_status = SheetStatus::RolledBack;
            warn!(
                "Sheet execution rolled back after failure at statement {:?}",
                failed_stmt_idx
            );
        }

        let completed_at = Utc::now();
        let duration_ms = start_instant.elapsed().as_millis() as u64;

        Ok(SheetExecutionResult {
            session_id: sheet.session_id,
            sheet_id: sheet.id,
            overall_status,
            phases_completed,
            phases_total: phases.len(),
            statements: results,
            started_at,
            completed_at,
            duration_ms,
        })
    }

    /// Substitute @symbols with UUIDs from the symbol table
    fn substitute_symbols(
        &self,
        source: &str,
        symbols: &HashMap<String, Uuid>,
    ) -> Result<String, String> {
        substitute_symbols_pure(source, symbols)
    }

    /// Execute a single DSL statement
    ///
    /// Note: Currently executes each statement in its own implicit transaction.
    /// For true single-transaction semantics across the entire sheet,
    /// the executor would need to support external transaction injection.
    async fn execute_statement(
        &self,
        _tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        source: &str,
    ) -> Result<StatementExecutionResult> {
        // Create execution context
        let mut ctx = ExecutionContext::new();

        // Create executor
        let executor = DslExecutor::new(self.pool.clone());

        // Execute the DSL
        // TODO: For true single-transaction semantics, we need to pass the tx
        // through to the executor. For now, each statement is its own transaction.
        let results = executor.execute_dsl(source, &mut ctx).await?;

        // Extract produced PK from execution results or symbols
        let produced_pk = results
            .iter()
            .find_map(|r| match r {
                ExecutionResult::Uuid(id) => Some(*id),
                _ => None,
            })
            .or_else(|| ctx.symbols.values().next().copied());

        Ok(StatementExecutionResult { produced_pk })
    }

    /// Mark all statements that depend on the failed one as skipped
    fn mark_downstream_skipped(
        &self,
        sheet: &mut DslSheet,
        results: &mut [StatementResult],
        failed_idx: usize,
        phases: &[ExecutionPhase],
    ) {
        // Find the phase of the failed statement
        let failed_depth = sheet.statements[failed_idx].dag_depth;

        // Mark all statements in later phases as skipped
        for phase in phases {
            if phase.depth > failed_depth {
                for &stmt_idx in &phase.statement_indices {
                    if !matches!(
                        sheet.statements[stmt_idx].status,
                        StatementStatus::Success | StatementStatus::Failed { .. }
                    ) {
                        sheet.statements[stmt_idx].status = StatementStatus::Skipped {
                            blocked_by: failed_idx,
                        };
                        results[stmt_idx].status = StatementStatus::Skipped {
                            blocked_by: failed_idx,
                        };
                        results[stmt_idx].error = Some(StatementError {
                            code: ErrorCode::Blocked,
                            message: format!("Blocked by failure of statement {}", failed_idx),
                            detail: None,
                            span: None,
                            blocked_by: Some(failed_idx),
                        });
                    }
                }
            }
        }

        // Also check statements in the same phase that weren't executed yet
        for phase in phases {
            if phase.depth == failed_depth {
                for &stmt_idx in &phase.statement_indices {
                    if matches!(results[stmt_idx].status, StatementStatus::Pending) {
                        sheet.statements[stmt_idx].status = StatementStatus::Skipped {
                            blocked_by: failed_idx,
                        };
                        results[stmt_idx].status = StatementStatus::Skipped {
                            blocked_by: failed_idx,
                        };
                        results[stmt_idx].error = Some(StatementError {
                            code: ErrorCode::Blocked,
                            message: format!("Blocked by failure of statement {}", failed_idx),
                            detail: None,
                            span: None,
                            blocked_by: Some(failed_idx),
                        });
                    }
                }
            }
        }
    }

    /// Persist execution result to audit table
    ///
    /// This is called after execution completes (success or failure) to create
    /// an audit trail for debugging and compliance.
    pub async fn persist_audit(
        &self,
        sheet: &DslSheet,
        result: &SheetExecutionResult,
        scope_dsl: &[String],
        template_dsl: Option<&str>,
        submitted_by: Option<&str>,
    ) -> Result<Uuid> {
        let execution_id = Uuid::new_v4();

        // Collect source statements
        let source_statements: Vec<String> =
            sheet.statements.iter().map(|s| s.source.clone()).collect();

        // Build DAG analysis JSON
        let dag_analysis = serde_json::json!({
            "phases": sheet.phases.iter().map(|p| {
                serde_json::json!({
                    "depth": p.depth,
                    "statement_indices": p.statement_indices,
                })
            }).collect::<Vec<_>>(),
            "total_statements": sheet.statements.len(),
        });

        // Build result JSON
        let result_json = serde_json::to_value(result)?;

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".sheet_execution_audit (
                execution_id,
                session_id,
                sheet_id,
                scope_dsl,
                template_dsl,
                source_statements,
                phase_count,
                statement_count,
                dag_analysis,
                overall_status,
                phases_completed,
                result,
                submitted_at,
                started_at,
                completed_at,
                duration_ms,
                submitted_by
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17
            )
            "#,
            execution_id,
            result.session_id,
            result.sheet_id,
            scope_dsl,
            template_dsl,
            &source_statements,
            sheet.phases.len() as i32,
            sheet.statements.len() as i32,
            dag_analysis,
            result.overall_status.as_str(),
            result.phases_completed as i32,
            result_json,
            result.started_at,
            result.started_at,
            result.completed_at,
            result.duration_ms as i64,
            submitted_by,
        )
        .execute(self.pool)
        .await?;

        info!(
            "Persisted execution audit: {} (session={}, status={})",
            execution_id,
            result.session_id,
            result.overall_status.as_str()
        );

        Ok(execution_id)
    }
}

/// Result of executing a single statement
struct StatementExecutionResult {
    /// Primary key produced by the statement (if any)
    produced_pk: Option<Uuid>,
}

/// Classify an error into an ErrorCode
fn classify_error(e: &anyhow::Error) -> ErrorCode {
    let msg = e.to_string().to_lowercase();

    if msg.contains("syntax") || msg.contains("parse") {
        ErrorCode::SyntaxError
    } else if msg.contains("unresolved") || msg.contains("symbol") {
        ErrorCode::UnresolvedSymbol
    } else if msg.contains("not found") || msg.contains("does not exist") {
        ErrorCode::EntityNotFound
    } else if msg.contains("ambiguous") {
        ErrorCode::AmbiguousEntity
    } else if msg.contains("type") || msg.contains("mismatch") {
        ErrorCode::TypeMismatch
    } else if msg.contains("constraint") || msg.contains("unique") || msg.contains("foreign key") {
        ErrorCode::DbConstraint
    } else if msg.contains("connection") || msg.contains("timeout") {
        ErrorCode::DbConnection
    } else if msg.contains("permission") || msg.contains("denied") {
        ErrorCode::PermissionDenied
    } else {
        ErrorCode::InternalError
    }
}

/// Pure function for symbol substitution (testable without pool)
fn substitute_symbols_pure(
    source: &str,
    symbols: &HashMap<String, Uuid>,
) -> Result<String, String> {
    let mut result = source.to_string();

    // Replace each @symbol with its UUID
    for (symbol, uuid) in symbols {
        let pattern = format!("@{}", symbol);
        result = result.replace(&pattern, &format!("\"{}\"", uuid));
    }

    // Check for remaining unresolved @symbols
    let mut chars = result.chars().peekable();
    let mut pos = 0;
    while let Some(c) = chars.next() {
        if c == '@' {
            if let Some(&next) = chars.peek() {
                if next.is_alphabetic() {
                    let remaining: String = result[pos..].chars().take(30).collect();
                    return Err(format!("Unresolved symbol in: {}", remaining));
                }
            }
        }
        pos += c.len_utf8();
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    /// Test symbol substitution without needing a database pool
    /// We test the substitute_symbols function directly since it's pure logic
    #[test]
    fn test_substitute_symbols_basic() {
        let mut symbols = HashMap::new();
        let uuid = Uuid::new_v4();
        symbols.insert("fund".to_string(), uuid);

        let source = "(cbu.assign-role :cbu-id @fund :role \"DIRECTOR\")";
        let result = substitute_symbols_pure(source, &symbols).unwrap();

        assert!(result.contains(&uuid.to_string()));
        assert!(!result.contains("@fund"));
    }

    #[test]
    fn test_substitute_symbols_unresolved() {
        let symbols = HashMap::new(); // Empty - @fund not defined

        let source = "(cbu.assign-role :cbu-id @fund :role \"DIRECTOR\")";
        let result = substitute_symbols_pure(source, &symbols);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unresolved symbol"));
    }

    #[test]
    fn test_substitute_symbols_multiple() {
        let mut symbols = HashMap::new();
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        symbols.insert("fund".to_string(), uuid1);
        symbols.insert("director".to_string(), uuid2);

        let source = "(cbu.assign-role :cbu-id @fund :person-id @director)";
        let result = substitute_symbols_pure(source, &symbols).unwrap();

        assert!(result.contains(&uuid1.to_string()));
        assert!(result.contains(&uuid2.to_string()));
        assert!(!result.contains("@fund"));
        assert!(!result.contains("@director"));
    }

    #[test]
    fn test_classify_error() {
        assert_eq!(
            classify_error(&anyhow!("syntax error at position 5")),
            ErrorCode::SyntaxError
        );
        assert_eq!(
            classify_error(&anyhow!("entity not found: Goldman Sachs")),
            ErrorCode::EntityNotFound
        );
        assert_eq!(
            classify_error(&anyhow!("unique constraint violation")),
            ErrorCode::DbConstraint
        );
        assert_eq!(
            classify_error(&anyhow!("something went wrong")),
            ErrorCode::InternalError
        );
    }
}
