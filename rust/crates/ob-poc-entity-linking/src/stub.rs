//! Stub service for graceful degradation when no snapshot available.
//!
//! Implements the same trait as the real service but returns empty results.
//! This allows the system to function (with reduced capability) when:
//! - Entity snapshot hasn't been compiled yet
//! - Snapshot file is missing or corrupted
//! - Running in a minimal test environment

use super::resolver::{EntityLinkingService, EntityResolution};

/// Stub implementation that returns empty results.
/// Used for graceful degradation when no snapshot is available.
pub struct StubEntityLinkingService;

impl StubEntityLinkingService {
    /// Create a new stub service
    pub fn new() -> Self {
        Self
    }
}

impl Default for StubEntityLinkingService {
    fn default() -> Self {
        Self::new()
    }
}

impl EntityLinkingService for StubEntityLinkingService {
    fn snapshot_hash(&self) -> &str {
        "stub-no-snapshot"
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
    fn test_stub_returns_empty() {
        let stub = StubEntityLinkingService::new();

        assert_eq!(stub.snapshot_hash(), "stub-no-snapshot");
        assert_eq!(stub.snapshot_version(), 0);
        assert_eq!(stub.entity_count(), 0);
        assert!(stub.resolve_mentions("test", None, None, 10).is_empty());
    }

    #[test]
    fn test_stub_default() {
        let stub: StubEntityLinkingService = Default::default();
        assert_eq!(stub.entity_count(), 0);
    }
}
