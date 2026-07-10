//! G3 — Semantic Pack Resolution (V&S §6.3).
//!
//! T2.3 wires the adapter over the constraint gate + SemReg fail-closed
//! (ledger C-015, C-016).

/// `PackResolution` — V&S §6.3 "Output". Variant names mirror the possible
/// outcomes listed there. No active pack means no execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackResolutionOutcome {
    Resolved(ResolvedPack),
    AmbiguousPack,
    MissingPack,
    PackDeniesIntent,
    PackDeniesEntity,
}

/// Success-form proof: exactly one SemOS pack governs this execution.
/// Constructible only from within this module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedPack {
    pack_id: String,
}

impl ResolvedPack {
    // Called by the (future) T2.3 adapter; the only caller today is the
    // cfg(test) bridge below.
    #[allow(dead_code)]
    fn new(pack_id: impl Into<String>) -> Self {
        Self {
            pack_id: pack_id.into(),
        }
    }

    pub fn pack_id(&self) -> &str {
        &self.pack_id
    }
}

#[cfg(test)]
pub(crate) mod tests_support {
    use super::ResolvedPack;

    pub(crate) fn resolved(pack_id: &str) -> ResolvedPack {
        ResolvedPack::new(pack_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_pack_is_constructible_within_its_own_module() {
        let pack = ResolvedPack::new("ob-poc.cbu");
        assert_eq!(pack.pack_id(), "ob-poc.cbu");
    }
}
