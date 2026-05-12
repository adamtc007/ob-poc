//! Event infrastructure health monitoring.
//!
//! Provides health checks and statistics for the event system.

use serde::Serialize;

use super::emitter::EmitterStats;

/// Health status of the event infrastructure.
#[derive(Debug, Clone, Serialize)]
pub struct EventHealth {
    /// Overall status: "healthy", "degraded", or "unhealthy"
    pub status: &'static str,

    /// Total events emitted since startup
    pub emitted: u64,

    /// Total events dropped since startup
    pub dropped: u64,

    /// Drop rate as a fraction (0.0 to 1.0)
    pub drop_rate: f64,

    /// Drop rate as a percentage string
    pub drop_rate_pct: String,
}

impl EventHealth {
    /// Create health status from emitter stats.
    pub fn from_stats(stats: EmitterStats) -> Self {
        let drop_rate = stats.drop_rate();
        let status = if drop_rate < 0.001 {
            "healthy" // < 0.1% drop rate
        } else if drop_rate < 0.01 {
            "degraded" // 0.1% - 1% drop rate
        } else {
            "unhealthy" // > 1% drop rate
        };

        Self {
            status,
            emitted: stats.emitted,
            dropped: stats.dropped,
            drop_rate,
            drop_rate_pct: format!("{:.2}%", drop_rate * 100.0),
        }
    }

    /// Check if the system is healthy.
    pub fn is_healthy(&self) -> bool {
        self.status == "healthy"
    }

    /// Check if the system is at least operational.
    pub fn is_operational(&self) -> bool {
        self.status != "unhealthy"
    }
}

impl std::fmt::Display for EventHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Event Infrastructure Status")?;
        writeln!(f, "  Status:    {}", self.status)?;
        writeln!(f, "  Emitted:   {}", self.emitted)?;
        writeln!(f, "  Dropped:   {}", self.dropped)?;
        writeln!(f, "  Drop Rate: {}", self.drop_rate_pct)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_healthy_status() {
        let stats = EmitterStats {
            emitted: 10000,
            dropped: 5,
        };
        let health = EventHealth::from_stats(stats);
        assert_eq!(health.status, "healthy");
        assert!(health.is_healthy());
        assert!(health.is_operational());
    }

    #[test]
    fn test_degraded_status() {
        let stats = EmitterStats {
            emitted: 1000,
            dropped: 5, // 0.5% drop rate
        };
        let health = EventHealth::from_stats(stats);
        assert_eq!(health.status, "degraded");
        assert!(!health.is_healthy());
        assert!(health.is_operational());
    }

    #[test]
    fn test_unhealthy_status() {
        let stats = EmitterStats {
            emitted: 100,
            dropped: 10, // 10% drop rate
        };
        let health = EventHealth::from_stats(stats);
        assert_eq!(health.status, "unhealthy");
        assert!(!health.is_healthy());
        assert!(!health.is_operational());
    }

    #[test]
    fn test_display() {
        let stats = EmitterStats {
            emitted: 1000,
            dropped: 5,
        };
        let health = EventHealth::from_stats(stats);
        let display = format!("{}", health);
        assert!(display.contains("Status:"));
        assert!(display.contains("Emitted:"));
        assert!(display.contains("1000"));
    }
}
