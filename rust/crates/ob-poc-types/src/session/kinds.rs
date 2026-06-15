//! Session-tier shared enums.
//!
//! `WorkspaceKind`, `SubjectKind`, `AgentMode`, and `WorkspaceRegistryEntry`
//! live in the boundary tier because they are referenced by both the
//! execution-tier session machinery (`repl::types_v2`, `session_trace`) and
//! by audit/envelope contracts (`audit_chain`, workbook handoffs). Keeping
//! them outside `repl::types_v2` prevents the envelope tier from having to
//! depend on the execution-tier session module.
//!
//! Phase 3 slice 2c.2b (2026-05-12) extracted these from
//! `rust/src/repl/types_v2.rs`. `repl::types_v2` keeps a `pub use
//! ob_poc_boundary::session::*` re-export so the 46 existing consumer call
//! sites are unaffected.

use serde::{Deserialize, Serialize};

/// A top-level workspace available after scope selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    ProductMaintenance,
    Catalogue,
    Deal,
    Cbu,
    Kyc,
    InstrumentMatrix,
    #[serde(rename = "onboarding_request")]
    OnBoarding,
    #[serde(rename = "semos_maintenance")]
    SemOsMaintenance,
    LifecycleResources,
    Bpmn,
}

impl WorkspaceKind {
    /// Human-readable display label for the workspace.
    pub fn label(&self) -> &'static str {
        match self {
            Self::ProductMaintenance => "Product Maintenance",
            Self::Catalogue => "Catalogue",
            Self::Deal => "Deal",
            Self::Cbu => "CBU",
            Self::Kyc => "KYC",
            Self::InstrumentMatrix => "Instrument Matrix",
            Self::OnBoarding => "OnBoarding",
            Self::SemOsMaintenance => "SemOS Maintenance",
            Self::LifecycleResources => "Lifecycle Resources",
            Self::Bpmn => "BPMN",
        }
    }

    /// Registry metadata for this workspace.
    pub fn registry_entry(&self) -> WorkspaceRegistryEntry {
        match self {
            Self::ProductMaintenance => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec!["product_workspace", "product_service_taxonomy"],
                subject_kinds: vec![
                    SubjectKind::Product,
                    SubjectKind::Service,
                    SubjectKind::Resource,
                    SubjectKind::Attribute,
                ],
                subject_required: false,
                default_constellation_family: "product_service_taxonomy",
                default_constellation_map: "product.service.resource.taxonomy",
                supports_handoff_mode: false,
            },
            Self::Catalogue => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec!["registry_governance"],
                subject_kinds: vec![],
                subject_required: false,
                default_constellation_family: "registry_governance",
                default_constellation_map: "registry.stewardship",
                supports_handoff_mode: false,
            },
            Self::Deal => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec!["deal_workspace", "commercial", "handoff"],
                subject_kinds: vec![SubjectKind::Deal, SubjectKind::Handoff],
                subject_required: true,
                default_constellation_family: "commercial",
                default_constellation_map: "deal.lifecycle",
                supports_handoff_mode: true,
            },
            Self::Cbu => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec![
                    "cbu_workspace",
                    "operating",
                    "maintenance",
                    "lu_ucits",
                    "ie_icav",
                    "uk_auth",
                    "us_40act",
                    "cross_border_hedge",
                    "cross_border_pe",
                ],
                subject_kinds: vec![SubjectKind::Cbu, SubjectKind::Resource],
                subject_required: true,
                default_constellation_family: "operating",
                default_constellation_map: "struct.lux.ucits.sicav",
                supports_handoff_mode: true,
            },
            Self::Kyc => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec![
                    "kyc_workspace",
                    "ownership",
                    "clearance",
                    "delta_review",
                    "screening",
                ],
                subject_kinds: vec![
                    SubjectKind::ClientGroup,
                    SubjectKind::Case,
                    SubjectKind::Cbu,
                ],
                subject_required: false,
                default_constellation_family: "ownership",
                default_constellation_map: "group.ownership",
                supports_handoff_mode: true,
            },
            Self::InstrumentMatrix => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec![
                    "instrument_workspace",
                    "instrument_template",
                    "trading_streetside",
                    "trading_mandate",
                ],
                subject_kinds: vec![
                    SubjectKind::Matrix,
                    SubjectKind::Cbu,
                    SubjectKind::ClientGroup,
                ],
                subject_required: false,
                default_constellation_family: "trading_streetside",
                default_constellation_map: "trading.streetside",
                supports_handoff_mode: true,
            },
            Self::OnBoarding => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec!["onboarding_workspace", "handoff", "activation"],
                subject_kinds: vec![SubjectKind::Handoff, SubjectKind::Cbu],
                subject_required: true,
                default_constellation_family: "handoff",
                default_constellation_map: "deal.lifecycle",
                supports_handoff_mode: true,
            },
            Self::SemOsMaintenance => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec!["semos_workspace", "registry_governance"],
                subject_kinds: vec![],
                subject_required: false,
                default_constellation_family: "registry_governance",
                default_constellation_map: "registry.stewardship",
                supports_handoff_mode: false,
            },
            Self::LifecycleResources => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec!["lifecycle_resources_workspace", "platform"],
                subject_kinds: vec![SubjectKind::Resource],
                subject_required: false,
                default_constellation_family: "platform",
                default_constellation_map: "lifecycle.resources",
                supports_handoff_mode: false,
            },
            Self::Bpmn => WorkspaceRegistryEntry {
                workspace_id: self.clone(),
                display_name: self.label(),
                constellation_families: vec!["bpmn_workspace", "platform"],
                subject_kinds: vec![],
                subject_required: false,
                default_constellation_family: "bpmn_workspace",
                default_constellation_map: "bpmn.workspace",
                supports_handoff_mode: false,
            },
        }
    }

    /// Known workspaces exposed in session-scoped navigation.
    pub fn all() -> Vec<Self> {
        vec![
            Self::ProductMaintenance,
            Self::Catalogue,
            Self::Deal,
            Self::Cbu,
            Self::Kyc,
            Self::InstrumentMatrix,
            Self::OnBoarding,
            Self::SemOsMaintenance,
            Self::LifecycleResources,
            Self::Bpmn,
        ]
    }

    /// Detect a workspace hint from free text.
    pub fn from_hint(message: &str) -> Option<Self> {
        let msg = message.to_lowercase();
        if msg.contains("product")
            || msg.contains("service catalog")
            || msg.contains("resource dictionary")
        {
            return Some(Self::ProductMaintenance);
        }
        if msg.contains("catalogue")
            || msg.contains("catalog")
            || msg.contains("verb declaration")
            || msg.contains("verb proposal")
        {
            return Some(Self::Catalogue);
        }
        if msg.contains("deal") || msg.contains("contract") || msg.contains("rate card") {
            return Some(Self::Deal);
        }
        if msg.contains("cbu") || msg.contains("operating") {
            return Some(Self::Cbu);
        }
        if msg.contains("kyc") || msg.contains("ubo") || msg.contains("clearance") {
            return Some(Self::Kyc);
        }
        if msg.contains("matrix")
            || msg.contains("instruction")
            || msg.contains("trading permission")
        {
            return Some(Self::InstrumentMatrix);
        }
        if msg.contains("onboarding") || msg.contains("handoff") || msg.contains("activation") {
            return Some(Self::OnBoarding);
        }
        if msg.contains("semos")
            || msg.contains("sem os")
            || msg.contains("semantic os")
            || msg.contains("registry governance")
            || msg.contains("stewardship")
        {
            return Some(Self::SemOsMaintenance);
        }
        if msg.contains("lifecycle resource")
            || msg.contains("application instance")
            || msg.contains("capability binding")
        {
            return Some(Self::LifecycleResources);
        }
        if msg.contains("bpmn") || msg.contains("workflow") || msg.contains("orchestration") {
            return Some(Self::Bpmn);
        }
        None
    }
}

/// Sage vs REPL mode at the current top-of-stack context.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    #[default]
    Sage,
    Repl,
}

impl AgentMode {
    /// Whether this mode permits stack operations (push/pop/commit).
    pub fn can_stack_op(&self) -> bool {
        matches!(self, Self::Sage)
    }

    /// Whether this mode permits verb execution.
    pub fn can_execute(&self) -> bool {
        matches!(self, Self::Repl)
    }

    /// Whether this mode permits runbook compilation.
    pub fn can_compile(&self) -> bool {
        matches!(self, Self::Sage)
    }
}

/// Subject kinds supported by the session-scoped navigation layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    ClientGroup,
    Cbu,
    Deal,
    Case,
    Handoff,
    Matrix,
    Product,
    Service,
    Resource,
    Attribute,
}

/// Static registry entry for one workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceRegistryEntry {
    pub workspace_id: WorkspaceKind,
    pub display_name: &'static str,
    pub constellation_families: Vec<&'static str>,
    pub subject_kinds: Vec<SubjectKind>,
    pub subject_required: bool,
    pub default_constellation_family: &'static str,
    pub default_constellation_map: &'static str,
    pub supports_handoff_mode: bool,
}
