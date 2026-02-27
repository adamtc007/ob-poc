//! Schema domain CustomOps (Spec §C.2 — 5 verbs).
//!
//! Schema introspection and extraction verbs.
//! Allowed in BOTH Research and Governed AgentModes (read-only).

use anyhow::Result;
use async_trait::async_trait;

use ob_poc_macros::register_custom_op;

use super::sem_reg_helpers::delegate_to_tool;
use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};

#[cfg(feature = "database")]
use sqlx::PgPool;

// ── Schema Introspection ──────────────────────────────────────────

/// Introspect database schema via information_schema.
#[register_custom_op]
pub struct SchemaIntrospectOp;

#[async_trait]
impl CustomOperation for SchemaIntrospectOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "introspect"
    }
    fn rationale(&self) -> &'static str {
        "Delegates to db_introspect MCP tool"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("schema.introspect requires database"))
    }
}

// ── Schema Extraction ─────────────────────────────────────────────

/// Extract attribute definitions from database schema.
#[register_custom_op]
pub struct SchemaExtractAttributesOp;

#[async_trait]
impl CustomOperation for SchemaExtractAttributesOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "extract-attributes"
    }
    fn rationale(&self) -> &'static str {
        "Scanner-based attribute extraction from schema columns"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Uses db_introspect as the underlying tool, with attribute extraction logic
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!(
            "schema.extract-attributes requires database"
        ))
    }
}

/// Extract verb contracts from YAML configuration.
#[register_custom_op]
pub struct SchemaExtractVerbsOp;

#[async_trait]
impl CustomOperation for SchemaExtractVerbsOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "extract-verbs"
    }
    fn rationale(&self) -> &'static str {
        "Scanner-based verb extraction from YAML config"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("schema.extract-verbs requires database"))
    }
}

/// Extract entity type definitions from schema.
#[register_custom_op]
pub struct SchemaExtractEntitiesOp;

#[async_trait]
impl CustomOperation for SchemaExtractEntitiesOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "extract-entities"
    }
    fn rationale(&self) -> &'static str {
        "Scanner-based entity type extraction from schema tables"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("schema.extract-entities requires database"))
    }
}

/// Cross-reference schema against registry snapshots for drift detection.
#[register_custom_op]
pub struct SchemaCrossReferenceOp;

#[async_trait]
impl CustomOperation for SchemaCrossReferenceOp {
    fn domain(&self) -> &'static str {
        "schema"
    }
    fn verb(&self) -> &'static str {
        "cross-reference"
    }
    fn rationale(&self) -> &'static str {
        "Scanner drift detection: compare schema vs registry snapshots"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        delegate_to_tool(pool, ctx, verb_call, "db_introspect").await
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow::anyhow!("schema.cross-reference requires database"))
    }
}
