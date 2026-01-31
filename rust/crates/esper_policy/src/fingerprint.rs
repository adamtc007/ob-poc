//! Policy fingerprinting for cache invalidation.

use crate::user::UserPolicy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Content-addressed fingerprint of a policy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PolicyFingerprint {
    /// SHA-256 hash of the policy.
    pub hash: String,
    /// Policy version.
    pub version: u64,
}

impl PolicyFingerprint {
    /// Compute fingerprint for a user policy.
    pub fn from_policy(policy: &UserPolicy) -> Self {
        Self::compute(policy)
    }

    /// Compute fingerprint for a user policy.
    pub fn compute(policy: &UserPolicy) -> Self {
        let mut hasher = Sha256::new();

        // Hash key policy fields
        hasher.update(policy.user_id.as_bytes());
        hasher.update(policy.permissions.bits().to_le_bytes());
        hasher.update(policy.expires_at.to_le_bytes());
        hasher.update([policy.is_active as u8]);

        // Hash verb policy
        hasher.update([policy.verb_policy.default_allow as u8]);
        let allow_count = policy.verb_policy.allow_list.len() as u32;
        let deny_count = policy.verb_policy.deny_list.len() as u32;
        hasher.update(allow_count.to_le_bytes());
        hasher.update(deny_count.to_le_bytes());

        // Hash entity policy
        let entity_count = policy.entity_policy.entity_visibility.len() as u32;
        let kind_count = policy.entity_policy.kind_visibility.len() as u32;
        hasher.update(entity_count.to_le_bytes());
        hasher.update(kind_count.to_le_bytes());

        let hash = hex::encode(hasher.finalize());

        Self { hash, version: 1 }
    }

    /// Get a short version of the hash (first 16 chars).
    pub fn short(&self) -> &str {
        &self.hash[..16.min(self.hash.len())]
    }

    /// Convert to u64 for embedding in snapshot envelope.
    pub fn to_u64(&self) -> u64 {
        // Take first 8 bytes of hash
        let bytes: [u8; 8] = self.hash.as_bytes()[..8].try_into().unwrap_or([0; 8]);
        u64::from_le_bytes(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_deterministic() {
        let policy = UserPolicy::new("user-123");

        let fp1 = PolicyFingerprint::compute(&policy);
        let fp2 = PolicyFingerprint::compute(&policy);

        assert_eq!(fp1.hash, fp2.hash);
    }

    #[test]
    fn fingerprint_changes_with_policy() {
        let policy1 = UserPolicy::new("user-123");
        let policy2 = UserPolicy::new("user-456");

        let fp1 = PolicyFingerprint::compute(&policy1);
        let fp2 = PolicyFingerprint::compute(&policy2);

        assert_ne!(fp1.hash, fp2.hash);
    }

    #[test]
    fn fingerprint_changes_with_permissions() {
        let policy1 = UserPolicy::new("user");
        let policy2 = UserPolicy::admin("user");

        let fp1 = PolicyFingerprint::compute(&policy1);
        let fp2 = PolicyFingerprint::compute(&policy2);

        assert_ne!(fp1.hash, fp2.hash);
    }

    #[test]
    fn fingerprint_short() {
        let policy = UserPolicy::new("user");
        let fp = PolicyFingerprint::compute(&policy);

        assert_eq!(fp.short().len(), 16);
    }

    #[test]
    fn fingerprint_to_u64() {
        let policy = UserPolicy::new("user");
        let fp = PolicyFingerprint::compute(&policy);

        let n = fp.to_u64();
        assert!(n > 0);
    }
}
