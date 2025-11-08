//! Database connection and management module
//!
//! This module provides database connection management, connection pooling,
//! and configuration for the DSL architecture.

use sqlx::Row;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;
use tracing::{info, warn};

pub mod dsl_domain_repository;

// Re-export repository and trait for convenience
pub use dsl_domain_repository::{DslDomainRepository, DslDomainRepositoryTrait};

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
                .unwrap_or_else(|_| "postgresql://localhost/dsl_ob_poc".to_string()),
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

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new DSL domain repository using this database connection
    pub fn dsl_repository(&self) -> DslDomainRepository {
        DslDomainRepository::new(self.pool.clone())
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

        // Note: In a real implementation, you might want to use sqlx-migrate
        // or implement a custom migration runner here
        // For now, we'll just verify the schema exists

        let tables_exist = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM information_schema.tables
            WHERE table_schema = 'dsl-ob-poc'
            AND table_name IN ('dsl_domains', 'dsl_versions', 'parsed_asts', 'dsl_execution_log')
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| sqlx::migrate::MigrateError::Execute(e))?;

        let count: i64 = tables_exist.get("count");

        if count < 4 {
            warn!("Expected database tables not found. Please run the migration script: sql/migrations/001_dsl_domain_architecture.sql");
            return Err(sqlx::migrate::MigrateError::VersionMissing(1));
        }

        info!("Database schema verification complete");
        Ok(())
    }

    /// Get database connection statistics
    pub fn connection_stats(&self) -> ConnectionStats {
        ConnectionStats {
            size: self.pool.size(),
            num_idle: self.pool.num_idle() as u32,
        }
    }

    /// Close the database connection pool
    pub async fn close(self) {
        info!("Closing database connection pool");
        self.pool.close().await;
    }
}

/// Database connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub size: u32,
    pub num_idle: u32,
}

impl std::fmt::Display for ConnectionStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pool size: {}, Idle: {}", self.size, self.num_idle)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.connection_timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_mask_database_url() {
        let url = "postgresql://user:password@localhost:5432/database";
        let masked = mask_database_url(url);
        assert!(masked.contains("***"));
        assert!(!masked.contains("password"));
    }

    #[test]
    fn test_mask_invalid_url() {
        let url = "not-a-valid-url-but-longer-than-twenty-chars";
        let masked = mask_database_url(url);
        assert!(masked.contains("***"));
    }
}
