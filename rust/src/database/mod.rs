//! Database connection and management module
//!
//! This module provides database connection management, connection pooling,
//! and configuration for the DSL architecture.
//!
//! ## Architecture
//! Database operations flow through dsl_v2::DslExecutor which generates SQL
//! from verb definitions. Domain services provide specialized operations.

use sqlx::Row;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use tracing::{info, warn};

pub mod attribute_values_service;
pub mod cbu_entity_roles_service;
pub mod cbu_service;
pub mod crud_service;
pub mod decision_service;
pub mod dictionary_service;
pub mod document_service;
pub mod dsl_repository;
pub mod entity_service;
pub mod fuzzy_search_service;
pub mod generation_log_repository;
pub mod investigation_service;
pub mod monitoring_service;
pub mod product_service;
pub mod resource_instance_service;
pub mod risk_service;
pub mod screening_service;
pub mod service_resource_service;
pub mod service_service;
pub mod session_repository;
pub mod visualization_repository;

// Legacy modules not yet integrated - kept for reference but not compiled
// pub mod attribute_repository;
// pub mod document_type_repository;
// pub mod taxonomy_repository;

// Re-export for convenience
pub use attribute_values_service::{AttributeValueRow, AttributeValuesService};
pub use cbu_entity_roles_service::{CbuEntityRoleExpanded, CbuEntityRolesService, RoleRow};
pub use cbu_service::{CbuRow, CbuService, NewCbuFields};
pub use crud_service::{AssetType, CrudOperation, CrudService, OperationType};
pub use dictionary_service::DictionaryDatabaseService;
pub use document_service::{
    DocumentCatalogEntry, DocumentService, DocumentType, NewDocumentFields,
};
pub use dsl_repository::{DslRepository, DslSaveResult};
pub use entity_service::{
    CbuEntityRoleRow, EntityRow, EntityService, LimitedCompanyRow, NewEntityFields,
    NewLimitedCompanyFields, NewPartnershipFields, NewProperPersonFields, NewTrustFields,
    PartnershipRow, TrustRow,
};
pub use fuzzy_search_service::{
    fuzzy_match_small_list, FuzzyCbuMatch, FuzzyCompanyMatch, FuzzyEntityMatch, FuzzyPersonMatch,
    FuzzySearchResult, FuzzySearchService,
};
pub use generation_log_repository::{
    CompileResult, CorrectionPair, GenerationAttempt, GenerationLogRepository, GenerationLogRow,
    GenerationStatsSummary, LintResult, ParseResult, PromptStats, TrainingPair,
};
pub use product_service::{NewProductFields, ProductRow, ProductService};
pub use resource_instance_service::{
    NewResourceInstance, ResourceInstanceAttributeRow, ResourceInstanceRow,
    ResourceInstanceService, ServiceDeliveryRow, SetInstanceAttribute,
};
pub use service_resource_service::{
    NewServiceResourceFields, ServiceResourceRow, ServiceResourceService,
};
pub use service_service::{NewServiceFields, ServiceRow, ServiceService};
pub use visualization_repository::{
    CbuBasicView, CbuDocumentView, CbuEntityView, CbuRoleView, CbuScreeningView, CbuSummaryView,
    CbuView, ControlRelationshipView, DocumentAttributeView, DocumentTypeView, EntityAttributeView,
    EntityBasicView, EntityCbuView, EntityRoleView, EntityScreeningView, EntityTypeView,
    EntityView, EntityWithRoleView, HoldingView, OfficerView, RoleView, ServiceDeliveryView,
    ShareClassView, VisualizationRepository,
};

// KYC Investigation services
pub use decision_service::{
    DecisionConditionRow, DecisionRow, DecisionService, NewConditionFields, NewDecisionFields,
    SatisfyConditionFields,
};
pub use investigation_service::{
    InvestigationAssignmentRow, InvestigationRow, InvestigationService, NewAssignmentFields,
    NewInvestigationFields,
};
pub use monitoring_service::{
    MonitoringEventRow, MonitoringService, MonitoringSetupFields, MonitoringSetupRow,
    NewMonitoringEventFields, NewScheduledReviewFields, ScheduledReviewRow,
};
pub use risk_service::{
    NewRiskAssessmentFields, NewRiskFlagFields, RiskAssessmentRow, RiskFlagRow, RiskRatingFields,
    RiskService,
};
pub use screening_service::{
    NewAdverseMediaScreeningFields, NewPepScreeningFields, NewSanctionsScreeningFields,
    ScreeningResolutionFields, ScreeningResultFields, ScreeningRow, ScreeningService,
};
pub use session_repository::{
    detect_domain, extract_domains, DslSnapshot, EntityCreated, PersistedSession, SessionEventType,
    SessionRepository, SessionStatus,
};

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://localhost:5432/ob-poc".to_string()),
            max_connections: std::env::var("DATABASE_POOL_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            connection_timeout: Duration::from_secs(30),
            idle_timeout: Some(Duration::from_secs(600)), // 10 minutes
            max_lifetime: Some(Duration::from_secs(1800)), // 30 minutes
        }
    }
}

/// Database connection manager
pub struct DatabaseManager {
    pool: PgPool,
}

impl DatabaseManager {
    /// Create a new database manager with the given configuration
    pub async fn new(config: DatabaseConfig) -> Result<Self, sqlx::Error> {
        info!(
            "Connecting to database: {}",
            mask_database_url(&config.database_url)
        );

        let mut pool_options = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.connection_timeout);

        if let Some(idle_timeout) = config.idle_timeout {
            pool_options = pool_options.idle_timeout(idle_timeout);
        }

        if let Some(max_lifetime) = config.max_lifetime {
            pool_options = pool_options.max_lifetime(max_lifetime);
        }

        let pool = pool_options
            .connect(&config.database_url)
            .await
            .map_err(|e| {
                warn!("Failed to connect to database: {}", e);
                e
            })?;

        info!("Database connection pool created successfully");

        Ok(Self { pool })
    }

    /// Create a new database manager with default configuration
    pub async fn with_default_config() -> Result<Self, sqlx::Error> {
        let config = DatabaseConfig::default();
        Self::new(config).await
    }

    /// Create a new database manager from an existing pool
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new dictionary database service using this database connection
    pub fn dictionary_service(&self) -> DictionaryDatabaseService {
        DictionaryDatabaseService::new(self.pool.clone())
    }

    /// Test database connectivity
    pub async fn test_connection(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map(|_| ())
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> Result<(), sqlx::migrate::MigrateError> {
        info!("Running database migrations");

        // Verify the schema exists
        let tables_exist = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM information_schema.tables
            WHERE table_schema = 'ob-poc'
            AND table_name IN ('cbus', 'dictionary', 'attribute_values', 'entities',
                               'dsl_instances', 'parsed_asts', 'ubo_registry', 'document_catalog')
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(sqlx::migrate::MigrateError::Execute)?;

        let count: i64 = tables_exist.get("count");

        if count < 6 {
            warn!("Expected database tables not found. Please run sql/demo_setup.sql");
            return Err(sqlx::migrate::MigrateError::VersionMissing(1));
        }

        info!("Database schema verification complete");
        Ok(())
    }

    /// Close the database connection pool
    pub async fn close(self) {
        info!("Closing database connection pool");
        self.pool.close().await;
    }
}

/// Mask sensitive information in database URL for logging
fn mask_database_url(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        let mut masked = parsed.clone();
        if parsed.password().is_some() {
            let _ = masked.set_password(Some("***"));
        }
        masked.to_string()
    } else {
        // If URL parsing fails, just mask the middle part
        if url.len() > 20 {
            format!("{}***{}", &url[..10], &url[url.len() - 10..])
        } else {
            "***".to_string()
        }
    }
}
