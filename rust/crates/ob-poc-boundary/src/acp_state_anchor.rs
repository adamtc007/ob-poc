//! ACP state-anchor provider routing — pure boundary contracts.
//!
//! Hosts the read-only provider registry, supported-transition projection,
//! and the report/outcome shapes consumed by the Repl-coupled provider
//! drivers that still live in `ob_poc::acp_state_anchor`. The execution-tier
//! drivers depend on this crate; this crate must not depend on them.

use serde_json::{json, Value};
use uuid::Uuid;

use crate::acp_facade::load_ob_poc_kyc_domain_pack;

pub const KYC_UPDATE_STATUS_TASK: &str = "kyc-case.update-status";
pub const DEAL_UPDATE_STATUS_TASK: &str = "deal.update-status";
pub const DEAL_PACK_ID: &str = "deal.lifecycle";
pub const DEAL_PACK_VERSION: &str = "1.0";
pub const DEAL_STATE_MACHINE: &str = "deal_commercial_lifecycle";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpStateAnchorProviderDescriptor {
    pub provider_id: &'static str,
    pub task: &'static str,
    pub subject_kind: &'static str,
    pub live_state_source: &'static str,
    pub language_pack_boundary: &'static str,
    pub dry_run_only: bool,
    pub mutation_authority: bool,
    pub supported_verbs: &'static [&'static str],
}

pub fn provider_registry() -> &'static [AcpStateAnchorProviderDescriptor] {
    static PROVIDERS: &[AcpStateAnchorProviderDescriptor] = &[
        AcpStateAnchorProviderDescriptor {
            provider_id: "kyc.update_status.live_case_state",
            task: KYC_UPDATE_STATUS_TASK,
            subject_kind: "kyc_case",
            live_state_source: "postgres.ob-poc.cases.status",
            language_pack_boundary: "kyc_update_status_language_pack_v1",
            dry_run_only: true,
            mutation_authority: false,
            supported_verbs: &[KYC_UPDATE_STATUS_TASK],
        },
        AcpStateAnchorProviderDescriptor {
            provider_id: "deal.update_status.live_deal_state",
            task: DEAL_UPDATE_STATUS_TASK,
            subject_kind: "deal",
            live_state_source: "postgres.ob-poc.deals.deal_status",
            language_pack_boundary: "update_status_language_pack_v1",
            dry_run_only: true,
            mutation_authority: false,
            supported_verbs: &[DEAL_UPDATE_STATUS_TASK],
        },
    ];
    PROVIDERS
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpStateAnchorProviderSupportedTransition {
    pub provider_id: String,
    pub task: String,
    pub transition_ref: String,
    pub from_state: String,
    pub to_state: String,
}

pub fn provider_supported_transition_registry() -> Vec<AcpStateAnchorProviderSupportedTransition> {
    let mut transitions = Vec::new();

    if let Some(provider) = provider_registry()
        .iter()
        .find(|provider| provider.task == KYC_UPDATE_STATUS_TASK)
    {
        if let Ok(manifest) = load_ob_poc_kyc_domain_pack() {
            transitions.extend(
                manifest
                    .allowed_transitions
                    .iter()
                    .filter(|transition| transition.verb == KYC_UPDATE_STATUS_TASK)
                    .map(|transition| AcpStateAnchorProviderSupportedTransition {
                        provider_id: provider.provider_id.to_string(),
                        task: provider.task.to_string(),
                        transition_ref: transition.transition_ref.clone(),
                        from_state: transition.from_state.clone(),
                        to_state: transition.to_state.clone(),
                    }),
            );
        }
    }

    if let Some(provider) = provider_registry()
        .iter()
        .find(|provider| provider.task == DEAL_UPDATE_STATUS_TASK)
    {
        transitions.extend(DEAL_UPDATE_STATUS_TRANSITIONS.iter().map(|transition| {
            AcpStateAnchorProviderSupportedTransition {
                provider_id: provider.provider_id.to_string(),
                task: provider.task.to_string(),
                transition_ref: transition.transition_ref.to_string(),
                from_state: transition.from_state.to_string(),
                to_state: transition.to_state.to_string(),
            }
        }));
    }

    transitions
}

pub fn supported_tasks() -> Vec<&'static str> {
    provider_registry()
        .iter()
        .map(|provider| provider.task)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpPromptStateAnchorProvider {
    KycUpdateStatus,
    DealUpdateStatus,
}

impl AcpPromptStateAnchorProvider {
    pub fn id(self) -> &'static str {
        match self {
            Self::KycUpdateStatus => "kyc.update_status.live_case_state",
            Self::DealUpdateStatus => "deal.update_status.live_deal_state",
        }
    }

    pub fn task(self) -> &'static str {
        match self {
            Self::KycUpdateStatus => KYC_UPDATE_STATUS_TASK,
            Self::DealUpdateStatus => DEAL_UPDATE_STATUS_TASK,
        }
    }

    pub fn language_pack_boundary(self) -> &'static str {
        provider_registry()
            .iter()
            .find(|provider| provider.task == self.task())
            .map(|provider| provider.language_pack_boundary)
            .unwrap_or("unknown_language_pack_boundary")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpPromptStateAnchorProviderReport {
    pub provider_id: Option<&'static str>,
    pub task: Option<&'static str>,
    pub language_pack_boundary: Option<&'static str>,
    pub status: &'static str,
    pub state_anchor_source: Option<&'static str>,
    pub subject_id: Option<Uuid>,
    pub supported_tasks: Vec<&'static str>,
    pub needed: Vec<&'static str>,
}

impl AcpPromptStateAnchorProviderReport {
    pub fn not_applicable() -> Self {
        Self {
            provider_id: None,
            task: None,
            language_pack_boundary: None,
            status: "not_applicable",
            state_anchor_source: None,
            subject_id: None,
            supported_tasks: supported_tasks(),
            needed: Vec::new(),
        }
    }

    pub fn for_provider(provider: AcpPromptStateAnchorProvider, status: &'static str) -> Self {
        Self {
            provider_id: Some(provider.id()),
            task: Some(provider.task()),
            language_pack_boundary: Some(provider.language_pack_boundary()),
            status,
            state_anchor_source: None,
            subject_id: None,
            supported_tasks: supported_tasks(),
            needed: Vec::new(),
        }
    }

    pub fn unsupported(needed: Vec<&'static str>) -> Self {
        Self {
            provider_id: None,
            task: None,
            language_pack_boundary: None,
            status: "provider_unavailable",
            state_anchor_source: None,
            subject_id: None,
            supported_tasks: supported_tasks(),
            needed,
        }
    }

    pub fn metrics(&self, result: Option<&Value>) -> Value {
        let language_pack_generated = result
            .and_then(|value| value.get("language_pack"))
            .is_some();
        let dry_run_valid = result
            .and_then(|value| {
                value
                    .get("metrics")
                    .and_then(|metrics| metrics.get("dry_run_valid"))
                    .and_then(|valid| valid.as_bool())
                    .or_else(|| {
                        value
                            .get("status")
                            .and_then(|status| status.as_str())
                            .map(|status| status == "dry_run_validated")
                    })
            })
            .unwrap_or(false);
        let structured_outcome = result
            .and_then(|value| value.get("status"))
            .and_then(|status| status.as_str())
            .map(|status| {
                matches!(
                    status,
                    "dry_run_validated"
                        | "structured_refusal"
                        | "pending_question"
                        | "dag_semantic_proposal"
                )
            })
            .unwrap_or(false);

        json!({
            "provider_selected": self.provider_id.is_some(),
            "provider_id": self.provider_id,
            "task": self.task,
            "language_pack_boundary": self.language_pack_boundary,
            "status": self.status,
            "state_anchor_source": self.state_anchor_source,
            "subject_id": self.subject_id,
            "supported_tasks": self.supported_tasks,
            "needed": self.needed,
            "language_pack_generated": language_pack_generated,
            "dry_run_valid": dry_run_valid,
            "structured_outcome": structured_outcome,
            "no_mutation_authority": true
        })
    }
}

pub enum AcpPromptStateAnchorProviderOutcome {
    Continue {
        outgoing: Vec<crate::acp_protocol::JsonRpcOutgoing>,
        report: AcpPromptStateAnchorProviderReport,
    },
    Complete {
        outgoing: Vec<crate::acp_protocol::JsonRpcOutgoing>,
        report: AcpPromptStateAnchorProviderReport,
    },
}

impl AcpPromptStateAnchorProviderOutcome {
    pub fn continue_without_provider() -> Self {
        Self::Continue {
            outgoing: Vec::new(),
            report: AcpPromptStateAnchorProviderReport::not_applicable(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DealTransitionSpec {
    pub transition_ref: &'static str,
    pub from_state: &'static str,
    pub to_state: &'static str,
}

pub const DEAL_UPDATE_STATUS_TRANSITIONS: &[DealTransitionSpec] = &[
    DealTransitionSpec {
        transition_ref: "deal.prospect-to-qualifying",
        from_state: "PROSPECT",
        to_state: "QUALIFYING",
    },
    DealTransitionSpec {
        transition_ref: "deal.qualifying-to-negotiating",
        from_state: "QUALIFYING",
        to_state: "NEGOTIATING",
    },
    DealTransitionSpec {
        transition_ref: "deal.in-clearance-to-contracted",
        from_state: "IN_CLEARANCE",
        to_state: "CONTRACTED",
    },
];
