//! Semantic Registry — immutable snapshot-based registry for the Semantic OS.
//!
//! # Architecture
//!
//! All registry objects (attribute definitions, entity types, verb contracts,
//! taxonomy nodes, policy rules, etc.) share a single `sem_reg.snapshots` table
//! with a JSONB `definition` column. Typed Rust structs provide compile-time
//! safety over the JSONB bodies.
//!
//! ## Key Invariants
//!
//! 1. **No in-place updates** — every change produces a new immutable snapshot
//! 2. **Proof Rule** — only governed-tier objects may have `TrustClass::Proof`
//! 3. **Security labels on both tiers** — classification, PII, jurisdictions
//! 4. **Operational auto-approved** — no governed approval gates on operational iteration
//! 5. **Point-in-time resolution** — `resolve_active(type, id)` and `resolve_at(type, id, as_of)`
//!
//! ## Module Structure
//!
//! - `types` — Core enums, `SecurityLabel`, `SnapshotMeta`, `SnapshotRow`
//! - `store` — `SnapshotStore` database operations (INSERT-only + supersede)
//! - `gates` — Publish gate pure functions (proof rule, security, approval, version)
//! - `attribute_def` — Attribute definition body type
//! - `entity_type_def` — Entity type definition body type
//! - `verb_contract` — Verb contract body type
//! - `registry` — `RegistryService` typed publish/resolve for each object type

// Phase 0: Core infrastructure
pub mod gates;
pub mod store;
pub mod types;

// Phase 1: Registry body types
pub mod attribute_def;
pub mod entity_type_def;
pub mod registry;
pub mod verb_contract;

// Phase 1.4: Verb-first onboarding scanner
pub mod scanner;

// Phase 2: Taxonomy, membership, view definitions
pub mod membership;
pub mod taxonomy_def;
pub mod view_def;

// Phase 3: Policy, evidence, observations, ABAC
pub mod abac;
pub mod document_type_def;
pub mod evidence;
pub mod observation_def;
pub mod policy_rule;

// Phase 4: Security label inheritance
pub mod security;

// Phase 5: Derived & composite attributes
pub mod derivation;
pub mod derivation_spec;

// Phase 6: Publish gates framework
pub mod gates_governance;
pub mod gates_technical;

// Phase 7: Context resolution API
pub mod context_resolution;

// Phase 8: Agent control plane + MCP tools
pub mod agent;

// Phase 9: Lineage, embeddings, coverage metrics
pub mod projections;

// Re-export core types at module boundary
pub use gates::{check_evidence_proof_rule, evaluate_publish_gates, GateResult, PublishGateResult};
pub use store::SnapshotStore;
pub use types::{
    ChangeType, Classification, GovernanceTier, HandlingControl, ObjectType, SecurityLabel,
    SnapshotMeta, SnapshotRow, SnapshotStatus, TrustClass,
};

// Re-export body types
pub use attribute_def::AttributeDefBody;
pub use entity_type_def::EntityTypeDefBody;
pub use membership::{MembershipKind, MembershipRuleBody};
pub use registry::RegistryService;
pub use taxonomy_def::{TaxonomyDefBody, TaxonomyNodeBody};
pub use verb_contract::VerbContractBody;
pub use view_def::ViewDefBody;

// Re-export Phase 3 types
pub use abac::{evaluate_abac, AccessDecision, AccessPurpose, ActorContext};
pub use derivation_spec::DerivationSpecBody;
pub use document_type_def::DocumentTypeDefBody;
pub use evidence::EvidenceRequirementBody;
pub use observation_def::ObservationDefBody;
pub use policy_rule::PolicyRuleBody;
pub use security::{compute_inherited_label, validate_verb_security_compatibility};

// Re-export Phase 5-6 types
pub use derivation::{DerivationFunctionRegistry, DerivationResult};
pub use gates::{ExtendedPublishGateResult, GateFailure, GateMode, GateSeverity};
pub use registry::PublishOutcome;

// Re-export Phase 7 types
pub use context_resolution::{
    resolve_context, ContextResolutionRequest, ContextResolutionResponse, EvidenceMode, SubjectRef,
};

// Re-export Phase 8 types
pub use agent::{
    all_tool_specs, dispatch_tool, AgentPlan, AgentPlanStatus, DecisionRecord, DecisionStore,
    PlanStep, PlanStepStatus, PlanStore, SemRegToolContext, SemRegToolResult,
};

// Re-export Phase 9 types
pub use projections::{
    CoverageReport, DerivationEdge, EmbeddingRecord, EmbeddingStore, LineageDirection,
    LineageStore, MetricsStore, RunRecord, SemanticText, TierDistribution,
};
