//! Repository for DSL instances and their versions
//!
//! This module provides database access and operations for DSL instances,
//! their versions, and associated AST nodes. It enables full persistence
//! of the DSL-as-State pattern.

use crate::database::cbu_repository::CbuRepository;
use crate::error::Error;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{Pool, Postgres, Transaction};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Status of a DSL instance
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "UPPERCASE")]
pub enum InstanceStatus {
    Created,
    Editing,
    Compiled,
    Finalized,
    Archived,
    Failed,
}

impl ToString for InstanceStatus {
    fn to_string(&self) -> String {
        match self {
            InstanceStatus::Created => "CREATED".to_string(),
            InstanceStatus::Editing => "EDITING".to_string(),
            InstanceStatus::Compiled => "COMPILED".to_string(),
            InstanceStatus::Finalized => "FINALIZED".to_string(),
            InstanceStatus::Archived => "ARCHIVED".to_string(),
            InstanceStatus::Failed => "FAILED".to_string(),
        }
    }
}

/// Type of operation that created a DSL instance version
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "UPPERCASE")]
pub enum OperationType {
    CreateFromTemplate,
    IncrementalEdit,
    TemplateAddition,
    ManualEdit,
    Recompilation,
}

impl ToString for OperationType {
    fn to_string(&self) -> String {
        match self {
            OperationType::CreateFromTemplate => "CREATE_FROM_TEMPLATE".to_string(),
            OperationType::IncrementalEdit => "INCREMENTAL_EDIT".to_string(),
            OperationType::TemplateAddition => "TEMPLATE_ADDITION".to_string(),
            OperationType::ManualEdit => "MANUAL_EDIT".to_string(),
            OperationType::Recompilation => "RECOMPILATION".to_string(),
        }
    }
}

/// Status of a DSL compilation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "UPPERCASE")]
pub enum CompilationStatus {
    Pending,
    Success,
    Error,
}

impl ToString for CompilationStatus {
    fn to_string(&self) -> String {
        match self {
            CompilationStatus::Pending => "PENDING".to_string(),
            CompilationStatus::Success => "SUCCESS".to_string(),
            CompilationStatus::Error => "ERROR".to_string(),
        }
    }
}

/// Type of AST node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "UPPERCASE")]
pub enum AstNodeType {
    Verb,
    Attribute,
    List,
    Map,
    Value,
    Root,
    Comment,
    Placeholder,
    Reference,
    Special,
}

impl ToString for AstNodeType {
    fn to_string(&self) -> String {
        match self {
            AstNodeType::Verb => "VERB".to_string(),
            AstNodeType::Attribute => "ATTRIBUTE".to_string(),
            AstNodeType::List => "LIST".to_string(),
            AstNodeType::Map => "MAP".to_string(),
            AstNodeType::Value => "VALUE".to_string(),
            AstNodeType::Root => "ROOT".to_string(),
            AstNodeType::Comment => "COMMENT".to_string(),
            AstNodeType::Placeholder => "PLACEHOLDER".to_string(),
            AstNodeType::Reference => "REFERENCE".to_string(),
            AstNodeType::Special => "SPECIAL".to_string(),
        }
    }
}

/// DSL Instance representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslInstance {
    pub instance_id: Uuid,
    pub domain_name: String,
    pub business_reference: String,
    pub current_version: i32,
    pub status: InstanceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: Option<JsonValue>,
}

/// DSL Instance Version representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslInstanceVersion {
    pub version_id: Uuid,
    pub instance_id: Uuid,
    pub version_number: i32,
    pub dsl_content: String,
    pub operation_type: OperationType,
    pub compilation_status: CompilationStatus,
    pub ast_json: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
    pub change_description: Option<String>,
}

/// AST Node representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstNode {
    pub node_id: Uuid,
    pub version_id: Uuid,
    pub parent_node_id: Option<Uuid>,
    pub node_type: AstNodeType,
    pub node_key: Option<String>,
    pub node_value: Option<JsonValue>,
    pub position_index: Option<i32>,
    pub depth: i32,
    pub path: String,
    pub created_at: DateTime<Utc>,
}

/// DSL Template representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslTemplate {
    pub template_id: Uuid,
    pub template_name: String,
    pub domain_name: String,
    pub template_type: String,
    pub content: String,
    pub variables: Option<JsonValue>,
    pub requirements: Option<JsonValue>,
    pub metadata: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// DSL Business Reference representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslBusinessReference {
    pub reference_id: Uuid,
    pub instance_id: Uuid,
    pub reference_type: String,
    pub reference_id_value: String,
    pub created_at: DateTime<Utc>,
}

/// DSL Compilation Log representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslCompilationLog {
    pub log_id: Uuid,
    pub version_id: Uuid,
    pub compilation_start: DateTime<Utc>,
    pub compilation_end: Option<DateTime<Utc>>,
    pub success: Option<bool>,
    pub error_message: Option<String>,
    pub error_location: Option<JsonValue>,
    pub node_count: Option<i32>,
    pub complexity_score: Option<f64>,
    pub performance_metrics: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
}

/// Repository trait defining operations for DSL instances
#[async_trait]
pub trait DslInstanceRepository: Send + Sync {
    // Instance operations
    async fn create_instance(
        &self,
        domain_name: &str,
        business_reference: &str,
        metadata: Option<JsonValue>,
    ) -> Result<DslInstance, Error>;

    async fn get_instance(&self, instance_id: Uuid) -> Result<Option<DslInstance>, Error>;

    async fn get_instance_by_reference(
        &self,
        domain_name: &str,
        business_reference: &str,
    ) -> Result<Option<DslInstance>, Error>;

    async fn update_instance_status(
        &self,
        instance_id: Uuid,
        status: InstanceStatus,
    ) -> Result<DslInstance, Error>;

    async fn update_instance_metadata(
        &self,
        instance_id: Uuid,
        metadata: JsonValue,
    ) -> Result<DslInstance, Error>;

    async fn list_instances(
        &self,
        domain_name: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<DslInstance>, Error>;

    async fn delete_instance(&self, instance_id: Uuid) -> Result<(), Error>;

    // Version operations
    async fn create_version(
        &self,
        instance_id: Uuid,
        dsl_content: &str,
        operation_type: OperationType,
        created_by: Option<&str>,
        change_description: Option<&str>,
    ) -> Result<DslInstanceVersion, Error>;

    async fn get_version(&self, version_id: Uuid) -> Result<Option<DslInstanceVersion>, Error>;

    async fn get_version_by_number(
        &self,
        instance_id: Uuid,
        version_number: i32,
    ) -> Result<Option<DslInstanceVersion>, Error>;

    async fn get_latest_version(
        &self,
        instance_id: Uuid,
    ) -> Result<Option<DslInstanceVersion>, Error>;

    async fn list_versions(
        &self,
        instance_id: Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<DslInstanceVersion>, Error>;

    async fn update_version_ast(
        &self,
        version_id: Uuid,
        ast_json: JsonValue,
        compilation_status: CompilationStatus,
    ) -> Result<DslInstanceVersion, Error>;

    // AST operations
    async fn store_ast_nodes(&self, version_id: Uuid, nodes: &[AstNode]) -> Result<(), Error>;

    async fn get_ast_node(&self, node_id: Uuid) -> Result<Option<AstNode>, Error>;

    async fn get_ast_nodes_by_version(&self, version_id: Uuid) -> Result<Vec<AstNode>, Error>;

    async fn get_ast_nodes_by_path(
        &self,
        version_id: Uuid,
        path_pattern: &str,
    ) -> Result<Vec<AstNode>, Error>;

    async fn get_ast_nodes_by_type(
        &self,
        version_id: Uuid,
        node_type: AstNodeType,
    ) -> Result<Vec<AstNode>, Error>;

    // Template operations
    async fn create_template(
        &self,
        template_name: &str,
        domain_name: &str,
        template_type: &str,
        content: &str,
        variables: Option<JsonValue>,
        requirements: Option<JsonValue>,
        metadata: Option<JsonValue>,
    ) -> Result<DslTemplate, Error>;

    async fn get_template(&self, template_id: Uuid) -> Result<Option<DslTemplate>, Error>;

    async fn get_template_by_name(&self, template_name: &str)
        -> Result<Option<DslTemplate>, Error>;

    async fn list_templates(
        &self,
        domain_name: Option<&str>,
        template_type: Option<&str>,
    ) -> Result<Vec<DslTemplate>, Error>;

    async fn update_template(
        &self,
        template_id: Uuid,
        content: &str,
        variables: Option<JsonValue>,
        requirements: Option<JsonValue>,
        metadata: Option<JsonValue>,
    ) -> Result<DslTemplate, Error>;

    // Business reference operations
    async fn create_business_reference(
        &self,
        instance_id: Uuid,
        reference_type: &str,
        reference_id_value: &str,
    ) -> Result<DslBusinessReference, Error>;

    async fn get_instances_by_reference(
        &self,
        reference_type: &str,
        reference_id_value: &str,
    ) -> Result<Vec<DslInstance>, Error>;

    async fn get_references_by_instance(
        &self,
        instance_id: Uuid,
    ) -> Result<Vec<DslBusinessReference>, Error>;

    // Compilation log operations
    async fn create_compilation_log(
        &self,
        version_id: Uuid,
        compilation_start: DateTime<Utc>,
    ) -> Result<DslCompilationLog, Error>;

    async fn complete_compilation_log(
        &self,
        log_id: Uuid,
        compilation_end: DateTime<Utc>,
        success: bool,
        error_message: Option<&str>,
        error_location: Option<JsonValue>,
        node_count: Option<i32>,
        complexity_score: Option<f64>,
        performance_metrics: Option<JsonValue>,
    ) -> Result<DslCompilationLog, Error>;

    async fn get_compilation_logs_by_version(
        &self,
        version_id: Uuid,
    ) -> Result<Vec<DslCompilationLog>, Error>;
}

/// PostgreSQL implementation of the DSL Instance Repository
pub struct PgDslInstanceRepository {
    pool: Pool<Postgres>,
    template_cache: Arc<RwLock<HashMap<String, DslTemplate>>>,
}

impl PgDslInstanceRepository {
    /// Create a new instance repository with the given connection pool
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self {
            pool,
            template_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Begin a transaction
    pub async fn begin(&self) -> Result<Transaction<'_, Postgres>, Error> {
        self.pool.begin().await.map_err(Error::Database)
    }
}

#[async_trait]
impl DslInstanceRepository for PgDslInstanceRepository {
    async fn create_instance(
        &self,
        domain_name: &str,
        business_reference: &str,
        metadata: Option<JsonValue>,
    ) -> Result<DslInstance, Error> {
        let instance = sqlx::query_as!(
            DslInstance,
            r#"
            INSERT INTO "ob-poc".dsl_instances
                (domain_name, business_reference, status, metadata)
            VALUES
                ($1, $2, $3, $4)
            RETURNING
                instance_id,
                domain_name,
                business_reference,
                current_version,
                status as "status: InstanceStatus",
                created_at,
                updated_at,
                metadata
            "#,
            domain_name,
            business_reference,
            InstanceStatus::Created.to_string(),
            metadata
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(instance)
    }

    async fn get_instance(&self, instance_id: Uuid) -> Result<Option<DslInstance>, Error> {
        let instance = sqlx::query_as!(
            DslInstance,
            r#"
            SELECT
                instance_id,
                domain_name,
                business_reference,
                current_version,
                status as "status: InstanceStatus",
                created_at,
                updated_at,
                metadata
            FROM "ob-poc".dsl_instances
            WHERE instance_id = $1
            "#,
            instance_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(instance)
    }

    async fn get_instance_by_reference(
        &self,
        domain_name: &str,
        business_reference: &str,
    ) -> Result<Option<DslInstance>, Error> {
        let instance = sqlx::query_as!(
            DslInstance,
            r#"
            SELECT
                instance_id,
                domain_name,
                business_reference,
                current_version,
                status as "status: InstanceStatus",
                created_at,
                updated_at,
                metadata
            FROM "ob-poc".dsl_instances
            WHERE domain_name = $1 AND business_reference = $2
            "#,
            domain_name,
            business_reference
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(instance)
    }

    async fn update_instance_status(
        &self,
        instance_id: Uuid,
        status: InstanceStatus,
    ) -> Result<DslInstance, Error> {
        let instance = sqlx::query_as!(
            DslInstance,
            r#"
            UPDATE "ob-poc".dsl_instances
            SET
                status = $2,
                updated_at = now() at time zone 'utc'
            WHERE instance_id = $1
            RETURNING
                instance_id,
                domain_name,
                business_reference,
                current_version,
                status as "status: InstanceStatus",
                created_at,
                updated_at,
                metadata
            "#,
            instance_id,
            status.to_string()
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(instance)
    }

    async fn update_instance_metadata(
        &self,
        instance_id: Uuid,
        metadata: JsonValue,
    ) -> Result<DslInstance, Error> {
        let instance = sqlx::query_as!(
            DslInstance,
            r#"
            UPDATE "ob-poc".dsl_instances
            SET
                metadata = $2,
                updated_at = now() at time zone 'utc'
            WHERE instance_id = $1
            RETURNING
                instance_id,
                domain_name,
                business_reference,
                current_version,
                status as "status: InstanceStatus",
                created_at,
                updated_at,
                metadata
            "#,
            instance_id,
            metadata
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(instance)
    }

    async fn list_instances(
        &self,
        domain_name: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<DslInstance>, Error> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);

        let instances = if let Some(domain) = domain_name {
            sqlx::query_as!(
                DslInstance,
                r#"
                SELECT
                    instance_id,
                    domain_name,
                    business_reference,
                    current_version,
                    status as "status: InstanceStatus",
                    created_at,
                    updated_at,
                    metadata
                FROM "ob-poc".dsl_instances
                WHERE domain_name = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
                domain,
                limit,
                offset
            )
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?
        } else {
            sqlx::query_as!(
                DslInstance,
                r#"
                SELECT
                    instance_id,
                    domain_name,
                    business_reference,
                    current_version,
                    status as "status: InstanceStatus",
                    created_at,
                    updated_at,
                    metadata
                FROM "ob-poc".dsl_instances
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#,
                limit,
                offset
            )
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?
        };

        Ok(instances)
    }

    async fn delete_instance(&self, instance_id: Uuid) -> Result<(), Error> {
        sqlx::query!(
            r#"
            DELETE FROM "ob-poc".dsl_instances
            WHERE instance_id = $1
            "#,
            instance_id
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(())
    }

    async fn create_version(
        &self,
        instance_id: Uuid,
        dsl_content: &str,
        operation_type: OperationType,
        created_by: Option<&str>,
        change_description: Option<&str>,
    ) -> Result<DslInstanceVersion, Error> {
        let mut tx = self.begin().await?;

        // Get the current version number
        let current_version: i32 = sqlx::query_scalar!(
            r#"
            SELECT current_version
            FROM "ob-poc".dsl_instances
            WHERE instance_id = $1
            "#,
            instance_id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;

        // Calculate the next version number
        let next_version = current_version + 1;

        // Create the new version
        let version = sqlx::query_as!(
            DslInstanceVersion,
            r#"
            INSERT INTO "ob-poc".dsl_instance_versions
                (instance_id, version_number, dsl_content, operation_type, created_by, change_description)
            VALUES
                ($1, $2, $3, $4, $5, $6)
            RETURNING
                version_id,
                instance_id,
                version_number,
                dsl_content,
                operation_type as "operation_type: OperationType",
                compilation_status as "compilation_status: CompilationStatus",
                ast_json,
                created_at,
                created_by,
                change_description
            "#,
            instance_id,
            next_version,
            dsl_content,
            operation_type.to_string(),
            created_by,
            change_description
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;

        // Update the instance's current version
        sqlx::query!(
            r#"
            UPDATE "ob-poc".dsl_instances
            SET
                current_version = $2,
                updated_at = now() at time zone 'utc'
            WHERE instance_id = $1
            "#,
            instance_id,
            next_version
        )
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        tx.commit().await.map_err(Error::Database)?;

        Ok(version)
    }

    async fn get_version(&self, version_id: Uuid) -> Result<Option<DslInstanceVersion>, Error> {
        let version = sqlx::query_as!(
            DslInstanceVersion,
            r#"
            SELECT
                version_id,
                instance_id,
                version_number,
                dsl_content,
                operation_type as "operation_type: OperationType",
                compilation_status as "compilation_status: CompilationStatus",
                ast_json,
                created_at,
                created_by,
                change_description
            FROM "ob-poc".dsl_instance_versions
            WHERE version_id = $1
            "#,
            version_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(version)
    }

    async fn get_version_by_number(
        &self,
        instance_id: Uuid,
        version_number: i32,
    ) -> Result<Option<DslInstanceVersion>, Error> {
        let version = sqlx::query_as!(
            DslInstanceVersion,
            r#"
            SELECT
                version_id,
                instance_id,
                version_number,
                dsl_content,
                operation_type as "operation_type: OperationType",
                compilation_status as "compilation_status: CompilationStatus",
                ast_json,
                created_at,
                created_by,
                change_description
            FROM "ob-poc".dsl_instance_versions
            WHERE instance_id = $1 AND version_number = $2
            "#,
            instance_id,
            version_number
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(version)
    }

    async fn get_latest_version(
        &self,
        instance_id: Uuid,
    ) -> Result<Option<DslInstanceVersion>, Error> {
        let version = sqlx::query_as!(
            DslInstanceVersion,
            r#"
            SELECT
                version_id,
                instance_id,
                version_number,
                dsl_content,
                operation_type as "operation_type: OperationType",
                compilation_status as "compilation_status: CompilationStatus",
                ast_json,
                created_at,
                created_by,
                change_description
            FROM "ob-poc".dsl_instance_versions
            WHERE instance_id = $1
            ORDER BY version_number DESC
            LIMIT 1
            "#,
            instance_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(version)
    }

    async fn list_versions(
        &self,
        instance_id: Uuid,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<DslInstanceVersion>, Error> {
        let limit = limit.unwrap_or(100);
        let offset = offset.unwrap_or(0);

        let versions = sqlx::query_as!(
            DslInstanceVersion,
            r#"
            SELECT
                version_id,
                instance_id,
                version_number,
                dsl_content,
                operation_type as "operation_type: OperationType",
                compilation_status as "compilation_status: CompilationStatus",
                ast_json,
                created_at,
                created_by,
                change_description
            FROM "ob-poc".dsl_instance_versions
            WHERE instance_id = $1
            ORDER BY version_number DESC
            LIMIT $2 OFFSET $3
            "#,
            instance_id,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(versions)
    }

    async fn update_version_ast(
        &self,
        version_id: Uuid,
        ast_json: JsonValue,
        compilation_status: CompilationStatus,
    ) -> Result<DslInstanceVersion, Error> {
        let version = sqlx::query_as!(
            DslInstanceVersion,
            r#"
            UPDATE "ob-poc".dsl_instance_versions
            SET
                ast_json = $2,
                compilation_status = $3
            WHERE version_id = $1
            RETURNING
                version_id,
                instance_id,
                version_number,
                dsl_content,
                operation_type as "operation_type: OperationType",
                compilation_status as "compilation_status: CompilationStatus",
                ast_json,
                created_at,
                created_by,
                change_description
            "#,
            version_id,
            ast_json,
            compilation_status.to_string()
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(version)
    }

    async fn store_ast_nodes(&self, version_id: Uuid, nodes: &[AstNode]) -> Result<(), Error> {
        let mut tx = self.begin().await?;

        // Delete existing nodes for this version if any
        sqlx::query!(
            r#"
            DELETE FROM "ob-poc".ast_nodes
            WHERE version_id = $1
            "#,
            version_id
        )
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        // Insert all nodes
        for node in nodes {
            sqlx::query!(
                r#"
                INSERT INTO "ob-poc".ast_nodes
                    (
                        version_id, parent_node_id, node_type, node_key, node_value,
                        position_index, depth, path
                    )
                VALUES
                    ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                version_id,
                node.parent_node_id,
                node.node_type.to_string(),
                node.node_key,
                node.node_value,
                node.position_index,
                node.depth,
                node.path
            )
            .execute(&mut *tx)
            .await
            .map_err(Error::Database)?;
        }

        tx.commit().await.map_err(Error::Database)?;

        Ok(())
    }

    async fn get_ast_node(&self, node_id: Uuid) -> Result<Option<AstNode>, Error> {
        let node = sqlx::query_as!(
            AstNode,
            r#"
            SELECT
                node_id,
                version_id,
                parent_node_id,
                node_type as "node_type: AstNodeType",
                node_key,
                node_value,
                position_index,
                depth,
                path,
                created_at
            FROM "ob-poc".ast_nodes
            WHERE node_id = $1
            "#,
            node_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(node)
    }

    async fn get_ast_nodes_by_version(&self, version_id: Uuid) -> Result<Vec<AstNode>, Error> {
        let nodes = sqlx::query_as!(
            AstNode,
            r#"
            SELECT
                node_id,
                version_id,
                parent_node_id,
                node_type as "node_type: AstNodeType",
                node_key,
                node_value,
                position_index,
                depth,
                path,
                created_at
            FROM "ob-poc".ast_nodes
            WHERE version_id = $1
            ORDER BY depth, COALESCE(position_index, 0)
            "#,
            version_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(nodes)
    }

    async fn get_ast_nodes_by_path(
        &self,
        version_id: Uuid,
        path_pattern: &str,
    ) -> Result<Vec<AstNode>, Error> {
        let nodes = sqlx::query_as!(
            AstNode,
            r#"
            SELECT
                node_id,
                version_id,
                parent_node_id,
                node_type as "node_type: AstNodeType",
                node_key,
                node_value,
                position_index,
                depth,
                path,
                created_at
            FROM "ob-poc".ast_nodes
            WHERE version_id = $1 AND path LIKE $2
            ORDER BY depth, COALESCE(position_index, 0)
            "#,
            version_id,
            format!("%{}%", path_pattern)
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(nodes)
    }

    async fn get_ast_nodes_by_type(
        &self,
        version_id: Uuid,
        node_type: AstNodeType,
    ) -> Result<Vec<AstNode>, Error> {
        let nodes = sqlx::query_as!(
            AstNode,
            r#"
            SELECT
                node_id,
                version_id,
                parent_node_id,
                node_type as "node_type: AstNodeType",
                node_key,
                node_value,
                position_index,
                depth,
                path,
                created_at
            FROM "ob-poc".ast_nodes
            WHERE version_id = $1 AND node_type = $2
            ORDER BY depth, COALESCE(position_index, 0)
            "#,
            version_id,
            node_type.to_string()
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(nodes)
    }

    // Template operations
    async fn create_template(
        &self,
        template_name: &str,
        domain_name: &str,
        template_type: &str,
        content: &str,
        variables: Option<JsonValue>,
        requirements: Option<JsonValue>,
        metadata: Option<JsonValue>,
    ) -> Result<DslTemplate, Error> {
        let template = sqlx::query_as!(
            DslTemplate,
            r#"
            INSERT INTO "ob-poc".dsl_templates
                (template_name, domain_name, template_type, content, variables, requirements, metadata)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                template_id,
                template_name,
                domain_name,
                template_type,
                content,
                variables,
                requirements,
                metadata,
                created_at,
                updated_at
            "#,
            template_name,
            domain_name,
            template_type,
            content,
            variables,
            requirements,
            metadata
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        // Update the template cache
        let mut cache = self.template_cache.write().await;
        cache.insert(template_name.to_string(), template.clone());

        Ok(template)
    }

    async fn get_template(&self, template_id: Uuid) -> Result<Option<DslTemplate>, Error> {
        let template = sqlx::query_as!(
            DslTemplate,
            r#"
            SELECT
                template_id,
                template_name,
                domain_name,
                template_type,
                content,
                variables,
                requirements,
                metadata,
                created_at,
                updated_at
            FROM "ob-poc".dsl_templates
            WHERE template_id = $1
            "#,
            template_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(template)
    }

    async fn get_template_by_name(
        &self,
        template_name: &str,
    ) -> Result<Option<DslTemplate>, Error> {
        // Check the cache first
        {
            let cache = self.template_cache.read().await;
            if let Some(template) = cache.get(template_name) {
                return Ok(Some(template.clone()));
            }
        }

        // If not in cache, fetch from database
        let template = sqlx::query_as!(
            DslTemplate,
            r#"
            SELECT
                template_id,
                template_name,
                domain_name,
                template_type,
                content,
                variables,
                requirements,
                metadata,
                created_at,
                updated_at
            FROM "ob-poc".dsl_templates
            WHERE template_name = $1
            "#,
            template_name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        // Update cache if found
        if let Some(ref t) = template {
            let mut cache = self.template_cache.write().await;
            cache.insert(template_name.to_string(), t.clone());
        }

        Ok(template)
    }

    async fn list_templates(
        &self,
        domain_name: Option<&str>,
        template_type: Option<&str>,
    ) -> Result<Vec<DslTemplate>, Error> {
        let templates = match (domain_name, template_type) {
            (Some(domain), Some(template_type)) => sqlx::query_as!(
                DslTemplate,
                r#"
                    SELECT
                        template_id,
                        template_name,
                        domain_name,
                        template_type,
                        content,
                        variables,
                        requirements,
                        metadata,
                        created_at,
                        updated_at
                    FROM "ob-poc".dsl_templates
                    WHERE domain_name = $1 AND template_type = $2
                    ORDER BY template_name
                    "#,
                domain,
                template_type
            )
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?,
            (Some(domain), None) => sqlx::query_as!(
                DslTemplate,
                r#"
                    SELECT
                        template_id,
                        template_name,
                        domain_name,
                        template_type,
                        content,
                        variables,
                        requirements,
                        metadata,
                        created_at,
                        updated_at
                    FROM "ob-poc".dsl_templates
                    WHERE domain_name = $1
                    ORDER BY template_name
                    "#,
                domain
            )
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?,
            (None, Some(template_type)) => sqlx::query_as!(
                DslTemplate,
                r#"
                    SELECT
                        template_id,
                        template_name,
                        domain_name,
                        template_type,
                        content,
                        variables,
                        requirements,
                        metadata,
                        created_at,
                        updated_at
                    FROM "ob-poc".dsl_templates
                    WHERE template_type = $1
                    ORDER BY template_name
                    "#,
                template_type
            )
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?,
            (None, None) => sqlx::query_as!(
                DslTemplate,
                r#"
                    SELECT
                        template_id,
                        template_name,
                        domain_name,
                        template_type,
                        content,
                        variables,
                        requirements,
                        metadata,
                        created_at,
                        updated_at
                    FROM "ob-poc".dsl_templates
                    ORDER BY template_name
                    "#
            )
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?,
        };

        Ok(templates)
    }

    async fn update_template(
        &self,
        template_id: Uuid,
        content: &str,
        variables: Option<JsonValue>,
        requirements: Option<JsonValue>,
        metadata: Option<JsonValue>,
    ) -> Result<DslTemplate, Error> {
        let template = sqlx::query_as!(
            DslTemplate,
            r#"
            UPDATE "ob-poc".dsl_templates
            SET
                content = $2,
                variables = $3,
                requirements = $4,
                metadata = $5,
                updated_at = now() at time zone 'utc'
            WHERE template_id = $1
            RETURNING
                template_id,
                template_name,
                domain_name,
                template_type,
                content,
                variables,
                requirements,
                metadata,
                created_at,
                updated_at
            "#,
            template_id,
            content,
            variables,
            requirements,
            metadata
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        // Update the template cache
        let mut cache = self.template_cache.write().await;
        cache.insert(template.template_name.clone(), template.clone());

        Ok(template)
    }

    // Business reference operations
    async fn create_business_reference(
        &self,
        instance_id: Uuid,
        reference_type: &str,
        reference_id_value: &str,
    ) -> Result<DslBusinessReference, Error> {
        let reference = sqlx::query_as!(
            DslBusinessReference,
            r#"
            INSERT INTO "ob-poc".dsl_business_references
                (instance_id, reference_type, reference_id_value)
            VALUES
                ($1, $2, $3)
            RETURNING
                reference_id,
                instance_id,
                reference_type,
                reference_id_value,
                created_at
            "#,
            instance_id,
            reference_type,
            reference_id_value
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(reference)
    }

    async fn get_instances_by_reference(
        &self,
        reference_type: &str,
        reference_id_value: &str,
    ) -> Result<Vec<DslInstance>, Error> {
        let instances = sqlx::query_as!(
            DslInstance,
            r#"
            SELECT
                i.instance_id,
                i.domain_name,
                i.business_reference,
                i.current_version,
                i.status as "status: InstanceStatus",
                i.created_at,
                i.updated_at,
                i.metadata
            FROM "ob-poc".dsl_instances i
            JOIN "ob-poc".dsl_business_references r ON i.instance_id = r.instance_id
            WHERE r.reference_type = $1 AND r.reference_id_value = $2
            "#,
            reference_type,
            reference_id_value
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(instances)
    }

    async fn get_references_by_instance(
        &self,
        instance_id: Uuid,
    ) -> Result<Vec<DslBusinessReference>, Error> {
        let references = sqlx::query_as!(
            DslBusinessReference,
            r#"
            SELECT
                reference_id,
                instance_id,
                reference_type,
                reference_id_value,
                created_at
            FROM "ob-poc".dsl_business_references
            WHERE instance_id = $1
            "#,
            instance_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(references)
    }

    // Compilation log operations
    async fn create_compilation_log(
        &self,
        version_id: Uuid,
        compilation_start: DateTime<Utc>,
    ) -> Result<DslCompilationLog, Error> {
        let log = sqlx::query_as!(
            DslCompilationLog,
            r#"
            INSERT INTO "ob-poc".dsl_compilation_logs
                (version_id, compilation_start)
            VALUES
                ($1, $2)
            RETURNING
                log_id,
                version_id,
                compilation_start,
                compilation_end,
                success,
                error_message,
                error_location,
                node_count,
                complexity_score,
                performance_metrics,
                created_at
            "#,
            version_id,
            compilation_start
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(log)
    }

    async fn complete_compilation_log(
        &self,
        log_id: Uuid,
        compilation_end: DateTime<Utc>,
        success: bool,
        error_message: Option<&str>,
        error_location: Option<JsonValue>,
        node_count: Option<i32>,
        complexity_score: Option<f64>,
        performance_metrics: Option<JsonValue>,
    ) -> Result<DslCompilationLog, Error> {
        let log = sqlx::query_as!(
            DslCompilationLog,
            r#"
            UPDATE "ob-poc".dsl_compilation_logs
            SET
                compilation_end = $2,
                success = $3,
                error_message = $4,
                error_location = $5,
                node_count = $6,
                complexity_score = $7,
                performance_metrics = $8
            WHERE log_id = $1
            RETURNING
                log_id,
                version_id,
                compilation_start,
                compilation_end,
                success,
                error_message,
                error_location,
                node_count,
                complexity_score,
                performance_metrics,
                created_at
            "#,
            log_id,
            compilation_end,
            success,
            error_message,
            error_location,
            node_count,
            complexity_score,
            performance_metrics
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(log)
    }

    async fn get_compilation_logs_by_version(
        &self,
        version_id: Uuid,
    ) -> Result<Vec<DslCompilationLog>, Error> {
        let logs = sqlx::query_as!(
            DslCompilationLog,
            r#"
            SELECT
                log_id,
                version_id,
                compilation_start,
                compilation_end,
                success,
                error_message,
                error_location,
                node_count,
                complexity_score,
                performance_metrics,
                created_at
            FROM "ob-poc".dsl_compilation_logs
            WHERE version_id = $1
            ORDER BY compilation_start DESC
            "#,
            version_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(logs)
    }
}
