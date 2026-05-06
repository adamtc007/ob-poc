use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
    };
}

string_id!(
    /// Immutable ULID for an authored eval case.
    CaseId
);
string_id!(
    /// SemOS configuration version pinned by a case or candidate.
    ConfigVersion
);
string_id!(
    /// Deterministic SemOS state snapshot identifier.
    StateSnapshotId
);
string_id!(
    /// Domain Pack identifier.
    PackId
);
string_id!(
    /// Domain Pack version.
    PackVersion
);
string_id!(
    /// Seed bundle identifier.
    SeedBundleId
);
string_id!(
    /// Phrasing bundle identifier.
    PhrasingBundleId
);
