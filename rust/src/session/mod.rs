//! Session module - session state, scope, and navigation support types.
//!
//! `UnifiedSession` (in `unified.rs`) is the canonical interactive session
//! model. The other submodules here provide supporting types (scope
//! definitions, verb tiering, view state, etc.) used by the API layer.

pub mod canonical_hash;
pub mod constraint_cascade;
pub mod research_context;
pub mod scope;
pub mod scope_path;
pub mod struct_mass;
pub mod unified;
pub mod verb_contract;
pub mod verb_hash_lookup;
pub mod verb_sync;
pub mod verb_tiering_linter;
pub mod view_state;

pub use canonical_hash::{sha256};
pub(crate) use canonical_hash::canonical_json_hash;
pub(crate) use research_context::{ResearchContext, ResearchState};
pub(crate) use scope::{ExpandableNode, LoadStatus, ScopeSummary, SessionScope};
pub(crate) use scope_path::ScopePath;
pub(crate) use struct_mass::{MassBreakdown, MassViewMode, StructMass};
pub use unified::{EntryStatus, UnifiedSession, ViewState as UnifiedViewState};
pub(crate) use unified::{
    BoundEntity, CaseRef, CbuSnapshot, ChatMessage, ClientRef, CorrectionSubSession, DagState,
    DiscriminatorField, EntityMatch, EntityMatchInfo, EntityScope,
    EnumValue, FieldType,
    MessageRole, Persona, ReplState, ResearchSubSession, ResolutionState,
    ResolutionSubSession, ResolvedRef, ReviewStatus, ReviewSubSession, RunSheet, RunSheetEntry,
    SearchKeyField, SearchScope, SessionEvent, SessionListItem, SessionState,
    StructureRef, StructureType, SubSessionType, TargetUniverse, UniverseDefinition,
    UnresolvedRef, UnresolvedRefInfo, ValidationError as UnifiedValidationError, ZoomLevel,
};
pub use verb_contract::{codes as diagnostic_codes, VerbDiagnostics};
pub(crate) use verb_contract::{VerbDiagnostic};
pub use verb_sync::{VerbSyncService};
pub use verb_tiering_linter::{lint_all_verbs_with_config, LintConfig, LintTier};
pub use view_state::{ViewState};
pub(crate) use view_state::{BatchOperation, Refinement};
