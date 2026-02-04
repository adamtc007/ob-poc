//! DecisionPacket Builder
//!
//! Provides a builder pattern for constructing DecisionPackets
//! from existing clarification types (VerbDisambiguationRequest,
//! IntentTierRequest, etc.).

use ob_poc_types::{
    ClarificationPayload, DecisionKind, DecisionPacket, DecisionTrace, EffectMode, EffectsPreview,
    GroupClarificationPayload, GroupOption, PlanPreview, ProposalPayload, RefusePayload,
    ScopeOption, ScopePayload, SessionStateView, UserChoice, VerbOption, VerbPayload,
};
use thiserror::Error;
use uuid::Uuid;

use super::confirm::generate_confirm_token;

/// Errors that can occur during packet building
#[derive(Debug, Error)]
pub enum PacketBuildError {
    #[error("Missing required field: {0}")]
    MissingField(&'static str),

    #[error("Invalid payload for kind {kind:?}: {reason}")]
    InvalidPayload { kind: DecisionKind, reason: String },

    #[error("Token generation failed: {0}")]
    TokenGenerationFailed(String),
}

/// Builder for constructing DecisionPackets
#[derive(Default)]
pub struct DecisionPacketBuilder {
    kind: Option<DecisionKind>,
    utterance: Option<String>,
    payload: Option<ClarificationPayload>,
    session_state: Option<SessionStateView>,
    prompt: Option<String>,
    best_plan: Option<PlanPreview>,
    alternatives: Vec<PlanPreview>,
    requires_confirm: bool,
    trace_config_version: String,
    trace_decision_reason: String,
}

impl DecisionPacketBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            trace_config_version: "1.0".to_string(),
            trace_decision_reason: "user_input".to_string(),
            requires_confirm: true, // Default to requiring confirm in regulated domain
            ..Default::default()
        }
    }

    /// Set the decision kind
    pub fn kind(mut self, kind: DecisionKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// Set the original user utterance
    pub fn utterance(mut self, input: impl Into<String>) -> Self {
        self.utterance = Some(input.into());
        self
    }

    /// Set the session state view
    pub fn session_state(mut self, state: SessionStateView) -> Self {
        self.session_state = Some(state);
        self
    }

    /// Set the prompt
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Set best plan preview
    pub fn best_plan(mut self, plan: PlanPreview) -> Self {
        self.best_plan = Some(plan);
        self
    }

    /// Add alternative plan
    pub fn alternative(mut self, plan: PlanPreview) -> Self {
        self.alternatives.push(plan);
        self
    }

    /// Set requires_confirm flag
    pub fn requires_confirm(mut self, requires: bool) -> Self {
        self.requires_confirm = requires;
        self
    }

    /// Set trace config version
    pub fn trace_config_version(mut self, version: impl Into<String>) -> Self {
        self.trace_config_version = version.into();
        self
    }

    /// Set trace decision reason
    pub fn trace_decision_reason(mut self, reason: impl Into<String>) -> Self {
        self.trace_decision_reason = reason.into();
        self
    }

    /// Set payload directly
    pub fn payload(mut self, payload: ClarificationPayload) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Build a Proposal packet from DSL and effects
    pub fn proposal(mut self, payload: ProposalPayload) -> Self {
        self.kind = Some(DecisionKind::Proposal);
        self.payload = Some(ClarificationPayload::Proposal(payload));
        self
    }

    /// Build a ClarifyGroup packet from group options
    pub fn clarify_group(mut self, payload: GroupClarificationPayload) -> Self {
        self.kind = Some(DecisionKind::ClarifyGroup);
        self.payload = Some(ClarificationPayload::Group(payload));
        self
    }

    /// Build a ClarifyVerb packet from verb options
    pub fn clarify_verb(mut self, payload: VerbPayload) -> Self {
        self.kind = Some(DecisionKind::ClarifyVerb);
        self.payload = Some(ClarificationPayload::Verb(payload));
        self
    }

    /// Build a ClarifyScope packet from scope options
    pub fn clarify_scope(mut self, payload: ScopePayload) -> Self {
        self.kind = Some(DecisionKind::ClarifyScope);
        self.payload = Some(ClarificationPayload::Scope(payload));
        self
    }

    /// Build a Refuse packet
    pub fn refuse(mut self, payload: RefusePayload) -> Self {
        self.kind = Some(DecisionKind::Refuse);
        self.payload = Some(ClarificationPayload::Refuse(payload));
        self
    }

    /// Build the DecisionPacket
    pub fn build(self) -> Result<DecisionPacket, PacketBuildError> {
        let kind = self.kind.ok_or(PacketBuildError::MissingField("kind"))?;
        let utterance = self
            .utterance
            .ok_or(PacketBuildError::MissingField("utterance"))?;
        let payload = self
            .payload
            .ok_or(PacketBuildError::MissingField("payload"))?;

        // Validate payload matches kind
        match (&kind, &payload) {
            (DecisionKind::Proposal, ClarificationPayload::Proposal(_)) => {}
            (DecisionKind::ClarifyGroup, ClarificationPayload::Group(_)) => {}
            (DecisionKind::ClarifyVerb, ClarificationPayload::Verb(_)) => {}
            (DecisionKind::ClarifyScope, ClarificationPayload::Scope(_)) => {}
            (DecisionKind::Refuse, ClarificationPayload::Refuse(_)) => {}
            _ => {
                return Err(PacketBuildError::InvalidPayload {
                    kind,
                    reason: "Payload type does not match DecisionKind".to_string(),
                });
            }
        }

        // Generate confirm token for Proposal kind
        let confirm_token = if matches!(kind, DecisionKind::Proposal) {
            Some(
                generate_confirm_token()
                    .map_err(|e| PacketBuildError::TokenGenerationFailed(e.to_string()))?,
            )
        } else {
            None
        };

        // Generate prompt and choices from payload
        let (prompt, choices) = generate_prompt_and_choices(&kind, &payload, &self.prompt);

        Ok(DecisionPacket {
            packet_id: Uuid::new_v4().to_string(),
            kind,
            session: self.session_state.unwrap_or_default(),
            utterance,
            payload,
            prompt,
            choices,
            best_plan: self.best_plan,
            alternatives: self.alternatives,
            requires_confirm: self.requires_confirm,
            confirm_token,
            trace: DecisionTrace {
                config_version: self.trace_config_version,
                entity_snapshot_hash: None,
                lexicon_snapshot_hash: None,
                semantic_lane_enabled: true,
                embedding_model_id: Some("bge-small-en-v1.5".to_string()),
                verb_margin: 0.0,
                scope_margin: 0.0,
                kind_margin: 0.0,
                decision_reason: self.trace_decision_reason,
            },
        })
    }
}

/// Generate the prompt and choices from the payload
fn generate_prompt_and_choices(
    kind: &DecisionKind,
    payload: &ClarificationPayload,
    custom_prompt: &Option<String>,
) -> (String, Vec<UserChoice>) {
    match (kind, payload) {
        (DecisionKind::Proposal, ClarificationPayload::Proposal(p)) => {
            let prompt = custom_prompt.clone().unwrap_or_else(|| {
                format!(
                    "Ready to execute: {}\n\nType CONFIRM to proceed.",
                    p.summary
                )
            });
            let choices = vec![
                UserChoice {
                    id: "CONFIRM".to_string(),
                    label: "Confirm execution".to_string(),
                    description: "Execute the proposed DSL".to_string(),
                    is_escape: false,
                },
                UserChoice {
                    id: "CANCEL".to_string(),
                    label: "Cancel".to_string(),
                    description: "Abort without executing".to_string(),
                    is_escape: true,
                },
            ];
            (prompt, choices)
        }

        (DecisionKind::ClarifyGroup, ClarificationPayload::Group(g)) => {
            let prompt = custom_prompt
                .clone()
                .unwrap_or_else(|| "Which client group did you mean?".to_string());
            let choices = g
                .options
                .iter()
                .enumerate()
                .map(|(i, opt)| UserChoice {
                    id: letter_key(i),
                    label: opt.alias.clone(),
                    description: format!(
                        "Score: {:.0}%, Method: {}",
                        opt.score * 100.0,
                        opt.method
                    ),
                    is_escape: false,
                })
                .chain(std::iter::once(UserChoice {
                    id: "TYPE".to_string(),
                    label: "Type another name".to_string(),
                    description: "Enter a different search term".to_string(),
                    is_escape: true,
                }))
                .collect();
            (prompt, choices)
        }

        (DecisionKind::ClarifyVerb, ClarificationPayload::Verb(v)) => {
            let prompt = custom_prompt
                .clone()
                .unwrap_or_else(|| "Multiple verbs match. Which did you mean?".to_string());
            let choices = v
                .options
                .iter()
                .enumerate()
                .map(|(i, opt)| UserChoice {
                    id: letter_key(i),
                    label: opt.verb_fqn.clone(),
                    description: if opt.example.is_empty() {
                        format!("Score: {:.0}%", opt.score * 100.0)
                    } else {
                        opt.example.clone()
                    },
                    is_escape: false,
                })
                .chain(std::iter::once(UserChoice {
                    id: "CANCEL".to_string(),
                    label: "Cancel".to_string(),
                    description: "Start over".to_string(),
                    is_escape: true,
                }))
                .collect();
            (prompt, choices)
        }

        (DecisionKind::ClarifyScope, ClarificationPayload::Scope(s)) => {
            let prompt = custom_prompt
                .clone()
                .unwrap_or_else(|| "What scope should this apply to?".to_string());
            let choices = s
                .options
                .iter()
                .enumerate()
                .map(|(i, opt)| UserChoice {
                    id: letter_key(i),
                    label: opt.desc.clone(),
                    description: format!(
                        "Method: {}, Score: {:.0}%",
                        opt.method,
                        opt.score * 100.0
                    ),
                    is_escape: false,
                })
                .chain(std::iter::once(UserChoice {
                    id: "NARROW".to_string(),
                    label: "Narrow scope".to_string(),
                    description: "Add filter to narrow results".to_string(),
                    is_escape: true,
                }))
                .collect();
            (prompt, choices)
        }

        (DecisionKind::Refuse, ClarificationPayload::Refuse(r)) => {
            let prompt = custom_prompt.clone().unwrap_or_else(|| {
                format!("{}\n\n{}", r.reason, r.suggestion.as_deref().unwrap_or(""))
            });
            let choices = vec![UserChoice {
                id: "OK".to_string(),
                label: "Understood".to_string(),
                description: "Acknowledge and try something else".to_string(),
                is_escape: true,
            }];
            (prompt, choices)
        }

        _ => ("Unknown decision type".to_string(), vec![]),
    }
}

/// Convert index to letter key (A, B, C, ...)
fn letter_key(index: usize) -> String {
    let letter = (b'A' + index as u8) as char;
    letter.to_string()
}

// Convenience constructors for common patterns

/// Helper to create a simple proposal
pub fn build_proposal(
    utterance: impl Into<String>,
    dsl_source: impl Into<String>,
    summary: impl Into<String>,
) -> Result<DecisionPacket, PacketBuildError> {
    DecisionPacketBuilder::new()
        .utterance(utterance)
        .proposal(ProposalPayload {
            dsl_source: dsl_source.into(),
            summary: summary.into(),
            affected_entities: vec![],
            effects: EffectsPreview {
                mode: EffectMode::Write,
                summary: "Will create/modify entities".to_string(),
                affected_count: None,
            },
            warnings: vec![],
        })
        .build()
}

/// Helper to create a group clarification
pub fn build_group_clarification(
    utterance: impl Into<String>,
    options: Vec<GroupOption>,
) -> Result<DecisionPacket, PacketBuildError> {
    DecisionPacketBuilder::new()
        .utterance(utterance)
        .clarify_group(GroupClarificationPayload { options })
        .build()
}

/// Helper to create a verb clarification
pub fn build_verb_clarification(
    utterance: impl Into<String>,
    options: Vec<VerbOption>,
) -> Result<DecisionPacket, PacketBuildError> {
    DecisionPacketBuilder::new()
        .utterance(utterance)
        .clarify_verb(VerbPayload {
            options,
            context_hint: None,
        })
        .build()
}

/// Helper to create a refuse packet
pub fn build_refuse(
    utterance: impl Into<String>,
    reason: impl Into<String>,
    suggestion: Option<String>,
) -> Result<DecisionPacket, PacketBuildError> {
    DecisionPacketBuilder::new()
        .utterance(utterance)
        .refuse(RefusePayload {
            reason: reason.into(),
            suggestion,
        })
        .build()
}

/// Convert existing VerbDisambiguationRequest to DecisionPacket
#[cfg(feature = "database")]
pub fn from_verb_disambiguation(
    request: &ob_poc_types::VerbDisambiguationRequest,
) -> Result<DecisionPacket, PacketBuildError> {
    let options: Vec<VerbOption> = request
        .options
        .iter()
        .map(|opt| VerbOption {
            verb_fqn: opt.verb_fqn.clone(),
            description: opt.description.clone(),
            score: opt.score,
            example: opt.example.clone(),
            matched_phrase: None,
            domain_label: None,
            category_label: None,
        })
        .collect();

    build_verb_clarification(&request.original_input, options)
}

/// Convert existing IntentTierRequest to DecisionPacket
#[cfg(feature = "database")]
pub fn from_intent_tier(
    request: &ob_poc_types::IntentTierRequest,
) -> Result<DecisionPacket, PacketBuildError> {
    let options: Vec<ScopeOption> = request
        .options
        .iter()
        .map(|opt| ScopeOption {
            desc: opt.label.clone(),
            method: "tier".to_string(),
            score: 1.0, // Intent tiers don't have scores
            expect_count: Some(opt.verb_count),
            sample: vec![],
            snapshot_id: None,
        })
        .collect();

    DecisionPacketBuilder::new()
        .utterance(&request.original_input)
        .prompt(&request.prompt)
        .clarify_scope(ScopePayload {
            options,
            context_hint: None,
        })
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_proposal() {
        let packet = build_proposal(
            "create a fund",
            "(cbu.create :name \"Test\")",
            "Create a new CBU named Test",
        )
        .unwrap();

        assert!(matches!(packet.kind, DecisionKind::Proposal));
        assert!(packet.confirm_token.is_some());
        assert_eq!(packet.choices.len(), 2);
        assert_eq!(packet.choices[0].id, "CONFIRM");
        assert_eq!(packet.choices[1].id, "CANCEL");
    }

    #[test]
    fn test_build_group_clarification() {
        let packet = build_group_clarification(
            "load allianz",
            vec![
                GroupOption {
                    id: Uuid::new_v4().to_string(),
                    alias: "Allianz Global Investors".to_string(),
                    score: 0.95,
                    method: "alias".to_string(),
                },
                GroupOption {
                    id: Uuid::new_v4().to_string(),
                    alias: "Allianz SE".to_string(),
                    score: 0.85,
                    method: "alias".to_string(),
                },
            ],
        )
        .unwrap();

        assert!(matches!(packet.kind, DecisionKind::ClarifyGroup));
        assert!(packet.confirm_token.is_none());
        assert_eq!(packet.choices.len(), 3); // A, B, TYPE
        assert_eq!(packet.choices[0].id, "A");
        assert_eq!(packet.choices[1].id, "B");
        assert_eq!(packet.choices[2].id, "TYPE");
    }

    #[test]
    fn test_build_refuse() {
        let packet = build_refuse(
            "delete everything",
            "Bulk delete is not allowed",
            Some("Try deleting items individually".to_string()),
        )
        .unwrap();

        assert!(matches!(packet.kind, DecisionKind::Refuse));
        assert!(packet.confirm_token.is_none());
        assert_eq!(packet.choices.len(), 1);
        assert_eq!(packet.choices[0].id, "OK");
    }

    #[test]
    fn test_missing_field_error() {
        let result = DecisionPacketBuilder::new().build();
        assert!(matches!(result, Err(PacketBuildError::MissingField(_))));
    }

    #[test]
    fn test_payload_kind_mismatch() {
        let result = DecisionPacketBuilder::new()
            .kind(DecisionKind::Proposal)
            .utterance("test")
            .payload(ClarificationPayload::Refuse(RefusePayload {
                reason: "test".to_string(),
                suggestion: None,
            }))
            .build();

        assert!(matches!(
            result,
            Err(PacketBuildError::InvalidPayload { .. })
        ));
    }
}
