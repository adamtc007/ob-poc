//! Sage-classification / Coder-drafting turn stages.
//!
//! T11.2 Part A (2026-07-13): relocated from `ob_poc::agent::orchestrator`,
//! the design doc's recommended first consumers of `AgentTurnContext` —
//! pure interpretation logic (Sage shadow classification, NLCI/Drafter
//! resolution), zero capability-crate coupling, confirmed by the T11.1b/
//! slice-2 boundary trace. See `docs/todo/control-plane/
//! EOP-DESIGN-CONTROLPLANE-T11.2-CAPABILITY-INVOCATION-001.md`.

use crate::agent_turn_context::AgentTurnContext;
use crate::sage::{DeterministicSage, DraftResult, DrafterEngine};
use crate::sage::drafter_result::DraftResolution;
use crate::sage::{OutcomeIntent, SageContext};
use crate::semtaxonomy_v2::{
    compiler_input_from_outcome_intent, supports_cbu_compiler_slice, CompilerSelection,
};
use std::sync::Arc;

pub struct SageStageOutcome {
    pub intent: Option<OutcomeIntent>,
}

pub struct DraftStageOutcome {
    pub result: Option<DraftResult>,
    pub elapsed_ms: Option<u128>,
    pub error: Option<String>,
}

pub async fn run_sage_stage(
    ctx: &AgentTurnContext,
    utterance: &str,
    enabled: bool,
) -> SageStageOutcome {
    if !enabled {
        return SageStageOutcome { intent: None };
    }

    let sage_ctx = SageContext {
        session_id: ctx.session_id,
        stage_focus: ctx.stage_focus.clone(),
        goals: ctx.goals.clone(),
        entity_kind: ctx.pre_sage_entity_kind.clone(),
        dominant_entity_name: ctx.pre_sage_entity_name.clone(),
        last_intents: ctx.recent_sage_intents.clone(),
    };
    let sage_engine = ctx
        .sage_engine
        .clone()
        .unwrap_or_else(|| Arc::new(DeterministicSage));

    let intent = match sage_engine.classify(utterance, &sage_ctx).await {
        Ok(intent) => {
            tracing::info!(
                sage_plane = ?intent.plane,
                sage_polarity = ?intent.polarity,
                sage_domain = %intent.domain_concept,
                "Stage 1.5: Sage shadow classification"
            );
            Some(intent)
        }
        Err(e) => {
            tracing::warn!(error = %e, "Stage 1.5: SageEngine failed (non-fatal)");
            None
        }
    };

    SageStageOutcome { intent }
}

pub fn run_coder_stage(
    ctx: &AgentTurnContext,
    intent: Option<&OutcomeIntent>,
) -> DraftStageOutcome {
    let Some(intent) = intent else {
        return DraftStageOutcome {
            result: None,
            elapsed_ms: None,
            error: None,
        };
    };

    let started_at = std::time::Instant::now();
    if let Some(compiler) = &ctx.nlci_compiler {
        let compiler_input = compiler_input_from_outcome_intent(
            intent,
            ctx.session_id,
            ctx.dominant_entity_id,
            ctx.pre_sage_entity_kind.as_deref(),
            ctx.pre_sage_entity_name.as_deref(),
        );
        if supports_cbu_compiler_slice(&compiler_input) {
            return match compiler.compile(compiler_input) {
                Ok(output) => match output.selection {
                    Some(selection) => DraftStageOutcome {
                        result: Some(coder_result_from_compiler_selection(selection)),
                        elapsed_ms: Some(started_at.elapsed().as_millis()),
                        error: None,
                    },
                    None => DraftStageOutcome {
                        result: None,
                        elapsed_ms: Some(started_at.elapsed().as_millis()),
                        error: Some(
                            output
                                .failure
                                .map(|failure| failure.user_message)
                                .unwrap_or_else(|| {
                                    "NLCI compiler returned no selection for supported CBU intent"
                                        .to_string()
                                }),
                        ),
                    },
                },
                Err(error) => DraftStageOutcome {
                    result: None,
                    elapsed_ms: Some(started_at.elapsed().as_millis()),
                    error: Some(error.to_string()),
                },
            };
        }
    }

    match DrafterEngine::load().and_then(|engine| engine.resolve(intent)) {
        Ok(drafter_result) => DraftStageOutcome {
            result: Some(drafter_result),
            elapsed_ms: Some(started_at.elapsed().as_millis()),
            error: None,
        },
        Err(error) => DraftStageOutcome {
            result: None,
            elapsed_ms: Some(started_at.elapsed().as_millis()),
            error: Some(error.to_string()),
        },
    }
}

pub fn coder_result_from_compiler_selection(selection: CompilerSelection) -> DraftResult {
    let dsl = render_selection_dsl(&selection);
    DraftResult {
        verb_fqn: selection.verb_id,
        dsl,
        resolution: DraftResolution::Confident,
        missing_args: vec![],
        unresolved_refs: vec![],
        diagnostics: None,
    }
}

pub fn render_selection_dsl(selection: &CompilerSelection) -> String {
    let args = selection
        .arguments
        .iter()
        .map(|(name, value)| format!(" :{} {}", name, render_dsl_string(value)))
        .collect::<String>();
    format!("({}{})", selection.verb_id, args)
}

pub fn render_dsl_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
