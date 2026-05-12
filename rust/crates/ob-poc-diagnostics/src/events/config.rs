//! Event infrastructure configuration.
//!
//! Configuration for the event emission system, including buffer sizes,
//! store backend selection, and feature flags.

use std::path::PathBuf;

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Configuration for the event infrastructure.
#[derive(Debug, Clone)]
pub struct EventConfig {
    /// Whether event emission is enabled.
    pub enabled: bool,

    /// Size of the event buffer (channel capacity).
    ///
    /// - Too small: drops events under load
    /// - Too large: memory pressure
    /// - Default: 4096 (~1MB for typical events)
    pub buffer_size: usize,

    /// Event store configuration.
    pub store: StoreConfig,

    /// Drain task configuration.
    pub drain: DrainConfig,
}

impl Default for EventConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            buffer_size: 4096,
            store: StoreConfig::default(),
            drain: DrainConfig::default(),
        }
    }
}

impl EventConfig {
    /// Create a disabled configuration.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Create a configuration with a file store.
    pub fn with_file_store(path: PathBuf) -> Self {
        Self {
            store: StoreConfig::File { path },
            ..Default::default()
        }
    }

    /// Create a configuration with a database store.
    #[cfg(feature = "database")]
    pub fn with_db_store(pool: PgPool) -> Self {
        Self {
            store: StoreConfig::Database { pool },
            ..Default::default()
        }
    }

    /// Set the buffer size.
    pub fn buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Enable or disable events.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// Event store configuration.
#[derive(Debug, Clone)]
pub enum StoreConfig {
    /// File-based store (JSONL format).
    File { path: PathBuf },

    /// Database-backed store.
    #[cfg(feature = "database")]
    Database { pool: PgPool },
}

impl Default for StoreConfig {
    fn default() -> Self {
        // Default to file store in data directory
        StoreConfig::File {
            path: PathBuf::from("data/events.jsonl"),
        }
    }
}

/// Drain task configuration.
#[derive(Debug, Clone)]
pub struct DrainConfig {
    /// Maximum events to drain per batch.
    pub batch_size: usize,

    /// Interval between drain cycles (milliseconds).
    pub flush_interval_ms: u64,
}

impl Default for DrainConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            flush_interval_ms: 1000, // 1 second
        }
    }
}

impl DrainConfig {
    /// Get flush interval as Duration.
    pub fn flush_interval(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.flush_interval_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EventConfig::default();
        assert!(config.enabled);
        assert_eq!(config.buffer_size, 4096);
        assert_eq!(config.drain.batch_size, 100);
        assert_eq!(config.drain.flush_interval_ms, 1000);
    }

    #[test]
    fn test_disabled_config() {
        let config = EventConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_file_store_config() {
        let config = EventConfig::with_file_store(PathBuf::from("/tmp/events.jsonl"));
        match config.store {
            StoreConfig::File { path } => {
                assert_eq!(path, PathBuf::from("/tmp/events.jsonl"));
            }
            #[cfg(feature = "database")]
            _ => panic!("Expected file store"),
        }
    }

    #[test]
    fn test_builder_pattern() {
        let config = EventConfig::default().buffer_size(8192).enabled(false);

        assert!(!config.enabled);
        assert_eq!(config.buffer_size, 8192);
    }
}
