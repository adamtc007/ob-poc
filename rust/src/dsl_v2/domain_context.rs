//! Domain context model for tracking "where we are" in multi-step workflows.
//!
//! This module provides context-aware state tracking for DSL execution,
//! enabling proper domain switching in batch/macro operations.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

// ============================================================================
// Task 1: ActiveDomain Enum
// ============================================================================

/// The primary domain/entity type currently being operated on
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ActiveDomain {
    /// No specific domain active (initial state)
    #[default]
    None,
    /// CBU operations (onboarding a client)
    Cbu,
    /// KYC Case operations (compliance workflow)
    KycCase,
    /// Onboarding Request (provisioning workflow)
    OnboardingRequest,
    /// Entity Workstream (per-entity KYC within a case)
    EntityWorkstream,
    /// UBO Graph operations (ownership verification)
    UboGraph,
    /// Trading Profile (investment mandate)
    TradingProfile,
    /// Contract/ISDA (legal agreements)
    Contract,
}

impl fmt::Display for ActiveDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActiveDomain::None => write!(f, "none"),
            ActiveDomain::Cbu => write!(f, "cbu"),
            ActiveDomain::KycCase => write!(f, "kyc_case"),
            ActiveDomain::OnboardingRequest => write!(f, "onboarding_request"),
            ActiveDomain::EntityWorkstream => write!(f, "entity_workstream"),
            ActiveDomain::UboGraph => write!(f, "ubo_graph"),
            ActiveDomain::TradingProfile => write!(f, "trading_profile"),
            ActiveDomain::Contract => write!(f, "contract"),
        }
    }
}

// ============================================================================
// Task 2: IterationContext
// ============================================================================

/// Context for a single batch iteration
/// Captures what we're iterating over and where to return
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IterationContext {
    /// Index in the batch (0-based)
    pub index: usize,

    /// Human-readable key for this iteration (e.g., "fund:Apex Capital")
    pub iteration_key: String,

    /// The source entity being processed in this iteration
    pub source_entity_id: Uuid,

    /// Entity type of the source (e.g., "fund", "entity")
    pub source_entity_type: String,

    /// Template being executed (if any)
    pub template_id: Option<String>,
}

// ============================================================================
// Task 3: DomainContext
// ============================================================================

/// Domain context that tracks "where we are" in a workflow
///
/// This captures the active domain and associated entity IDs,
/// supporting nested operations via a context stack.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct DomainContext {
    /// Currently active domain
    pub active_domain: ActiveDomain,

    /// Active CBU ID (if in CBU or child domain)
    pub active_cbu_id: Option<Uuid>,

    /// Active CBU name (for display/logging)
    pub active_cbu_name: Option<String>,

    /// Active KYC Case ID (if in KycCase or child domain)
    pub active_case_id: Option<Uuid>,

    /// Active Onboarding Request ID
    pub active_request_id: Option<Uuid>,

    /// Active Entity ID (for workstream/UBO operations)
    pub active_entity_id: Option<Uuid>,

    /// Active Trading Profile ID
    pub active_profile_id: Option<Uuid>,

    /// Active Contract/ISDA ID
    pub active_contract_id: Option<Uuid>,

    /// Batch iteration context (if inside a batch loop)
    pub iteration: Option<IterationContext>,
}

// ============================================================================
// Task 5: Stack Operations
// ============================================================================

impl DomainContext {
    /// Create a new empty context
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

// ============================================================================
// Task 6: Domain Setters
// ============================================================================

impl DomainContext {
    /// Set the active CBU (switches to CBU domain if not already there)
    pub(crate) fn set_active_cbu(&mut self, cbu_id: Uuid, name: Option<String>) {
        self.active_cbu_id = Some(cbu_id);
        self.active_cbu_name = name;
        if self.active_domain == ActiveDomain::None {
            self.active_domain = ActiveDomain::Cbu;
        }
    }
}

// ============================================================================
// Task 7: Iteration Context Methods
// ============================================================================

impl DomainContext {
    /// Enter a batch iteration context
    pub(crate) fn enter_iteration(
        &mut self,
        index: usize,
        key: impl Into<String>,
        source_entity_id: Uuid,
        source_entity_type: impl Into<String>,
        template_id: Option<String>,
    ) {
        self.iteration = Some(IterationContext {
            index,
            iteration_key: key.into(),
            source_entity_id,
            source_entity_type: source_entity_type.into(),
            template_id,
        });

        tracing::debug!(index, source = %source_entity_id, "Entered batch iteration");
    }

    /// Exit batch iteration context
    pub(crate) fn exit_iteration(&mut self) {
        if let Some(iter) = self.iteration.take() {
            tracing::debug!(index = iter.index, "Exited batch iteration");
        }
    }
}

// ============================================================================
// Task 8: From<SessionContext> Conversion
// ============================================================================

#[cfg(feature = "server")]
impl From<&crate::api::session::SessionContext> for DomainContext {
    fn from(ctx: &crate::api::session::SessionContext) -> Self {
        let mut domain_ctx = DomainContext::new();

        // Set active CBU from session
        if let Some(ref cbu) = ctx.active_cbu {
            domain_ctx.set_active_cbu(cbu.id, Some(cbu.display_name.clone()));
        }

        // Set primary keys
        if let Some(cbu_id) = ctx.primary_keys.cbu_id {
            if domain_ctx.active_cbu_id.is_none() {
                domain_ctx.active_cbu_id = Some(cbu_id);
            }
        }
        if let Some(case_id) = ctx.primary_keys.kyc_case_id {
            domain_ctx.active_case_id = Some(case_id);
            domain_ctx.active_domain = ActiveDomain::KycCase;
        }
        if let Some(request_id) = ctx.primary_keys.onboarding_request_id {
            domain_ctx.active_request_id = Some(request_id);
        }

        // Infer domain from stage_focus if set
        if let Some(ref stage) = ctx.stage_focus {
            domain_ctx.active_domain = match stage.as_str() {
                s if s.starts_with("kyc") => ActiveDomain::KycCase,
                s if s.starts_with("ubo") => ActiveDomain::UboGraph,
                s if s.starts_with("trading") => ActiveDomain::TradingProfile,
                s if s.starts_with("contract") => ActiveDomain::Contract,
                _ => domain_ctx.active_domain,
            };
        }

        domain_ctx
    }
}

#[cfg(feature = "server")]
impl From<crate::api::session::SessionContext> for DomainContext {
    fn from(ctx: crate::api::session::SessionContext) -> Self {
        DomainContext::from(&ctx)
    }
}

// ============================================================================
// Task 10: Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_context_is_empty() {
        let ctx = DomainContext::new();
        assert_eq!(ctx.active_domain, ActiveDomain::None);
        assert!(ctx.active_cbu_id.is_none());
    }

    #[test]
    fn test_iteration_enter_exit() {
        let mut ctx = DomainContext::new();

        assert!(ctx.iteration.is_none());

        ctx.enter_iteration(5, "test:item", Uuid::new_v4(), "entity", None);

        assert!(ctx.iteration.is_some());
        let iter = ctx.iteration.as_ref().unwrap();
        assert_eq!(iter.index, 5);
        assert_eq!(iter.iteration_key, "test:item");

        ctx.exit_iteration();
        assert!(ctx.iteration.is_none());
    }

    #[test]
    fn test_active_domain_display() {
        assert_eq!(format!("{}", ActiveDomain::None), "none");
        assert_eq!(format!("{}", ActiveDomain::Cbu), "cbu");
        assert_eq!(format!("{}", ActiveDomain::KycCase), "kyc_case");
        assert_eq!(format!("{}", ActiveDomain::UboGraph), "ubo_graph");
    }

    #[test]
    fn test_set_active_cbu_sets_domain_if_none() {
        let mut ctx = DomainContext::new();
        assert_eq!(ctx.active_domain, ActiveDomain::None);

        ctx.set_active_cbu(Uuid::new_v4(), Some("Test".to_string()));
        assert_eq!(ctx.active_domain, ActiveDomain::Cbu);
    }

    #[test]
    fn test_set_active_cbu_preserves_existing_domain() {
        let mut ctx = DomainContext::new();
        ctx.active_domain = ActiveDomain::KycCase;

        ctx.set_active_cbu(Uuid::new_v4(), Some("Test".to_string()));
        // Should NOT change domain since it's not None
        assert_eq!(ctx.active_domain, ActiveDomain::KycCase);
    }
}
