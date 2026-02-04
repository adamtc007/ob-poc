//! Deterministic Rendering Templates
//!
//! Renders DecisionPacket to user-facing text using ONLY packet fields.
//! NO hallucination - never invent entities, counts, verbs, or effects.

use ob_poc_types::{
    ClarificationPayload, DecisionPacket, EffectMode, GroupClarificationPayload, ProposalPayload,
    RefusePayload, ScopePayload, VerbPayload,
};

/// Render a DecisionPacket to user-facing markdown text.
///
/// This function uses ONLY the fields present in the packet.
/// It never invents or hallucinates content.
pub fn render_decision_packet(packet: &DecisionPacket) -> String {
    match &packet.payload {
        ClarificationPayload::Proposal(p) => render_proposal(packet, p),
        ClarificationPayload::Group(g) => render_group_clarification(packet, g),
        ClarificationPayload::Verb(v) => render_verb_clarification(packet, v),
        ClarificationPayload::Scope(s) => render_scope_clarification(packet, s),
        ClarificationPayload::Refuse(r) => render_refuse(packet, r),
    }
}

/// Render a Proposal packet (ready for execution, needs CONFIRM)
pub fn render_proposal(_packet: &DecisionPacket, payload: &ProposalPayload) -> String {
    let mut lines = Vec::new();

    lines.push("## Ready to Execute".to_string());
    lines.push(String::new());

    // Summary
    lines.push(format!("**{}**", payload.summary));
    lines.push(String::new());

    // DSL preview
    lines.push("```dsl".to_string());
    lines.push(payload.dsl_source.clone());
    lines.push("```".to_string());
    lines.push(String::new());

    // Effects summary
    lines.push("### Effects".to_string());
    lines.push(String::new());
    let effect_icon = match payload.effects.mode {
        EffectMode::ReadOnly => "👁️",
        EffectMode::Write => "✏️",
        EffectMode::Mixed => "🔄",
    };
    lines.push(format!("{} {}", effect_icon, payload.effects.summary));

    if let Some(count) = payload.effects.affected_count {
        lines.push(format!("Affected entities: {}", count));
    }
    lines.push(String::new());

    // Affected entities if present
    if !payload.affected_entities.is_empty() {
        lines.push("**Entities:**".to_string());
        for entity in &payload.affected_entities {
            lines.push(format!(
                "- {} ({}) `{}`",
                entity.canonical_name, entity.entity_kind, entity.entity_id
            ));
        }
        lines.push(String::new());
    }

    // Warnings if present
    if !payload.warnings.is_empty() {
        lines.push("### ⚠️ Warnings".to_string());
        for warning in &payload.warnings {
            lines.push(format!("- {}", warning));
        }
        lines.push(String::new());
    }

    // Confirm prompt
    lines.push("---".to_string());
    lines.push("Type **CONFIRM** to execute, or **CANCEL** to abort.".to_string());

    lines.join("\n")
}

/// Render a Group clarification (client group selection)
pub fn render_group_clarification(
    _packet: &DecisionPacket,
    payload: &GroupClarificationPayload,
) -> String {
    let mut lines = vec![
        "## Select Client Group".to_string(),
        String::new(),
        "Multiple client groups match your input. Please select one:".to_string(),
        String::new(),
    ];

    for (i, option) in payload.options.iter().enumerate() {
        let key = (b'A' + i as u8) as char;
        lines.push(format!(
            "**{}. {}** (score: {:.0}%, found via: {})",
            key,
            option.alias,
            option.score * 100.0,
            option.method
        ));
    }

    lines.push(String::new());
    lines.push("---".to_string());
    lines.push(
        "Type a letter (A/B/C...) to select, or **TYPE** followed by a name to search again."
            .to_string(),
    );

    lines.join("\n")
}

/// Render a Verb clarification (verb disambiguation)
pub fn render_verb_clarification(_packet: &DecisionPacket, payload: &VerbPayload) -> String {
    let mut lines = vec![
        "## Clarify Intent".to_string(),
        String::new(),
        "Multiple operations match your request. Which did you mean?".to_string(),
        String::new(),
    ];

    if let Some(hint) = &payload.context_hint {
        lines.push(format!("_Context: {}_", hint));
        lines.push(String::new());
    }

    for (i, option) in payload.options.iter().enumerate() {
        let key = (b'A' + i as u8) as char;
        lines.push(format!(
            "**{}. {}** (score: {:.0}%)",
            key,
            option.verb_fqn,
            option.score * 100.0
        ));
        lines.push(format!("   {}", option.description));
        if !option.example.is_empty() {
            lines.push(format!("   _Example: \"{}\"_", option.example));
        }
        lines.push(String::new());
    }

    lines.push("---".to_string());
    lines.push("Type a letter (A/B/C...) to select, or **CANCEL** to start over.".to_string());

    lines.join("\n")
}

/// Render a Scope clarification (scope/tier selection)
pub fn render_scope_clarification(_packet: &DecisionPacket, payload: &ScopePayload) -> String {
    let mut lines = vec![
        "## Select Scope".to_string(),
        String::new(),
        "What scope should this apply to?".to_string(),
        String::new(),
    ];

    if let Some(hint) = &payload.context_hint {
        lines.push(format!("_Context: {}_", hint));
        lines.push(String::new());
    }

    for (i, option) in payload.options.iter().enumerate() {
        let key = (b'A' + i as u8) as char;
        lines.push(format!(
            "**{}. {}** (score: {:.0}%, method: {})",
            key,
            option.desc,
            option.score * 100.0,
            option.method
        ));

        if let Some(count) = option.expect_count {
            lines.push(format!("   Expected: ~{} entities", count));
        }

        if !option.sample.is_empty() {
            let samples: Vec<&str> = option
                .sample
                .iter()
                .take(3)
                .map(|s| s.canonical_name.as_str())
                .collect();
            lines.push(format!("   Samples: {}", samples.join(", ")));
        }

        lines.push(String::new());
    }

    lines.push("---".to_string());
    lines.push(
        "Type a letter (A/B/C...) to select, or **NARROW** followed by a term to filter."
            .to_string(),
    );

    lines.join("\n")
}

/// Render a Refuse packet (cannot proceed)
pub fn render_refuse(_packet: &DecisionPacket, payload: &RefusePayload) -> String {
    let mut lines = Vec::new();

    lines.push("## Cannot Proceed".to_string());
    lines.push(String::new());
    lines.push(format!("❌ {}", payload.reason));
    lines.push(String::new());

    if let Some(suggestion) = &payload.suggestion {
        lines.push(format!("💡 **Suggestion**: {}", suggestion));
        lines.push(String::new());
    }

    lines.push("---".to_string());
    lines.push("Type **OK** to acknowledge, or try a different request.".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ob_poc_types::{
        AffectedEntityPreview, DecisionKind, DecisionTrace, EffectsPreview, GroupOption,
        SessionStateView, UserChoice, VerbOption,
    };

    fn make_proposal_packet() -> DecisionPacket {
        DecisionPacket {
            packet_id: "test".to_string(),
            kind: DecisionKind::Proposal,
            session: SessionStateView::default(),
            utterance: "create a fund".to_string(),
            confirm_token: Some("token".to_string()),
            payload: ClarificationPayload::Proposal(ProposalPayload {
                dsl_source: "(cbu.create :name \"Test Fund\")".to_string(),
                summary: "Create CBU named Test Fund".to_string(),
                affected_entities: vec![AffectedEntityPreview {
                    entity_id: "uuid-123".to_string(),
                    canonical_name: "Test Fund".to_string(),
                    entity_kind: "cbu".to_string(),
                }],
                effects: EffectsPreview {
                    mode: EffectMode::Write,
                    summary: "Will create 1 entity".to_string(),
                    affected_count: Some(1),
                },
                warnings: vec![],
            }),
            prompt: "Confirm?".to_string(),
            choices: vec![
                UserChoice {
                    id: "CONFIRM".to_string(),
                    label: "Execute".to_string(),
                    description: "Execute the DSL".to_string(),
                    is_escape: false,
                },
                UserChoice {
                    id: "CANCEL".to_string(),
                    label: "Cancel".to_string(),
                    description: "Abort".to_string(),
                    is_escape: true,
                },
            ],
            best_plan: None,
            alternatives: vec![],
            requires_confirm: true,
            trace: DecisionTrace {
                config_version: "1.0".to_string(),
                entity_snapshot_hash: None,
                lexicon_snapshot_hash: None,
                semantic_lane_enabled: true,
                embedding_model_id: None,
                verb_margin: 0.0,
                scope_margin: 0.0,
                kind_margin: 0.0,
                decision_reason: "test".to_string(),
            },
        }
    }

    #[test]
    fn test_render_proposal() {
        let packet = make_proposal_packet();
        let rendered = render_decision_packet(&packet);

        assert!(rendered.contains("Ready to Execute"));
        assert!(rendered.contains("(cbu.create"));
        assert!(rendered.contains("Test Fund"));
        assert!(rendered.contains("CONFIRM"));
    }

    #[test]
    fn test_render_group_clarification() {
        let packet = DecisionPacket {
            packet_id: "test".to_string(),
            kind: DecisionKind::ClarifyGroup,
            session: SessionStateView::default(),
            utterance: "allianz".to_string(),
            confirm_token: None,
            payload: ClarificationPayload::Group(GroupClarificationPayload {
                options: vec![
                    GroupOption {
                        id: "1".to_string(),
                        alias: "Allianz Global Investors".to_string(),
                        score: 0.95,
                        method: "alias".to_string(),
                    },
                    GroupOption {
                        id: "2".to_string(),
                        alias: "Allianz SE".to_string(),
                        score: 0.85,
                        method: "alias".to_string(),
                    },
                ],
            }),
            prompt: "Select group".to_string(),
            choices: vec![],
            best_plan: None,
            alternatives: vec![],
            requires_confirm: false,
            trace: DecisionTrace {
                config_version: "1.0".to_string(),
                entity_snapshot_hash: None,
                lexicon_snapshot_hash: None,
                semantic_lane_enabled: true,
                embedding_model_id: None,
                verb_margin: 0.0,
                scope_margin: 0.0,
                kind_margin: 0.0,
                decision_reason: "test".to_string(),
            },
        };

        let rendered = render_decision_packet(&packet);

        assert!(rendered.contains("Select Client Group"));
        assert!(rendered.contains("Allianz Global Investors"));
        assert!(rendered.contains("95%"));
    }

    #[test]
    fn test_render_verb_clarification() {
        let packet = DecisionPacket {
            packet_id: "test".to_string(),
            kind: DecisionKind::ClarifyVerb,
            session: SessionStateView::default(),
            utterance: "load book".to_string(),
            confirm_token: None,
            payload: ClarificationPayload::Verb(VerbPayload {
                options: vec![
                    VerbOption {
                        verb_fqn: "session.load-galaxy".to_string(),
                        description: "Load all CBUs under apex".to_string(),
                        score: 0.85,
                        example: "load the allianz book".to_string(),
                        matched_phrase: None,
                        domain_label: None,
                        category_label: None,
                    },
                    VerbOption {
                        verb_fqn: "session.load-cbu".to_string(),
                        description: "Load single CBU".to_string(),
                        score: 0.80,
                        example: String::new(),
                        matched_phrase: None,
                        domain_label: None,
                        category_label: None,
                    },
                ],
                context_hint: None,
            }),
            prompt: "Which verb?".to_string(),
            choices: vec![],
            best_plan: None,
            alternatives: vec![],
            requires_confirm: false,
            trace: DecisionTrace {
                config_version: "1.0".to_string(),
                entity_snapshot_hash: None,
                lexicon_snapshot_hash: None,
                semantic_lane_enabled: true,
                embedding_model_id: None,
                verb_margin: 0.0,
                scope_margin: 0.0,
                kind_margin: 0.0,
                decision_reason: "test".to_string(),
            },
        };

        let rendered = render_decision_packet(&packet);

        assert!(rendered.contains("Clarify Intent"));
        assert!(rendered.contains("session.load-galaxy"));
        assert!(rendered.contains("85%"));
    }
}
