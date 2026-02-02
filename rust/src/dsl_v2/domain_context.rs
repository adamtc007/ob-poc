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
pub enum ActiveDomain {
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
pub struct IterationContext {
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
// Task 4: DomainContextFrame (before DomainContext so it can be used)
// ============================================================================

/// A single frame in the domain context stack
/// Captures state before entering a nested domain
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainContextFrame {
    /// Domain that was active before push
    domain: ActiveDomain,

    /// Entity IDs that were active
    cbu_id: Option<Uuid>,
    case_id: Option<Uuid>,
    request_id: Option<Uuid>,
    entity_id: Option<Uuid>,
    profile_id: Option<Uuid>,
    contract_id: Option<Uuid>,

    /// Iteration context (preserved if we push within a batch)
    iteration: Option<IterationContext>,

    /// Why this frame was pushed (for debugging)
    push_reason: String,
}

// ============================================================================
// Task 3: DomainContext
// ============================================================================

/// Domain context that tracks "where we are" in a workflow
///
/// This captures the active domain and associated entity IDs,
/// supporting nested operations via a context stack.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DomainContext {
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

    /// Stack of previous domain contexts (for push/pop)
    /// Inner domains push onto this when entering nested operations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    context_stack: Vec<DomainContextFrame>,
}

// ============================================================================
// Task 5: Stack Operations
// ============================================================================

impl DomainContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create context with a CBU as the active domain
    pub fn for_cbu(cbu_id: Uuid, cbu_name: Option<String>) -> Self {
        Self {
            active_domain: ActiveDomain::Cbu,
            active_cbu_id: Some(cbu_id),
            active_cbu_name: cbu_name,
            ..Default::default()
        }
    }

    /// Push current state and enter a new domain
    ///
    /// Use when entering a nested context (e.g., CBU → KYC Case)
    pub fn push_domain(&mut self, domain: ActiveDomain, reason: impl Into<String>) {
        let frame = DomainContextFrame {
            domain: self.active_domain,
            cbu_id: self.active_cbu_id,
            case_id: self.active_case_id,
            request_id: self.active_request_id,
            entity_id: self.active_entity_id,
            profile_id: self.active_profile_id,
            contract_id: self.active_contract_id,
            iteration: self.iteration.clone(),
            push_reason: reason.into(),
        };
        self.context_stack.push(frame);
        self.active_domain = domain;

        tracing::debug!(
            ?domain,
            stack_depth = self.context_stack.len(),
            "Pushed domain context"
        );
    }

    /// Pop back to previous domain context
    ///
    /// Returns false if stack was empty (nothing to pop)
    pub fn pop_domain(&mut self) -> bool {
        if let Some(frame) = self.context_stack.pop() {
            self.active_domain = frame.domain;
            self.active_cbu_id = frame.cbu_id;
            self.active_case_id = frame.case_id;
            self.active_request_id = frame.request_id;
            self.active_entity_id = frame.entity_id;
            self.active_profile_id = frame.profile_id;
            self.active_contract_id = frame.contract_id;
            self.iteration = frame.iteration;

            tracing::debug!(
                domain = ?self.active_domain,
                stack_depth = self.context_stack.len(),
                "Popped domain context"
            );
            true
        } else {
            tracing::warn!("Attempted to pop empty domain context stack");
            false
        }
    }

    /// Execute a closure with a pushed domain, automatically popping after
    pub fn with_domain<F, R>(&mut self, domain: ActiveDomain, reason: &str, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        self.push_domain(domain, reason);
        let result = f(self);
        self.pop_domain();
        result
    }

    /// Get current stack depth (0 = root context)
    pub fn stack_depth(&self) -> usize {
        self.context_stack.len()
    }

    /// Check if we're inside a batch iteration
    pub fn in_batch_iteration(&self) -> bool {
        self.iteration.is_some()
    }

    /// Get iteration info if in batch
    pub fn iteration_info(&self) -> Option<(usize, &str)> {
        self.iteration
            .as_ref()
            .map(|i| (i.index, i.iteration_key.as_str()))
    }
}

// ============================================================================
// Task 6: Domain Setters
// ============================================================================

impl DomainContext {
    /// Set the active CBU (switches to CBU domain if not already there)
    pub fn set_active_cbu(&mut self, cbu_id: Uuid, name: Option<String>) {
        self.active_cbu_id = Some(cbu_id);
        self.active_cbu_name = name;
        if self.active_domain == ActiveDomain::None {
            self.active_domain = ActiveDomain::Cbu;
        }
    }

    /// Set the active KYC case (pushes KycCase domain)
    pub fn set_active_case(&mut self, case_id: Uuid) {
        if self.active_domain != ActiveDomain::KycCase {
            self.push_domain(ActiveDomain::KycCase, "kyc-case activated");
        }
        self.active_case_id = Some(case_id);
    }

    /// Set the active onboarding request
    pub fn set_active_request(&mut self, request_id: Uuid) {
        if self.active_domain != ActiveDomain::OnboardingRequest {
            self.push_domain(
                ActiveDomain::OnboardingRequest,
                "onboarding-request activated",
            );
        }
        self.active_request_id = Some(request_id);
    }

    /// Set the active entity (for workstream/UBO operations)
    pub fn set_active_entity(&mut self, entity_id: Uuid) {
        self.active_entity_id = Some(entity_id);
    }

    /// Set the active trading profile
    pub fn set_active_profile(&mut self, profile_id: Uuid) {
        if self.active_domain != ActiveDomain::TradingProfile {
            self.push_domain(ActiveDomain::TradingProfile, "trading-profile activated");
        }
        self.active_profile_id = Some(profile_id);
    }

    /// Clear the active entity (when leaving entity-specific operations)
    pub fn clear_active_entity(&mut self) {
        self.active_entity_id = None;
    }
}

// ============================================================================
// Task 7: Iteration Context Methods
// ============================================================================

impl DomainContext {
    /// Enter a batch iteration context
    pub fn enter_iteration(
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
    pub fn exit_iteration(&mut self) {
        if let Some(iter) = self.iteration.take() {
            tracing::debug!(index = iter.index, "Exited batch iteration");
        }
    }

    /// Create a child context for a batch iteration
    ///
    /// Inherits CBU/Case context from parent, sets iteration info
    pub fn child_for_iteration(
        &self,
        index: usize,
        key: impl Into<String>,
        source_entity_id: Uuid,
        source_entity_type: impl Into<String>,
        template_id: Option<String>,
    ) -> Self {
        Self {
            active_domain: self.active_domain,
            active_cbu_id: self.active_cbu_id,
            active_cbu_name: self.active_cbu_name.clone(),
            active_case_id: self.active_case_id,
            active_request_id: self.active_request_id,
            active_entity_id: None,  // Fresh for each iteration
            active_profile_id: None, // Fresh for each iteration
            active_contract_id: None,
            iteration: Some(IterationContext {
                index,
                iteration_key: key.into(),
                source_entity_id,
                source_entity_type: source_entity_type.into(),
                template_id,
            }),
            context_stack: Vec::new(), // Fresh stack for iteration
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
        assert_eq!(ctx.stack_depth(), 0);
    }

    #[test]
    fn test_for_cbu() {
        let cbu_id = Uuid::now_v7();
        let ctx = DomainContext::for_cbu(cbu_id, Some("Test Fund".to_string()));

        assert_eq!(ctx.active_domain, ActiveDomain::Cbu);
        assert_eq!(ctx.active_cbu_id, Some(cbu_id));
        assert_eq!(ctx.active_cbu_name, Some("Test Fund".to_string()));
    }

    #[test]
    fn test_push_pop_domain() {
        let cbu_id = Uuid::now_v7();
        let mut ctx = DomainContext::for_cbu(cbu_id, None);

        // Push KYC case
        ctx.push_domain(ActiveDomain::KycCase, "test");
        assert_eq!(ctx.active_domain, ActiveDomain::KycCase);
        assert_eq!(ctx.stack_depth(), 1);
        assert_eq!(ctx.active_cbu_id, Some(cbu_id)); // CBU preserved

        // Push entity workstream
        ctx.push_domain(ActiveDomain::EntityWorkstream, "test");
        assert_eq!(ctx.active_domain, ActiveDomain::EntityWorkstream);
        assert_eq!(ctx.stack_depth(), 2);

        // Pop back
        assert!(ctx.pop_domain());
        assert_eq!(ctx.active_domain, ActiveDomain::KycCase);
        assert_eq!(ctx.stack_depth(), 1);

        assert!(ctx.pop_domain());
        assert_eq!(ctx.active_domain, ActiveDomain::Cbu);
        assert_eq!(ctx.stack_depth(), 0);

        // Pop empty returns false
        assert!(!ctx.pop_domain());
    }

    #[test]
    fn test_with_domain_auto_pop() {
        let mut ctx = DomainContext::for_cbu(Uuid::now_v7(), None);

        let result = ctx.with_domain(ActiveDomain::KycCase, "scoped", |inner| {
            assert_eq!(inner.active_domain, ActiveDomain::KycCase);
            assert_eq!(inner.stack_depth(), 1);
            42
        });

        assert_eq!(result, 42);
        assert_eq!(ctx.active_domain, ActiveDomain::Cbu);
        assert_eq!(ctx.stack_depth(), 0);
    }

    #[test]
    fn test_child_for_iteration() {
        let cbu_id = Uuid::now_v7();
        let mut parent = DomainContext::for_cbu(cbu_id, Some("Parent Fund".to_string()));
        parent.set_active_case(Uuid::now_v7());

        let source_entity = Uuid::now_v7();
        let child = parent.child_for_iteration(
            0,
            "fund:Child Fund",
            source_entity,
            "fund",
            Some("onboard-fund".to_string()),
        );

        // Child inherits parent's CBU and case
        assert_eq!(child.active_cbu_id, parent.active_cbu_id);
        assert_eq!(child.active_case_id, parent.active_case_id);

        // Child has fresh entity/profile
        assert!(child.active_entity_id.is_none());
        assert!(child.active_profile_id.is_none());

        // Child has iteration context
        assert!(child.in_batch_iteration());
        let (idx, key) = child.iteration_info().unwrap();
        assert_eq!(idx, 0);
        assert_eq!(key, "fund:Child Fund");

        // Child has fresh stack
        assert_eq!(child.stack_depth(), 0);
    }

    #[test]
    fn test_iteration_enter_exit() {
        let mut ctx = DomainContext::new();

        assert!(!ctx.in_batch_iteration());

        ctx.enter_iteration(5, "test:item", Uuid::now_v7(), "entity", None);

        assert!(ctx.in_batch_iteration());
        let (idx, key) = ctx.iteration_info().unwrap();
        assert_eq!(idx, 5);
        assert_eq!(key, "test:item");

        ctx.exit_iteration();
        assert!(!ctx.in_batch_iteration());
    }

    #[test]
    fn test_set_active_case_pushes_domain() {
        let mut ctx = DomainContext::for_cbu(Uuid::now_v7(), None);
        let case_id = Uuid::now_v7();

        ctx.set_active_case(case_id);

        assert_eq!(ctx.active_domain, ActiveDomain::KycCase);
        assert_eq!(ctx.active_case_id, Some(case_id));
        assert_eq!(ctx.stack_depth(), 1); // Pushed!

        // Pop should restore CBU domain
        ctx.pop_domain();
        assert_eq!(ctx.active_domain, ActiveDomain::Cbu);
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

        ctx.set_active_cbu(Uuid::now_v7(), Some("Test".to_string()));
        assert_eq!(ctx.active_domain, ActiveDomain::Cbu);
    }

    #[test]
    fn test_set_active_cbu_preserves_existing_domain() {
        let mut ctx = DomainContext::new();
        ctx.push_domain(ActiveDomain::KycCase, "test");

        ctx.set_active_cbu(Uuid::now_v7(), Some("Test".to_string()));
        // Should NOT change domain since it's not None
        assert_eq!(ctx.active_domain, ActiveDomain::KycCase);
    }
}
