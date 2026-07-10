//! Null-object service for graceful degradation when no snapshot available.
//!
//! Implements the same trait as the real service but returns empty results.
//! This allows the system to function (with reduced capability) when:
//! - Entity snapshot hasn't been compiled yet
//! - Snapshot file is missing or corrupted
//! - Running in a minimal test environment

use super::resolver::{EntityLinkingService, EntityResolution};

/// Null-object implementation that returns empty results.
/// Used for graceful degradation when no snapshot is available.
pub struct NullEntityLinkingService;

impl NullEntityLinkingService {
    /// Create a new null-object service
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullEntityLinkingService {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityLinkingService for NullEntityLinkingService {
    fn snapshot_hash(&self) -> &str {
        "no-snapshot"
    }

    fn snapshot_version(&self) -> u32 {
        0
    }

    fn entity_count(&self) -> usize {
        0
    }

    fn resolve_mentions(
        &self,
        _utterance: &str,
        _expected_kinds: Option<&[String]>,
        _context_concepts: Option<&[String]>,
        _limit: usize,
    ) -> Vec<EntityResolution> {
        vec![] // No resolution without snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_service_returns_empty() {
        let svc = NullEntityLinkingService::new();

        assert_eq!(svc.snapshot_hash(), "no-snapshot");
        assert_eq!(svc.snapshot_version(), 0);
        assert_eq!(svc.entity_count(), 0);
        assert!(svc.resolve_mentions("test", None, None, 10).is_empty());
    }

    #[test]
    fn test_null_service_default() {
        let svc: NullEntityLinkingService = Default::default();
        assert_eq!(svc.entity_count(), 0);
    }
}
