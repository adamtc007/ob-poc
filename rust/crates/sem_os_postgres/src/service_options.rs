//! Service-options framework storage and pure planning logic.
//!
//! This module is intentionally not wired to SemOS verbs, DAG slots, or UI
//! routes. It provides the Phase 2 core used by later wiring phases.

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// Canonical source-kind values for service options and resource attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SourceKind {
    Derived,
    CbuProfile,
    InstrumentMatrix,
    LegalEntity,
    Document,
    ProductOption,
    Manual,
    OptionBinding,
}

impl SourceKind {
    /// Return the database representation for this source kind.
    ///
    /// # Examples
    ///
    /// ```
    /// use sem_os_postgres::service_options::SourceKind;
    ///
    /// assert_eq!(SourceKind::InstrumentMatrix.as_db_str(), "instrument_matrix");
    /// ```
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            SourceKind::Derived => "derived",
            SourceKind::CbuProfile => "cbu_profile",
            SourceKind::InstrumentMatrix => "instrument_matrix",
            SourceKind::LegalEntity => "legal_entity",
            SourceKind::Document => "document",
            SourceKind::ProductOption => "product_option",
            SourceKind::Manual => "manual",
            SourceKind::OptionBinding => "option_binding",
        }
    }
}

impl TryFrom<&str> for SourceKind {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "derived" => Ok(SourceKind::Derived),
            "cbu_profile" => Ok(SourceKind::CbuProfile),
            "instrument_matrix" => Ok(SourceKind::InstrumentMatrix),
            "legal_entity" => Ok(SourceKind::LegalEntity),
            "document" => Ok(SourceKind::Document),
            "product_option" => Ok(SourceKind::ProductOption),
            "manual" => Ok(SourceKind::Manual),
            "option_binding" => Ok(SourceKind::OptionBinding),
            other => bail!("unknown source kind: {other}"),
        }
    }
}

/// Axes along which an option can drive resource fan-out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FanoutAxis {
    None,
    Market,
    Currency,
    Counterparty,
    Account,
    Fund,
    ShareClass,
    LegalEntity,
    InstructionChannel,
    Jurisdiction,
    BookingPrincipal,
}

impl FanoutAxis {
    /// Return the database representation for this fan-out axis.
    ///
    /// # Examples
    ///
    /// ```
    /// use sem_os_postgres::service_options::FanoutAxis;
    ///
    /// assert_eq!(FanoutAxis::Market.as_db_str(), "market");
    /// ```
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            FanoutAxis::None => "none",
            FanoutAxis::Market => "market",
            FanoutAxis::Currency => "currency",
            FanoutAxis::Counterparty => "counterparty",
            FanoutAxis::Account => "account",
            FanoutAxis::Fund => "fund",
            FanoutAxis::ShareClass => "share_class",
            FanoutAxis::LegalEntity => "legal_entity",
            FanoutAxis::InstructionChannel => "instruction_channel",
            FanoutAxis::Jurisdiction => "jurisdiction",
            FanoutAxis::BookingPrincipal => "booking_principal",
        }
    }
}

impl TryFrom<&str> for FanoutAxis {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "none" => Ok(FanoutAxis::None),
            "market" => Ok(FanoutAxis::Market),
            "currency" => Ok(FanoutAxis::Currency),
            "counterparty" => Ok(FanoutAxis::Counterparty),
            "account" => Ok(FanoutAxis::Account),
            "fund" => Ok(FanoutAxis::Fund),
            "share_class" => Ok(FanoutAxis::ShareClass),
            "legal_entity" => Ok(FanoutAxis::LegalEntity),
            "instruction_channel" => Ok(FanoutAxis::InstructionChannel),
            "jurisdiction" => Ok(FanoutAxis::Jurisdiction),
            "booking_principal" => Ok(FanoutAxis::BookingPrincipal),
            other => bail!("unknown fanout axis: {other}"),
        }
    }
}

/// Resource fan-out materialisation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FanoutMode {
    PerValue,
    Shared,
    Grouped,
    Conditional,
}

impl FanoutMode {
    /// Return the database representation for this fan-out mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use sem_os_postgres::service_options::FanoutMode;
    ///
    /// assert_eq!(FanoutMode::PerValue.as_db_str(), "per_value");
    /// ```
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            FanoutMode::PerValue => "per_value",
            FanoutMode::Shared => "shared",
            FanoutMode::Grouped => "grouped",
            FanoutMode::Conditional => "conditional",
        }
    }
}

impl TryFrom<&str> for FanoutMode {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "per_value" => Ok(FanoutMode::PerValue),
            "shared" => Ok(FanoutMode::Shared),
            "grouped" => Ok(FanoutMode::Grouped),
            "conditional" => Ok(FanoutMode::Conditional),
            other => bail!("unknown fanout mode: {other}"),
        }
    }
}

/// Active service option definition row.
#[derive(Debug, Clone, FromRow)]
pub(crate) struct ServiceOptionDefRow {
    pub(crate) service_option_def_id: Uuid,
    pub(crate) service_id: Uuid,
    pub(crate) service_version_id: Uuid,
    pub(crate) option_key: String,
    pub(crate) option_kind: String,
    pub(crate) allowed_values: Option<Value>,
    pub(crate) default_value: Option<Value>,
    pub(crate) is_required: bool,
    pub(crate) is_fanout_driver: bool,
    pub(crate) fanout_axis: String,
    pub(crate) default_source_kind: String,
    pub(crate) source_path: Option<String>,
    pub(crate) fallback_policy: Value,
    pub(crate) override_policy: String,
    pub(crate) lifecycle_status: String,
    pub(crate) description: Option<String>,
}

/// Current product-service option override row.
#[derive(Debug, Clone, FromRow)]
pub(crate) struct ProductServiceOptionOverrideRow {
    pub(crate) override_id: Uuid,
    pub(crate) product_id: Uuid,
    pub(crate) service_id: Uuid,
    pub(crate) service_option_def_id: Uuid,
    pub(crate) default_value_override: Option<Value>,
    pub(crate) allowed_values_override: Option<Value>,
    pub(crate) is_required_override: Option<bool>,
    pub(crate) source_precedence_override: Option<Value>,
    pub(crate) activation_condition_ref: Option<Uuid>,
    pub(crate) effective_from: DateTime<Utc>,
    pub(crate) effective_to: Option<DateTime<Utc>>,
}

/// Runtime service option binding row.
#[derive(Debug, Clone, FromRow)]
pub(crate) struct CbuServiceOptionBindingRow {
    pub(crate) binding_id: Uuid,
    pub(crate) cbu_id: Uuid,
    pub(crate) product_id: Option<Uuid>,
    pub(crate) service_id: Uuid,
    pub(crate) service_version_id: Uuid,
    pub(crate) service_option_def_id: Uuid,
    pub(crate) option_key: String,
    pub(crate) value: Value,
    pub(crate) source_kind: String,
    pub(crate) source_ref: Option<Value>,
    pub(crate) source_version: Option<String>,
    pub(crate) value_hash: String,
    pub(crate) coherence_status: String,
    pub(crate) is_locked: bool,
    pub(crate) valid_from: DateTime<Utc>,
    pub(crate) valid_to: Option<DateTime<Utc>>,
    pub(crate) supersedes_binding_id: Option<Uuid>,
    pub(crate) activation_run_id: Option<Uuid>,
}

/// Resource eligibility constraint row.
#[derive(Debug, Clone, FromRow)]
pub(crate) struct ResourceOptionConstraintRow {
    pub(crate) constraint_id: Uuid,
    pub(crate) service_id: Uuid,
    pub(crate) resource_id: Uuid,
    pub(crate) service_option_def_id: Uuid,
    pub(crate) supported_values: Value,
    pub(crate) match_operator: String,
    pub(crate) priority: i32,
    pub(crate) is_required_for_coverage: bool,
    pub(crate) is_active: bool,
}

/// Resource fan-out rule row.
#[derive(Debug, Clone, FromRow)]
pub(crate) struct ResourceFanoutRuleRow {
    pub(crate) fanout_rule_id: Uuid,
    pub(crate) service_id: Uuid,
    pub(crate) resource_id: Uuid,
    pub(crate) service_option_def_id: Option<Uuid>,
    pub(crate) fanout_axis: String,
    pub(crate) fanout_mode: String,
    pub(crate) group_by_policy: Value,
    pub(crate) shared_when_null: bool,
    pub(crate) priority: i32,
    pub(crate) is_active: bool,
}

/// Runtime activation run row.
#[derive(Debug, Clone, FromRow)]
pub(crate) struct ActivationRunRow {
    pub(crate) activation_run_id: Uuid,
    pub(crate) cbu_id: Uuid,
    pub(crate) product_id: Option<Uuid>,
    pub(crate) run_kind: String,
    pub(crate) status: String,
    pub(crate) triggered_by: Option<String>,
    pub(crate) started_at: DateTime<Utc>,
    pub(crate) completed_at: Option<DateTime<Utc>>,
    pub(crate) failed_at: Option<DateTime<Utc>>,
    pub(crate) failure_reason: Option<String>,
    pub(crate) input_snapshot: Value,
    pub(crate) result_summary: Value,
}

/// Resource instance option-lineage row.
#[derive(Debug, Clone, FromRow)]
pub(crate) struct ResourceInstanceOptionLineageRow {
    pub(crate) lineage_id: Uuid,
    pub(crate) resource_instance_id: Uuid,
    pub(crate) binding_id: Uuid,
    pub(crate) contribution_type: String,
    pub(crate) fanout_axis: Option<String>,
    pub(crate) fanout_value: Option<Value>,
}

/// Repository for service-options framework tables.
#[derive(Debug, Clone)]
pub(crate) struct ServiceOptionsRepository {
    pool: PgPool,
}

impl ServiceOptionsRepository {
    /// Create a repository backed by a Postgres pool.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool) {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _pool = repo.pool();
    /// # }
    /// ```
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Return the backing Postgres pool.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool) {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _ = repo.pool();
    /// # }
    /// ```
    pub(crate) fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get the current published service version for a service.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, service_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _version_id = repo.current_published_service_version(service_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn current_published_service_version(&self, service_id: Uuid) -> Result<Uuid> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT id
            FROM "ob-poc".service_versions
            WHERE service_id = $1
              AND lifecycle_status = 'published'
            ORDER BY published_at DESC NULLS LAST, created_at DESC
            LIMIT 1
            "#,
        )
        .bind(service_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| anyhow!("no published service version for service {service_id}"))
    }

    /// List active option definitions for a service version.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, service_version_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _defs = repo.active_option_defs_for_version(service_version_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn active_option_defs_for_version(
        &self,
        service_version_id: Uuid,
    ) -> Result<Vec<ServiceOptionDefRow>> {
        sqlx::query_as::<_, ServiceOptionDefRow>(
            r#"
            SELECT service_option_def_id, service_id, service_version_id, option_key,
                   option_kind, allowed_values, default_value, is_required,
                   is_fanout_driver, fanout_axis, default_source_kind, source_path,
                   fallback_policy, override_policy, lifecycle_status, description
            FROM "ob-poc".service_option_defs
            WHERE service_version_id = $1
              AND lifecycle_status = 'active'
            ORDER BY option_key
            "#,
        )
        .bind(service_version_id)
        .fetch_all(&self.pool)
        .await
        .context("loading active service option definitions")
    }

    /// List current product-service overrides for a product and service.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, product_id: uuid::Uuid, service_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _rows = repo.current_overrides(product_id, service_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn current_overrides(
        &self,
        product_id: Uuid,
        service_id: Uuid,
    ) -> Result<Vec<ProductServiceOptionOverrideRow>> {
        sqlx::query_as::<_, ProductServiceOptionOverrideRow>(
            r#"
            SELECT override_id, product_id, service_id, service_option_def_id,
                   default_value_override, allowed_values_override, is_required_override,
                   source_precedence_override, activation_condition_ref,
                   effective_from, effective_to
            FROM "ob-poc".product_service_option_overrides
            WHERE product_id = $1
              AND service_id = $2
              AND effective_to IS NULL
            ORDER BY effective_from DESC
            "#,
        )
        .bind(product_id)
        .bind(service_id)
        .fetch_all(&self.pool)
        .await
        .context("loading current product-service option overrides")
    }

    /// List current option bindings for a CBU/service pair.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, cbu_id: uuid::Uuid, service_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _bindings = repo.current_bindings_for_cbu_service(cbu_id, service_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn current_bindings_for_cbu_service(
        &self,
        cbu_id: Uuid,
        service_id: Uuid,
    ) -> Result<Vec<CbuServiceOptionBindingRow>> {
        sqlx::query_as::<_, CbuServiceOptionBindingRow>(
            r#"
            SELECT binding_id, cbu_id, product_id, service_id, service_version_id,
                   service_option_def_id, option_key, value, source_kind, source_ref,
                   source_version, value_hash, coherence_status, is_locked,
                   valid_from, valid_to, supersedes_binding_id, activation_run_id
            FROM "ob-poc".cbu_service_option_bindings
            WHERE cbu_id = $1
              AND service_id = $2
              AND valid_to IS NULL
            ORDER BY option_key
            "#,
        )
        .bind(cbu_id)
        .bind(service_id)
        .fetch_all(&self.pool)
        .await
        .context("loading current CBU service option bindings")
    }

    /// List active resource eligibility constraints for a service.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, service_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _constraints = repo.active_constraints_for_service(service_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn active_constraints_for_service(
        &self,
        service_id: Uuid,
    ) -> Result<Vec<ResourceOptionConstraintRow>> {
        sqlx::query_as::<_, ResourceOptionConstraintRow>(
            r#"
            SELECT constraint_id, service_id, resource_id, service_option_def_id,
                   supported_values, match_operator, priority, is_required_for_coverage,
                   is_active
            FROM "ob-poc".service_resource_option_constraints
            WHERE service_id = $1
              AND is_active
            ORDER BY priority, resource_id
            "#,
        )
        .bind(service_id)
        .fetch_all(&self.pool)
        .await
        .context("loading active service resource option constraints")
    }

    /// List active resource fan-out rules for a service.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, service_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _rules = repo.active_fanout_rules_for_service(service_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn active_fanout_rules_for_service(
        &self,
        service_id: Uuid,
    ) -> Result<Vec<ResourceFanoutRuleRow>> {
        sqlx::query_as::<_, ResourceFanoutRuleRow>(
            r#"
            SELECT fanout_rule_id, service_id, resource_id, service_option_def_id,
                   fanout_axis, fanout_mode, group_by_policy, shared_when_null,
                   priority, is_active
            FROM "ob-poc".service_resource_fanout_rules
            WHERE service_id = $1
              AND is_active
            ORDER BY priority, resource_id
            "#,
        )
        .bind(service_id)
        .fetch_all(&self.pool)
        .await
        .context("loading active service resource fanout rules")
    }

    /// Insert a service option definition and return its id.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, def: sem_os_postgres::service_options::NewServiceOptionDef) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _def_id = repo.insert_service_option_def(&def).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn insert_service_option_def(
        &self,
        def: &NewServiceOptionDef,
    ) -> Result<Uuid> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".service_option_defs
                (service_id, service_version_id, option_key, option_kind,
                 allowed_values, default_value, is_required, is_fanout_driver,
                 fanout_axis, default_source_kind, source_path, fallback_policy,
                 override_policy, lifecycle_status, description)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                 $11, $12, $13, $14, $15)
            ON CONFLICT (service_version_id, option_key)
            DO UPDATE SET option_kind = EXCLUDED.option_kind,
                          allowed_values = EXCLUDED.allowed_values,
                          default_value = EXCLUDED.default_value,
                          is_required = EXCLUDED.is_required,
                          is_fanout_driver = EXCLUDED.is_fanout_driver,
                          fanout_axis = EXCLUDED.fanout_axis,
                          default_source_kind = EXCLUDED.default_source_kind,
                          source_path = EXCLUDED.source_path,
                          fallback_policy = EXCLUDED.fallback_policy,
                          override_policy = EXCLUDED.override_policy,
                          lifecycle_status = EXCLUDED.lifecycle_status,
                          description = EXCLUDED.description,
                          updated_at = now()
            RETURNING service_option_def_id
            "#,
        )
        .bind(def.service_id)
        .bind(def.service_version_id)
        .bind(&def.option_key)
        .bind(&def.option_kind)
        .bind(&def.allowed_values)
        .bind(&def.default_value)
        .bind(def.is_required)
        .bind(def.is_fanout_driver)
        .bind(def.fanout_axis.as_db_str())
        .bind(def.default_source_kind.as_db_str())
        .bind(&def.source_path)
        .bind(&def.fallback_policy)
        .bind(&def.override_policy)
        .bind(&def.lifecycle_status)
        .bind(&def.description)
        .fetch_one(&self.pool)
        .await
        .context("inserting service option definition")
    }

    /// Insert a product-service option override and return its id.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, row: sem_os_postgres::service_options::NewProductServiceOptionOverride) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _override_id = repo.insert_product_service_override(&row).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn insert_product_service_override(
        &self,
        row: &NewProductServiceOptionOverride,
    ) -> Result<Uuid> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".product_service_option_overrides
                (product_id, service_id, service_option_def_id, default_value_override,
                 allowed_values_override, is_required_override, source_precedence_override,
                 activation_condition_ref, effective_from, supersedes_override_id)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, COALESCE($9, now()), $10)
            RETURNING override_id
            "#,
        )
        .bind(row.product_id)
        .bind(row.service_id)
        .bind(row.service_option_def_id)
        .bind(&row.default_value_override)
        .bind(&row.allowed_values_override)
        .bind(row.is_required_override)
        .bind(&row.source_precedence_override)
        .bind(row.activation_condition_ref)
        .bind(row.effective_from)
        .bind(row.supersedes_override_id)
        .fetch_one(&self.pool)
        .await
        .context("inserting product-service option override")
    }

    /// Insert a service/resource eligibility constraint and return its id.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, row: sem_os_postgres::service_options::NewResourceOptionConstraint) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _constraint_id = repo.insert_resource_option_constraint(&row).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn insert_resource_option_constraint(
        &self,
        row: &NewResourceOptionConstraint,
    ) -> Result<Uuid> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".service_resource_option_constraints
                (service_id, resource_id, service_option_def_id, supported_values,
                 match_operator, priority, is_required_for_coverage, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING constraint_id
            "#,
        )
        .bind(row.service_id)
        .bind(row.resource_id)
        .bind(row.service_option_def_id)
        .bind(&row.supported_values)
        .bind(&row.match_operator)
        .bind(row.priority)
        .bind(row.is_required_for_coverage)
        .bind(row.is_active)
        .fetch_one(&self.pool)
        .await
        .context("inserting resource option constraint")
    }

    /// Insert a service/resource fan-out rule and return its id.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, row: sem_os_postgres::service_options::NewResourceFanoutRule) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _rule_id = repo.insert_fanout_rule(&row).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn insert_fanout_rule(&self, row: &NewResourceFanoutRule) -> Result<Uuid> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".service_resource_fanout_rules
                (service_id, resource_id, service_option_def_id, fanout_axis,
                 fanout_mode, group_by_policy, shared_when_null, priority, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING fanout_rule_id
            "#,
        )
        .bind(row.service_id)
        .bind(row.resource_id)
        .bind(row.service_option_def_id)
        .bind(row.fanout_axis.as_db_str())
        .bind(row.fanout_mode.as_db_str())
        .bind(&row.group_by_policy)
        .bind(row.shared_when_null)
        .bind(row.priority)
        .bind(row.is_active)
        .fetch_one(&self.pool)
        .await
        .context("inserting fanout rule")
    }

    /// Insert an activation run and return its id.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, cbu_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _run_id = repo
    ///     .insert_activation_run(cbu_id, None, "bind_options", None, serde_json::json!({}))
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn insert_activation_run(
        &self,
        cbu_id: Uuid,
        product_id: Option<Uuid>,
        run_kind: &str,
        triggered_by: Option<&str>,
        input_snapshot: Value,
    ) -> Result<Uuid> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".activation_runs
                (cbu_id, product_id, run_kind, triggered_by, input_snapshot)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING activation_run_id
            "#,
        )
        .bind(cbu_id)
        .bind(product_id)
        .bind(run_kind)
        .bind(triggered_by)
        .bind(input_snapshot)
        .fetch_one(&self.pool)
        .await
        .context("inserting activation run")
    }

    /// Mark an activation run as succeeded.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, run_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// repo.complete_activation_run(run_id, serde_json::json!({"status": "ok"})).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn complete_activation_run(
        &self,
        run_id: Uuid,
        result_summary: Value,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".activation_runs
            SET status = 'succeeded',
                completed_at = now(),
                result_summary = $2,
                updated_at = now()
            WHERE activation_run_id = $1
            "#,
        )
        .bind(run_id)
        .bind(result_summary)
        .execute(&self.pool)
        .await
        .context("completing activation run")?;

        Ok(())
    }

    /// Mark an activation run as failed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, run_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// repo.fail_activation_run(run_id, "missing required option").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn fail_activation_run(&self, run_id: Uuid, reason: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE "ob-poc".activation_runs
            SET status = 'failed',
                failed_at = now(),
                failure_reason = $2,
                updated_at = now()
            WHERE activation_run_id = $1
            "#,
        )
        .bind(run_id)
        .bind(reason)
        .execute(&self.pool)
        .await
        .context("failing activation run")?;

        Ok(())
    }

    /// Insert a new binding row.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, binding: sem_os_postgres::service_options::NewOptionBinding) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _binding_id = repo.insert_binding(&binding).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn insert_binding(&self, binding: &NewOptionBinding) -> Result<Uuid> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".cbu_service_option_bindings
                (cbu_id, product_id, service_id, service_version_id,
                 service_option_def_id, option_key, value, source_kind, source_ref,
                 source_version, value_hash, coherence_status, is_locked,
                 supersedes_binding_id, activation_run_id)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                 $11, $12, $13, $14, $15)
            RETURNING binding_id
            "#,
        )
        .bind(binding.cbu_id)
        .bind(binding.product_id)
        .bind(binding.service_id)
        .bind(binding.service_version_id)
        .bind(binding.service_option_def_id)
        .bind(&binding.option_key)
        .bind(&binding.value)
        .bind(binding.source_kind.as_db_str())
        .bind(&binding.source_ref)
        .bind(&binding.source_version)
        .bind(&binding.value_hash)
        .bind(&binding.coherence_status)
        .bind(binding.is_locked)
        .bind(binding.supersedes_binding_id)
        .bind(binding.activation_run_id)
        .fetch_one(&self.pool)
        .await
        .context("inserting option binding")
    }

    /// Supersede an existing current binding and insert its replacement.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, previous_id: uuid::Uuid, binding: sem_os_postgres::service_options::NewOptionBinding) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _binding_id = repo.supersede_binding(previous_id, &binding).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn supersede_binding(
        &self,
        previous_binding_id: Uuid,
        binding: &NewOptionBinding,
    ) -> Result<Uuid> {
        let mut tx = self
            .pool
            .begin()
            .await
            .context("starting binding supersession")?;

        sqlx::query(
            r#"
            UPDATE "ob-poc".cbu_service_option_bindings
            SET valid_to = now(),
                coherence_status = 'stale',
                updated_at = now()
            WHERE binding_id = $1
              AND valid_to IS NULL
            "#,
        )
        .bind(previous_binding_id)
        .execute(&mut *tx)
        .await
        .context("closing previous option binding")?;

        let new_binding_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".cbu_service_option_bindings
                (cbu_id, product_id, service_id, service_version_id,
                 service_option_def_id, option_key, value, source_kind, source_ref,
                 source_version, value_hash, coherence_status, is_locked,
                 supersedes_binding_id, activation_run_id)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                 $11, $12, $13, $14, $15)
            RETURNING binding_id
            "#,
        )
        .bind(binding.cbu_id)
        .bind(binding.product_id)
        .bind(binding.service_id)
        .bind(binding.service_version_id)
        .bind(binding.service_option_def_id)
        .bind(&binding.option_key)
        .bind(&binding.value)
        .bind(binding.source_kind.as_db_str())
        .bind(&binding.source_ref)
        .bind(&binding.source_version)
        .bind(&binding.value_hash)
        .bind(&binding.coherence_status)
        .bind(binding.is_locked)
        .bind(previous_binding_id)
        .bind(binding.activation_run_id)
        .fetch_one(&mut *tx)
        .await
        .context("inserting superseding option binding")?;

        tx.commit()
            .await
            .context("committing binding supersession")?;
        Ok(new_binding_id)
    }

    /// Insert resource-instance option lineage.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, lineage: sem_os_postgres::service_options::NewResourceInstanceOptionLineage) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _lineage_id = repo.insert_lineage(&lineage).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn insert_lineage(
        &self,
        lineage: &NewResourceInstanceOptionLineage,
    ) -> Result<Uuid> {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO "ob-poc".cbu_resource_instance_option_lineage
                (resource_instance_id, binding_id, contribution_type, fanout_axis, fanout_value)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (resource_instance_id, binding_id, contribution_type, fanout_axis, fanout_value)
            DO UPDATE SET contribution_type = EXCLUDED.contribution_type
            RETURNING lineage_id
            "#,
        )
        .bind(lineage.resource_instance_id)
        .bind(lineage.binding_id)
        .bind(lineage.contribution_type.as_db_str())
        .bind(lineage.fanout_axis.map(FanoutAxis::as_db_str))
        .bind(&lineage.fanout_value)
        .fetch_one(&self.pool)
        .await
        .context("inserting resource instance option lineage")
    }

    /// List option lineage rows for a resource instance.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn demo(pool: sqlx::PgPool, instance_id: uuid::Uuid) -> anyhow::Result<()> {
    /// let repo = sem_os_postgres::service_options::ServiceOptionsRepository::new(pool);
    /// let _lineage = repo.lineage_for_resource_instance(instance_id).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub(crate) async fn lineage_for_resource_instance(
        &self,
        resource_instance_id: Uuid,
    ) -> Result<Vec<ResourceInstanceOptionLineageRow>> {
        sqlx::query_as::<_, ResourceInstanceOptionLineageRow>(
            r#"
            SELECT lineage_id, resource_instance_id, binding_id, contribution_type,
                   fanout_axis, fanout_value
            FROM "ob-poc".cbu_resource_instance_option_lineage
            WHERE resource_instance_id = $1
            ORDER BY contribution_type, fanout_axis NULLS FIRST
            "#,
        )
        .bind(resource_instance_id)
        .fetch_all(&self.pool)
        .await
        .context("loading resource instance option lineage")
    }
}

/// Input row for inserting a CBU option binding.
#[derive(Debug, Clone)]
pub(crate) struct NewOptionBinding {
    pub(crate) cbu_id: Uuid,
    pub(crate) product_id: Option<Uuid>,
    pub(crate) service_id: Uuid,
    pub(crate) service_version_id: Uuid,
    pub(crate) service_option_def_id: Uuid,
    pub(crate) option_key: String,
    pub(crate) value: Value,
    pub(crate) source_kind: SourceKind,
    pub(crate) source_ref: Option<Value>,
    pub(crate) source_version: Option<String>,
    pub(crate) value_hash: String,
    pub(crate) coherence_status: String,
    pub(crate) is_locked: bool,
    pub(crate) supersedes_binding_id: Option<Uuid>,
    pub(crate) activation_run_id: Option<Uuid>,
}

/// Input row for declaring or updating a service option definition.
#[derive(Debug, Clone)]
pub(crate) struct NewServiceOptionDef {
    pub(crate) service_id: Uuid,
    pub(crate) service_version_id: Uuid,
    pub(crate) option_key: String,
    pub(crate) option_kind: String,
    pub(crate) allowed_values: Option<Value>,
    pub(crate) default_value: Option<Value>,
    pub(crate) is_required: bool,
    pub(crate) is_fanout_driver: bool,
    pub(crate) fanout_axis: FanoutAxis,
    pub(crate) default_source_kind: SourceKind,
    pub(crate) source_path: Option<String>,
    pub(crate) fallback_policy: Value,
    pub(crate) override_policy: String,
    pub(crate) lifecycle_status: String,
    pub(crate) description: Option<String>,
}

/// Input row for product-service option overrides.
#[derive(Debug, Clone)]
pub(crate) struct NewProductServiceOptionOverride {
    pub(crate) product_id: Uuid,
    pub(crate) service_id: Uuid,
    pub(crate) service_option_def_id: Uuid,
    pub(crate) default_value_override: Option<Value>,
    pub(crate) allowed_values_override: Option<Value>,
    pub(crate) is_required_override: Option<bool>,
    pub(crate) source_precedence_override: Option<Value>,
    pub(crate) activation_condition_ref: Option<Uuid>,
    pub(crate) effective_from: Option<DateTime<Utc>>,
    pub(crate) supersedes_override_id: Option<Uuid>,
}

/// Input row for service/resource option eligibility.
#[derive(Debug, Clone)]
pub(crate) struct NewResourceOptionConstraint {
    pub(crate) service_id: Uuid,
    pub(crate) resource_id: Uuid,
    pub(crate) service_option_def_id: Uuid,
    pub(crate) supported_values: Value,
    pub(crate) match_operator: String,
    pub(crate) priority: i32,
    pub(crate) is_required_for_coverage: bool,
    pub(crate) is_active: bool,
}

/// Input row for service/resource fan-out planning.
#[derive(Debug, Clone)]
pub(crate) struct NewResourceFanoutRule {
    pub(crate) service_id: Uuid,
    pub(crate) resource_id: Uuid,
    pub(crate) service_option_def_id: Option<Uuid>,
    pub(crate) fanout_axis: FanoutAxis,
    pub(crate) fanout_mode: FanoutMode,
    pub(crate) group_by_policy: Value,
    pub(crate) shared_when_null: bool,
    pub(crate) priority: i32,
    pub(crate) is_active: bool,
}

/// Contribution type recorded for resource-instance option lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LineageContributionType {
    Eligibility,
    Fanout,
    AttributeSource,
}

impl LineageContributionType {
    /// Return the database representation for this lineage contribution.
    ///
    /// # Examples
    ///
    /// ```
    /// use sem_os_postgres::service_options::LineageContributionType;
    ///
    /// assert_eq!(LineageContributionType::Fanout.as_db_str(), "fanout");
    /// ```
    pub(crate) fn as_db_str(self) -> &'static str {
        match self {
            LineageContributionType::Eligibility => "eligibility",
            LineageContributionType::Fanout => "fanout",
            LineageContributionType::AttributeSource => "attribute_source",
        }
    }
}

/// Input row for recording resource-instance option lineage.
#[derive(Debug, Clone)]
pub(crate) struct NewResourceInstanceOptionLineage {
    pub(crate) resource_instance_id: Uuid,
    pub(crate) binding_id: Uuid,
    pub(crate) contribution_type: LineageContributionType,
    pub(crate) fanout_axis: Option<FanoutAxis>,
    pub(crate) fanout_value: Option<Value>,
}

/// Source value available to the option resolver.
#[derive(Debug, Clone)]
pub(crate) struct OptionSourceValue {
    pub(crate) source_kind: SourceKind,
    pub(crate) source_path: String,
    pub(crate) value: Value,
    pub(crate) source_ref: Option<Value>,
    pub(crate) source_version: Option<String>,
}

/// Resolved option value ready to become a binding row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedOptionValue {
    pub(crate) service_option_def_id: Uuid,
    pub(crate) option_key: String,
    pub(crate) value: Value,
    pub(crate) source_kind: SourceKind,
    pub(crate) source_ref: Option<Value>,
    pub(crate) source_version: Option<String>,
    pub(crate) value_hash: String,
}

/// Pure option resolution helper.
#[derive(Debug, Default, Clone)]
pub(crate) struct OptionResolver;

impl OptionResolver {
    /// Create an option resolver.
    ///
    /// # Examples
    ///
    /// ```
    /// let _resolver = sem_os_postgres::service_options::OptionResolver::new();
    /// ```
    pub(crate) fn new() -> Self {
        Self
    }

    /// Resolve a single option from available sources, override defaults, or the service default.
    ///
    /// # Examples
    ///
    /// ```
    /// use sem_os_postgres::service_options::{OptionResolver, OptionSourceValue, SourceKind};
    /// use serde_json::json;
    ///
    /// let source = OptionSourceValue {
    ///     source_kind: SourceKind::CbuProfile,
    ///     source_path: "markets".into(),
    ///     value: json!(["US_EQUITY"]),
    ///     source_ref: None,
    ///     source_version: Some("v1".into()),
    /// };
    ///
    /// assert_eq!(source.value, json!(["US_EQUITY"]));
    /// let _resolver = OptionResolver::new();
    /// ```
    pub(crate) fn resolve(
        &self,
        def: &ServiceOptionDefRow,
        override_row: Option<&ProductServiceOptionOverrideRow>,
        sources: &[OptionSourceValue],
    ) -> Result<ResolvedOptionValue> {
        let source_kind = SourceKind::try_from(def.default_source_kind.as_str())?;
        let source_path = def.source_path.as_deref();
        let from_source = sources.iter().find(|source| {
            source.source_kind == source_kind
                && source_path
                    .map(|path| path == source.source_path)
                    .unwrap_or(true)
        });

        let (value, resolved_source_kind, source_ref, source_version) = if let Some(source) =
            from_source
        {
            (
                source.value.clone(),
                source.source_kind,
                source.source_ref.clone(),
                source.source_version.clone(),
            )
        } else if let Some(value) = override_row.and_then(|row| row.default_value_override.clone())
        {
            (value, SourceKind::ProductOption, None, None)
        } else if let Some(value) = def.default_value.clone() {
            (value, source_kind, None, None)
        } else if def.is_required
            || override_row
                .and_then(|row| row.is_required_override)
                .unwrap_or(false)
        {
            bail!(
                "required option `{}` has no resolvable value",
                def.option_key
            );
        } else {
            (Value::Null, source_kind, None, None)
        };

        Ok(ResolvedOptionValue {
            service_option_def_id: def.service_option_def_id,
            option_key: def.option_key.clone(),
            value_hash: hash_canonical_json(&value),
            value,
            source_kind: resolved_source_kind,
            source_ref,
            source_version,
        })
    }
}

/// Validation gap emitted by coverage checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CoverageGap {
    pub(crate) level: u8,
    pub(crate) code: String,
    pub(crate) message: String,
}

/// Pure coverage validator for option/resource planning inputs.
#[derive(Debug, Default, Clone)]
pub(crate) struct CoverageValidator;

impl CoverageValidator {
    /// Create a coverage validator.
    ///
    /// # Examples
    ///
    /// ```
    /// let _validator = sem_os_postgres::service_options::CoverageValidator::new();
    /// ```
    pub(crate) fn new() -> Self {
        Self
    }

    /// Validate that required option definitions have non-null bindings.
    ///
    /// # Examples
    ///
    /// ```
    /// let validator = sem_os_postgres::service_options::CoverageValidator::new();
    /// let gaps = validator.required_binding_gaps(&[], &[]);
    /// assert!(gaps.is_empty());
    /// ```
    pub(crate) fn required_binding_gaps(
        &self,
        defs: &[ServiceOptionDefRow],
        bindings: &[ResolvedOptionValue],
    ) -> Vec<CoverageGap> {
        defs.iter()
            .filter(|def| def.is_required)
            .filter(|def| {
                !bindings.iter().any(|binding| {
                    binding.service_option_def_id == def.service_option_def_id
                        && !binding.value.is_null()
                })
            })
            .map(|def| CoverageGap {
                level: 7,
                code: "missing_required_option_binding".to_string(),
                message: format!("required option `{}` has no binding", def.option_key),
            })
            .collect()
    }
}

/// Planned resource instance before materialisation through `service-resource.provision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedResourceInstance {
    pub(crate) service_id: Uuid,
    pub(crate) resource_id: Uuid,
    pub(crate) fanout_axis: FanoutAxis,
    pub(crate) fanout_value: Option<Value>,
}

/// Pure resource fan-out planner.
#[derive(Debug, Default, Clone)]
pub(crate) struct ResourceFanoutPlanner;

impl ResourceFanoutPlanner {
    /// Create a fan-out planner.
    ///
    /// # Examples
    ///
    /// ```
    /// let _planner = sem_os_postgres::service_options::ResourceFanoutPlanner::new();
    /// ```
    pub(crate) fn new() -> Self {
        Self
    }

    /// Build planned resource instances from fan-out rules and resolved option bindings.
    ///
    /// # Examples
    ///
    /// ```
    /// let planner = sem_os_postgres::service_options::ResourceFanoutPlanner::new();
    /// let planned = planner.plan(&[], &[]).unwrap();
    /// assert!(planned.is_empty());
    /// ```
    pub(crate) fn plan(
        &self,
        rules: &[ResourceFanoutRuleRow],
        bindings: &[ResolvedOptionValue],
    ) -> Result<Vec<PlannedResourceInstance>> {
        let mut planned = Vec::new();

        for rule in rules.iter().filter(|rule| rule.is_active) {
            let axis = FanoutAxis::try_from(rule.fanout_axis.as_str())?;
            let mode = FanoutMode::try_from(rule.fanout_mode.as_str())?;

            match mode {
                FanoutMode::Shared => {
                    planned.push(PlannedResourceInstance {
                        service_id: rule.service_id,
                        resource_id: rule.resource_id,
                        fanout_axis: axis,
                        fanout_value: None,
                    });
                }
                FanoutMode::PerValue => {
                    let Some(option_def_id) = rule.service_option_def_id else {
                        bail!("per_value fanout rule requires service_option_def_id");
                    };
                    let binding = bindings
                        .iter()
                        .find(|binding| binding.service_option_def_id == option_def_id)
                        .ok_or_else(|| {
                            anyhow!("missing binding for fanout rule {}", rule.fanout_rule_id)
                        })?;
                    for value in fanout_values(&binding.value, rule.shared_when_null) {
                        planned.push(PlannedResourceInstance {
                            service_id: rule.service_id,
                            resource_id: rule.resource_id,
                            fanout_axis: axis,
                            fanout_value: value,
                        });
                    }
                }
                FanoutMode::Grouped | FanoutMode::Conditional => {
                    bail!(
                        "{} fanout requires policy execution and is not implemented in the pure v1 planner",
                        rule.fanout_mode
                    );
                }
            }
        }

        Ok(planned)
    }
}

/// Compute SHA-256 over canonical JSON.
///
/// # Examples
///
/// ```
/// use sem_os_postgres::service_options::hash_canonical_json;
/// use serde_json::json;
///
/// let left = json!({"b": 2, "a": 1});
/// let right = json!({"a": 1, "b": 2});
/// assert_eq!(hash_canonical_json(&left), hash_canonical_json(&right));
/// ```
pub(crate) fn hash_canonical_json(value: &Value) -> String {
    let canonical = canonical_json(value);
    let digest = Sha256::digest(canonical.as_bytes());
    hex::encode(digest)
}

fn canonical_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).expect("string serialization"),
        Value::Array(values) => {
            let rendered: Vec<String> = values.iter().map(canonical_json).collect();
            format!("[{}]", rendered.join(","))
        }
        Value::Object(map) => canonical_object(map),
    }
}

fn canonical_object(map: &Map<String, Value>) -> String {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    let fields: Vec<String> = keys
        .into_iter()
        .map(|key| {
            let key_json = serde_json::to_string(key).expect("object key serialization");
            let value_json = canonical_json(&map[key]);
            format!("{key_json}:{value_json}")
        })
        .collect();
    format!("{{{}}}", fields.join(","))
}

fn fanout_values(value: &Value, shared_when_null: bool) -> Vec<Option<Value>> {
    match value {
        Value::Null if shared_when_null => vec![None],
        Value::Null => Vec::new(),
        Value::Array(values) => values.iter().cloned().map(Some).collect(),
        other => vec![Some(other.clone())],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn option_def(id: Uuid, required: bool, default: Option<Value>) -> ServiceOptionDefRow {
        ServiceOptionDefRow {
            service_option_def_id: id,
            service_id: Uuid::new_v4(),
            service_version_id: Uuid::new_v4(),
            option_key: "markets".to_string(),
            option_kind: "multi_choice".to_string(),
            allowed_values: None,
            default_value: default,
            is_required: required,
            is_fanout_driver: true,
            fanout_axis: "market".to_string(),
            default_source_kind: "cbu_profile".to_string(),
            source_path: Some("markets".to_string()),
            fallback_policy: json!([]),
            override_policy: "allowed_with_reason".to_string(),
            lifecycle_status: "active".to_string(),
            description: None,
        }
    }

    fn fanout_rule(option_id: Uuid, mode: &str) -> ResourceFanoutRuleRow {
        ResourceFanoutRuleRow {
            fanout_rule_id: Uuid::new_v4(),
            service_id: Uuid::new_v4(),
            resource_id: Uuid::new_v4(),
            service_option_def_id: Some(option_id),
            fanout_axis: "market".to_string(),
            fanout_mode: mode.to_string(),
            group_by_policy: json!({}),
            shared_when_null: true,
            priority: 100,
            is_active: true,
        }
    }

    #[test]
    fn canonical_hash_is_key_order_stable() {
        let left = json!({"b": 2, "a": {"d": 4, "c": 3}});
        let right = json!({"a": {"c": 3, "d": 4}, "b": 2});
        assert_eq!(hash_canonical_json(&left), hash_canonical_json(&right));
    }

    #[test]
    fn canonical_hash_changes_with_value() {
        let left = json!({"a": 1});
        let right = json!({"a": 2});
        assert_ne!(hash_canonical_json(&left), hash_canonical_json(&right));
    }

    #[test]
    fn resolver_prefers_matching_source_over_default() {
        let def_id = Uuid::new_v4();
        let def = option_def(def_id, true, Some(json!(["EU_EQUITY"])));
        let source = OptionSourceValue {
            source_kind: SourceKind::CbuProfile,
            source_path: "markets".to_string(),
            value: json!(["US_EQUITY"]),
            source_ref: Some(json!({"path": "cbu_profile.markets"})),
            source_version: Some("v12".to_string()),
        };

        let resolved = OptionResolver::new()
            .resolve(&def, None, &[source])
            .expect("resolves from source");

        assert_eq!(resolved.value, json!(["US_EQUITY"]));
        assert_eq!(resolved.source_kind, SourceKind::CbuProfile);
        assert_eq!(resolved.service_option_def_id, def_id);
    }

    #[test]
    fn resolver_fails_required_option_without_value() {
        let def = option_def(Uuid::new_v4(), true, None);
        let error = OptionResolver::new()
            .resolve(&def, None, &[])
            .expect_err("required option without source/default should fail");
        assert!(error.to_string().contains("required option"));
    }

    #[test]
    fn validator_reports_missing_required_binding() {
        let def = option_def(Uuid::new_v4(), true, None);
        let gaps = CoverageValidator::new().required_binding_gaps(&[def], &[]);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].code, "missing_required_option_binding");
    }

    #[test]
    fn fanout_planner_expands_array_values() {
        let option_id = Uuid::new_v4();
        let binding = ResolvedOptionValue {
            service_option_def_id: option_id,
            option_key: "markets".to_string(),
            value: json!(["US_EQUITY", "EU_EQUITY"]),
            source_kind: SourceKind::CbuProfile,
            source_ref: None,
            source_version: None,
            value_hash: hash_canonical_json(&json!(["US_EQUITY", "EU_EQUITY"])),
        };
        let rule = fanout_rule(option_id, "per_value");

        let planned = ResourceFanoutPlanner::new()
            .plan(&[rule], &[binding])
            .expect("plans fanout");

        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].fanout_axis, FanoutAxis::Market);
        assert_eq!(planned[0].fanout_value, Some(json!("US_EQUITY")));
        assert_eq!(planned[1].fanout_value, Some(json!("EU_EQUITY")));
    }

    #[test]
    fn shared_fanout_emits_single_shared_instance() {
        let rule = ResourceFanoutRuleRow {
            fanout_mode: "shared".to_string(),
            service_option_def_id: None,
            ..fanout_rule(Uuid::new_v4(), "shared")
        };

        let planned = ResourceFanoutPlanner::new()
            .plan(&[rule], &[])
            .expect("plans shared resource");

        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].fanout_value, None);
    }
}
