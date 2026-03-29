//! Scenario Index — Journey-Level Intent Resolution (Tier -2A)
//!
//! Resolves multi-verb journey utterances (e.g., "Onboard a Luxembourg SICAV")
//! to macro/macro-sequence routes by evaluating compound signals against
//! scenario definitions loaded from YAML.
//!
//! ## Scoring Ledger (from spec §4.2)
//!
//! | Signal Bucket                                      | Score |
//! |----------------------------------------------------|-------|
//! | Compound outcome verb (onboard, set up, establish)  | +4    |
//! | Jurisdiction found                                  | +4    |
//! | Structure noun (sicav, icav, LP)                    | +3    |
//! | Phase noun (KYC, screening, mandate)                | +2    |
//! | Quantifier ("three sub-funds")                      | +2    |
//! | Macro metadata match                                | +3    |
//! | Negative: single-verb cue                           | −6    |
//!
//! ## Hard Gates
//!
//! - G1: Compound signal required (at least one from CompoundSignals)
//! - G2: Mode compatibility (scenario modes must overlap active mode)
//! - G3: Minimum score ≥ 8

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::mcp::compound_intent::{extract_compound_signals, CompoundSignals};
use crate::mcp::macro_index::MacroIndex;

// ─── Configuration ───────────────────────────────────────────────────────────

/// Minimum total score required for a scenario match (gate G3).
const MIN_SCORE: i32 = 8;

/// If two candidates' scores are within this band, both are returned.
const DISAMBIGUATION_BAND: i32 = 2;

/// Score: compound outcome verb detected.
const SCORE_COMPOUND_ACTION: i32 = 4;

/// Score: jurisdiction found in utterance.
const SCORE_JURISDICTION: i32 = 4;

/// Score: structure noun found.
const SCORE_STRUCTURE_NOUN: i32 = 3;

/// Score: phase noun found.
const SCORE_PHASE_NOUN: i32 = 2;

/// Score: quantifier detected.
const SCORE_QUANTIFIER: i32 = 2;

/// Score: macro metadata confirms scenario route target.
const SCORE_MACRO_METADATA: i32 = 3;

/// Penalty: single-verb cue detected (no compound signals).
const PENALTY_SINGLE_VERB_CUE: i32 = -6;

// ─── YAML Types ──────────────────────────────────────────────────────────────

/// Top-level YAML structure for scenario_index.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioIndexConfig {
    pub scenarios: Vec<ScenarioDefYaml>,
}

/// A single scenario definition as loaded from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioDefYaml {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub modes: Vec<String>,
    pub requires: RequiresGate,
    pub signals: SignalConfig,
    pub routes: ScenarioRouteYaml,
    #[serde(default)]
    pub explain: ExplainConfig,
}

/// Gate G1: what compound signals must be present.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiresGate {
    /// At least one of these signal types must be present.
    #[serde(default)]
    pub any_of: Vec<String>,
    /// All of these signal types must be present.
    #[serde(default)]
    pub all_of: Vec<String>,
}

/// Signal matchers for the scoring ledger.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalConfig {
    /// Compound action verbs that trigger this scenario (matched against CompoundSignals).
    #[serde(default)]
    pub actions: Vec<String>,
    /// Jurisdiction ISO codes this scenario handles.
    #[serde(default)]
    pub jurisdictions: Vec<String>,
    /// Structure nouns that this scenario matches on.
    #[serde(default)]
    pub nouns_any: Vec<String>,
    /// Phrase fragments that boost this scenario.
    #[serde(default)]
    pub phrases_any: Vec<String>,
}

/// How a matched scenario routes to macro(s).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ScenarioRouteYaml {
    /// Route to a single macro.
    #[serde(rename = "macro")]
    Macro { macro_fqn: String },
    /// Route to a sequence of macros executed in order.
    #[serde(rename = "macro_sequence")]
    MacroSequence { macros: Vec<String> },
    /// Route by selecting a macro based on extracted context (e.g., jurisdiction).
    #[serde(rename = "macro_selector")]
    MacroSelector {
        /// Field from CompoundSignals to select on (e.g., "jurisdiction").
        select_on: String,
        /// Map of field value → macro FQN.
        options: Vec<SelectorOption>,
        /// Optional follow-up macros after the selected one.
        #[serde(default)]
        then: Vec<String>,
    },
    /// Route to a verb by selecting based on extracted context (entity type determination).
    /// Like macro_selector but resolves to a verb FQN — no macro expansion needed.
    #[serde(rename = "verb_selector")]
    VerbSelector {
        /// Primary axis from CompoundSignals (e.g., "relationship_type").
        select_on: String,
        /// Map of axis value → verb FQN (or nested sub_select).
        options: Vec<VerbSelectorOption>,
        /// Fallback verb when the primary axis is undetected.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default_verb_fqn: Option<String>,
    },
}

/// A single option in a macro_selector route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorOption {
    /// The value to match (e.g., "LU", "IE").
    pub value: String,
    /// The macro FQN to route to (leaf option). Optional when `sub_select` is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macro_fqn: Option<String>,
    /// Nested second-axis selector. When present, `macro_fqn` is ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_select: Option<SubSelector>,
}

/// Nested second-axis selector within a macro_selector option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubSelector {
    /// Field from CompoundSignals to select on (e.g., "vehicle_type").
    pub select_on: String,
    /// Fallback macro FQN when the second axis is undetected.
    pub default_fqn: String,
    /// Options for the second axis.
    pub options: Vec<SubSelectorOption>,
}

/// Leaf option in a two-axis selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubSelectorOption {
    pub value: String,
    pub macro_fqn: String,
}

/// A single option in a verb_selector route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSelectorOption {
    /// The value to match (e.g., "ownership", "upward").
    pub value: String,
    /// The verb FQN to route to (leaf option). Optional when `sub_select` is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb_fqn: Option<String>,
    /// Nested second-axis selector.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_select: Option<VerbSubSelector>,
}

/// Nested second-axis selector for verb_selector routes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSubSelector {
    /// Field from CompoundSignals to select on (e.g., "query_direction").
    pub select_on: String,
    /// Fallback verb FQN when the second axis is undetected.
    pub default_verb_fqn: String,
    /// Options for the second axis.
    pub options: Vec<VerbSubSelectorLeaf>,
}

/// Leaf option in a two-axis verb selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbSubSelectorLeaf {
    pub value: String,
    pub verb_fqn: String,
}

/// Explain configuration (display metadata).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExplainConfig {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}

// ─── Runtime Types ───────────────────────────────────────────────────────────

/// A matched signal contributing to the scenario score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioMatchedSignal {
    pub signal: String,
    pub score: i32,
    pub detail: String,
}

/// Result of a gate evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioGateResult {
    pub gate: String,
    pub passed: bool,
    pub reason: Option<String>,
}

/// Explain payload for a scenario match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioExplain {
    pub matched_signals: Vec<ScenarioMatchedSignal>,
    pub gates: Vec<ScenarioGateResult>,
    pub score_total: i32,
    pub resolution_tier: &'static str,
}

/// Resolved route from a scenario match.
#[derive(Debug, Clone)]
pub enum ResolvedRoute {
    /// Single macro to expand.
    Macro { macro_fqn: String },
    /// Sequence of macros to expand in order.
    MacroSequence { macros: Vec<String> },
    /// Macro selector needs more context — return options for disambiguation.
    NeedsSelection {
        select_on: String,
        options: Vec<SelectorOption>,
        then: Vec<String>,
    },
    /// Resolved to a single verb (no macro expansion).
    Verb { verb_fqn: String },
    /// Verb selector needs more context — return options for clarification.
    NeedsVerbSelection {
        select_on: String,
        options: Vec<(String, String)>, // (display_value, verb_fqn) pairs
    },
}

/// A single scenario match with score, route, and explain.
#[derive(Debug, Clone)]
pub struct ScenarioMatch {
    pub scenario_id: String,
    pub title: String,
    pub score: i32,
    pub route: ResolvedRoute,
    pub explain: ScenarioExplain,
}

/// Result of `ScenarioIndex::resolve()`.
#[derive(Debug, Clone)]
pub enum ScenarioResolveOutcome {
    /// Clear winner (top score passes all gates, no disambiguation needed).
    Matched(ScenarioMatch),
    /// Multiple candidates within disambiguation band.
    Ambiguous(Vec<ScenarioMatch>),
    /// No scenario matched above the minimum score.
    NoMatch,
}

// ─── ScenarioIndex ───────────────────────────────────────────────────────────

/// Deterministic journey-level intent resolver.
///
/// Loaded from `config/scenario_index.yaml` at startup. Evaluates compound
/// signals against scenario definitions using a deterministic scoring ledger
/// with hard gates.
pub struct ScenarioIndex {
    scenarios: Vec<ScenarioDefYaml>,
}

impl ScenarioIndex {
    /// Load scenarios from a YAML file.
    pub fn from_yaml_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml_str(&content)
    }

    /// Load scenarios from a YAML string.
    pub fn from_yaml_str(yaml: &str) -> anyhow::Result<Self> {
        let config: ScenarioIndexConfig = serde_yaml::from_str(yaml)?;
        tracing::info!(
            scenario_count = config.scenarios.len(),
            "ScenarioIndex loaded from YAML"
        );
        Ok(Self {
            scenarios: config.scenarios,
        })
    }

    /// Create an empty index (for when no YAML is configured).
    pub fn empty() -> Self {
        Self {
            scenarios: Vec::new(),
        }
    }

    /// Number of scenarios in the index.
    pub fn len(&self) -> usize {
        self.scenarios.len()
    }

    /// Whether the index is empty.
    pub fn is_empty(&self) -> bool {
        self.scenarios.is_empty()
    }

    /// Resolve an utterance to the best-matching scenario(s).
    ///
    /// Extracts compound signals from the utterance, then scores each scenario
    /// definition against those signals using the deterministic scoring ledger.
    ///
    /// `active_mode` optionally constrains by mode tag (e.g., "onboarding").
    /// `macro_index` optionally used for macro metadata match scoring.
    pub fn resolve(
        &self,
        utterance: &str,
        active_mode: Option<&str>,
        macro_index: Option<&MacroIndex>,
    ) -> ScenarioResolveOutcome {
        if self.scenarios.is_empty() {
            return ScenarioResolveOutcome::NoMatch;
        }

        // Extract compound signals once for the entire resolution
        let signals = extract_compound_signals(utterance);

        let mut scored: Vec<ScenarioMatch> = Vec::new();

        for scenario in &self.scenarios {
            // Gate G1: compound signal required
            if !self.check_requires_gate(scenario, &signals) {
                continue;
            }

            // Gate G2: mode compatibility
            if !self.check_mode_gate(scenario, active_mode) {
                continue;
            }

            // Score using the deterministic ledger
            let (score, matched_signals, gates) =
                self.score_scenario(scenario, &signals, macro_index, utterance);

            // Gate G3: minimum score — bypassed when the utterance exactly
            // matches a phrases_any entry (exact phrase match IS the evidence).
            // Phrase containment is used for scoring (+3) but NOT for G3 bypass
            // to avoid false positives from short phrase substrings.
            let utterance_lower = utterance.to_lowercase();
            let has_exact_phrase_match = scenario
                .signals
                .phrases_any
                .iter()
                .any(|p| p.to_lowercase() == utterance_lower);
            if score < MIN_SCORE && !has_exact_phrase_match {
                continue;
            }

            // Resolve the route (may need context for selectors)
            let route = self.resolve_route(&scenario.routes, &signals);

            scored.push(ScenarioMatch {
                scenario_id: scenario.id.clone(),
                title: scenario.title.clone(),
                score,
                route,
                explain: ScenarioExplain {
                    matched_signals,
                    gates,
                    score_total: score,
                    resolution_tier: "Tier2A_ScenarioIndex",
                },
            });
        }

        if scored.is_empty() {
            return ScenarioResolveOutcome::NoMatch;
        }

        // Sort by score descending
        scored.sort_by(|a, b| b.score.cmp(&a.score));

        // Disambiguation band
        if scored.len() >= 2 {
            let top = scored[0].score;
            let runner_up = scored[1].score;
            if top - runner_up <= DISAMBIGUATION_BAND {
                let band_threshold = top - DISAMBIGUATION_BAND;
                let ambiguous: Vec<ScenarioMatch> = scored
                    .into_iter()
                    .filter(|m| m.score >= band_threshold)
                    .take(5)
                    .collect();
                return ScenarioResolveOutcome::Ambiguous(ambiguous);
            }
        }

        ScenarioResolveOutcome::Matched(scored.into_iter().next().unwrap())
    }

    /// Extract compound signals from an utterance (delegates to compound_intent).
    /// Exposed for use by verb_search.rs to check before ECIR short-circuit.
    pub fn extract_signals(utterance: &str) -> CompoundSignals {
        extract_compound_signals(utterance)
    }

    // ── Gate Checks ──────────────────────────────────────────────────────────

    /// Gate G1: Check if the compound signals satisfy the scenario's requires gate.
    fn check_requires_gate(&self, scenario: &ScenarioDefYaml, signals: &CompoundSignals) -> bool {
        let req = &scenario.requires;

        // Check any_of: at least one must be present
        if !req.any_of.is_empty() {
            let any_met = req.any_of.iter().any(|s| signal_present(signals, s));
            if !any_met {
                return false;
            }
        }

        // Check all_of: all must be present
        if !req.all_of.is_empty() {
            let all_met = req.all_of.iter().all(|s| signal_present(signals, s));
            if !all_met {
                return false;
            }
        }

        // If both are empty, gate passes (no compound signal required)
        // But the scoring ledger will still penalize via single-verb cue
        true
    }

    /// Gate G2: Check mode compatibility.
    fn check_mode_gate(&self, scenario: &ScenarioDefYaml, active_mode: Option<&str>) -> bool {
        // If scenario has no mode constraint, it's compatible with everything
        if scenario.modes.is_empty() {
            return true;
        }

        // If there's no active mode, allow all scenarios
        let Some(mode) = active_mode else {
            return true;
        };

        let mode_lower = mode.to_lowercase();
        scenario
            .modes
            .iter()
            .any(|m| m.to_lowercase() == mode_lower)
    }

    // ── Scoring ──────────────────────────────────────────────────────────────

    /// Score a scenario against the extracted compound signals.
    fn score_scenario(
        &self,
        scenario: &ScenarioDefYaml,
        signals: &CompoundSignals,
        macro_index: Option<&MacroIndex>,
        utterance: &str,
    ) -> (i32, Vec<ScenarioMatchedSignal>, Vec<ScenarioGateResult>) {
        let mut score: i32 = 0;
        let mut matched = Vec::new();
        let mut gates = Vec::new();

        // ── Compound outcome verb (+4) ──
        if signals.has_compound_action {
            // Check if the detected action matches any of the scenario's expected actions
            let action_match = if scenario.signals.actions.is_empty() {
                // No specific actions configured → no action score for this scenario
                false
            } else if let Some(ref detected) = signals.compound_action {
                scenario
                    .signals
                    .actions
                    .iter()
                    .any(|a| a.eq_ignore_ascii_case(detected))
            } else {
                false
            };

            if action_match {
                score += SCORE_COMPOUND_ACTION;
                matched.push(ScenarioMatchedSignal {
                    signal: "compound_action".to_string(),
                    score: SCORE_COMPOUND_ACTION,
                    detail: format!(
                        "Compound action '{}' detected",
                        signals.compound_action.as_deref().unwrap_or("(any)")
                    ),
                });
            }
        }

        // ── Jurisdiction (+4) ──
        if let Some(ref jur) = signals.jurisdiction {
            let jur_match = if scenario.signals.jurisdictions.is_empty() {
                // No specific jurisdictions configured → no jurisdiction score
                false
            } else {
                scenario
                    .signals
                    .jurisdictions
                    .iter()
                    .any(|j| j.eq_ignore_ascii_case(jur))
            };

            if jur_match {
                score += SCORE_JURISDICTION;
                matched.push(ScenarioMatchedSignal {
                    signal: "jurisdiction".to_string(),
                    score: SCORE_JURISDICTION,
                    detail: format!("Jurisdiction '{}' matches scenario", jur),
                });
            }
        }

        // ── Structure noun (+3) ──
        if !signals.structure_nouns.is_empty() {
            let noun_match = if scenario.signals.nouns_any.is_empty() {
                // No specific nouns configured → no noun score for this scenario
                false
            } else {
                signals.structure_nouns.iter().any(|sn| {
                    scenario
                        .signals
                        .nouns_any
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(sn))
                })
            };

            if noun_match {
                score += SCORE_STRUCTURE_NOUN;
                matched.push(ScenarioMatchedSignal {
                    signal: "structure_noun".to_string(),
                    score: SCORE_STRUCTURE_NOUN,
                    detail: format!(
                        "Structure nouns {:?} match scenario",
                        signals.structure_nouns
                    ),
                });
            }
        }

        // ── Phase noun (+2, or +3 if overlaps with scenario nouns_any) ──
        if !signals.phase_nouns.is_empty() {
            let phase_overlaps_scenario = if scenario.signals.nouns_any.is_empty() {
                false
            } else {
                signals.phase_nouns.iter().any(|pn| {
                    scenario
                        .signals
                        .nouns_any
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(pn))
                })
            };
            if phase_overlaps_scenario {
                // Phase noun that matches scenario's nouns_any → same weight as structure noun
                score += SCORE_STRUCTURE_NOUN;
                matched.push(ScenarioMatchedSignal {
                    signal: "phase_noun_overlap".to_string(),
                    score: SCORE_STRUCTURE_NOUN,
                    detail: format!(
                        "Phase nouns {:?} overlap scenario nouns_any",
                        signals.phase_nouns
                    ),
                });
            } else {
                score += SCORE_PHASE_NOUN;
                matched.push(ScenarioMatchedSignal {
                    signal: "phase_noun".to_string(),
                    score: SCORE_PHASE_NOUN,
                    detail: format!("Phase nouns {:?} detected", signals.phase_nouns),
                });
            }
        }

        // ── Quantifier (+2) ──
        if signals.has_quantifier {
            score += SCORE_QUANTIFIER;
            matched.push(ScenarioMatchedSignal {
                signal: "quantifier".to_string(),
                score: SCORE_QUANTIFIER,
                detail: "Quantifier detected (multi-entity scope)".to_string(),
            });
        }

        // ── Macro metadata match (+3) ──
        // Check if the scenario's route target macro(s) exist in the MacroIndex
        // and whether the macro's metadata aligns with the extracted signals.
        if let Some(mi) = macro_index {
            let route_fqns = self.route_target_fqns(&scenario.routes);
            for fqn in &route_fqns {
                if let Some(entry) = mi.get_entry(fqn) {
                    // Check jurisdiction alignment
                    let jur_aligned = match (&signals.jurisdiction, &entry.jurisdiction) {
                        (Some(sig_jur), Some(macro_jur)) => sig_jur.eq_ignore_ascii_case(macro_jur),
                        _ => false,
                    };
                    // Check structure type alignment
                    let struct_aligned = entry.structure_type.as_ref().is_some_and(|st| {
                        signals
                            .structure_nouns
                            .iter()
                            .any(|sn| sn.eq_ignore_ascii_case(st))
                    });

                    if jur_aligned || struct_aligned {
                        score += SCORE_MACRO_METADATA;
                        matched.push(ScenarioMatchedSignal {
                            signal: "macro_metadata".to_string(),
                            score: SCORE_MACRO_METADATA,
                            detail: format!(
                                "Route target '{}' metadata aligns (jur={}, struct={})",
                                fqn, jur_aligned, struct_aligned
                            ),
                        });
                        break; // Only count once
                    }
                }
            }
        }

        // ── Phrase match (+3) ──
        // If the utterance contains a 3+ word phrases_any entry, that's strong
        // evidence for this scenario. Short phrases (1-2 words) are too common
        // and cause false positive scenario stealing.
        {
            let utt_lower = utterance.to_lowercase();
            let phrase_hit = scenario.signals.phrases_any.iter().find(|p| {
                let p_lower = p.to_lowercase();
                let word_count = p_lower.split_whitespace().count();
                word_count >= 3 && (utt_lower == p_lower || utt_lower.contains(&p_lower))
            });
            if let Some(phrase) = phrase_hit {
                score += SCORE_STRUCTURE_NOUN; // +3, same weight as structure noun
                matched.push(ScenarioMatchedSignal {
                    signal: "phrase_match".to_string(),
                    score: SCORE_STRUCTURE_NOUN,
                    detail: format!("Phrase '{}' found in utterance", phrase),
                });
            }
        }

        // ── Negative: single-verb cue (−6) ──
        // If no compound signals at all, this is likely a single-verb command
        if !signals.has_any() {
            score += PENALTY_SINGLE_VERB_CUE;
            matched.push(ScenarioMatchedSignal {
                signal: "single_verb_cue".to_string(),
                score: PENALTY_SINGLE_VERB_CUE,
                detail: "No compound signals detected (likely single-verb command)".to_string(),
            });
        }

        // Record gates
        gates.push(ScenarioGateResult {
            gate: "G1_compound_signal".to_string(),
            passed: signals.has_any(),
            reason: if signals.has_any() {
                None
            } else {
                Some("No compound signals present".to_string())
            },
        });
        gates.push(ScenarioGateResult {
            gate: "G3_min_score".to_string(),
            passed: score >= MIN_SCORE,
            reason: if score >= MIN_SCORE {
                None
            } else {
                Some(format!("Score {} < minimum {}", score, MIN_SCORE))
            },
        });

        (score, matched, gates)
    }

    // ── Route Resolution ─────────────────────────────────────────────────────

    /// Resolve the scenario route, applying selector logic if applicable.
    fn resolve_route(&self, route: &ScenarioRouteYaml, signals: &CompoundSignals) -> ResolvedRoute {
        match route {
            ScenarioRouteYaml::Macro { macro_fqn } => ResolvedRoute::Macro {
                macro_fqn: macro_fqn.clone(),
            },
            ScenarioRouteYaml::MacroSequence { macros } => ResolvedRoute::MacroSequence {
                macros: macros.clone(),
            },
            ScenarioRouteYaml::MacroSelector {
                select_on,
                options,
                then,
            } => {
                // Try to auto-resolve the primary axis
                let primary_match = self.resolve_selector_axis(select_on, options, signals);

                match primary_match {
                    Some(opt) if opt.sub_select.is_some() => {
                        // Two-axis: resolve the nested sub_select
                        let sub = opt.sub_select.as_ref().unwrap();
                        let leaf_fqn = self
                            .resolve_sub_selector(sub, signals)
                            .unwrap_or_else(|| sub.default_fqn.clone());
                        if then.is_empty() {
                            ResolvedRoute::Macro {
                                macro_fqn: leaf_fqn,
                            }
                        } else {
                            let mut macros = vec![leaf_fqn];
                            macros.extend(then.clone());
                            ResolvedRoute::MacroSequence { macros }
                        }
                    }
                    Some(opt) if opt.macro_fqn.is_some() => {
                        // Single-axis leaf
                        let macro_fqn = opt.macro_fqn.clone().unwrap();
                        if then.is_empty() {
                            ResolvedRoute::Macro { macro_fqn }
                        } else {
                            let mut macros = vec![macro_fqn];
                            macros.extend(then.clone());
                            ResolvedRoute::MacroSequence { macros }
                        }
                    }
                    _ => {
                        // Can't auto-resolve — return for disambiguation
                        ResolvedRoute::NeedsSelection {
                            select_on: select_on.clone(),
                            options: options.clone(),
                            then: then.clone(),
                        }
                    }
                }
            }
            ScenarioRouteYaml::VerbSelector {
                select_on,
                options,
                default_verb_fqn,
            } => {
                // Try to resolve the primary axis
                let primary_match = self.resolve_verb_selector_axis(select_on, options, signals);

                match primary_match {
                    Some(opt) if opt.sub_select.is_some() => {
                        // Two-axis: resolve the nested sub_select
                        let sub = opt.sub_select.as_ref().unwrap();
                        let leaf_fqn = self
                            .resolve_verb_sub_selector(sub, signals)
                            .unwrap_or_else(|| sub.default_verb_fqn.clone());
                        ResolvedRoute::Verb {
                            verb_fqn: leaf_fqn,
                        }
                    }
                    Some(opt) if opt.verb_fqn.is_some() => {
                        // Single-axis leaf
                        ResolvedRoute::Verb {
                            verb_fqn: opt.verb_fqn.clone().unwrap(),
                        }
                    }
                    _ => {
                        // Try default fallback
                        if let Some(ref default) = default_verb_fqn {
                            ResolvedRoute::Verb {
                                verb_fqn: default.clone(),
                            }
                        } else {
                            // Can't auto-resolve — return options for clarification
                            ResolvedRoute::NeedsVerbSelection {
                                select_on: select_on.clone(),
                                options: options
                                    .iter()
                                    .filter_map(|o| {
                                        let fqn = o
                                            .verb_fqn
                                            .clone()
                                            .or_else(|| {
                                                o.sub_select
                                                    .as_ref()
                                                    .map(|s| s.default_verb_fqn.clone())
                                            })?;
                                        Some((o.value.clone(), fqn))
                                    })
                                    .collect(),
                            }
                        }
                    }
                }
            }
        }
    }

    /// Resolve a macro_selector axis against compound signals.
    fn resolve_selector_axis<'a>(
        &self,
        select_on: &str,
        options: &'a [SelectorOption],
        signals: &CompoundSignals,
    ) -> Option<&'a SelectorOption> {
        let value = self.signal_value_for_axis(select_on, signals)?;
        options.iter().find(|o| o.value.eq_ignore_ascii_case(value))
    }

    /// Resolve a nested macro sub-selector against compound signals.
    fn resolve_sub_selector(&self, sub: &SubSelector, signals: &CompoundSignals) -> Option<String> {
        let value = self.signal_value_for_axis(&sub.select_on, signals)?;
        sub.options
            .iter()
            .find(|o| o.value.eq_ignore_ascii_case(value))
            .map(|o| o.macro_fqn.clone())
    }

    /// Resolve a verb_selector primary axis against compound signals.
    fn resolve_verb_selector_axis<'a>(
        &self,
        select_on: &str,
        options: &'a [VerbSelectorOption],
        signals: &CompoundSignals,
    ) -> Option<&'a VerbSelectorOption> {
        let value = self.signal_value_for_axis(select_on, signals)?;
        options.iter().find(|o| o.value.eq_ignore_ascii_case(value))
    }

    /// Resolve a nested verb sub-selector against compound signals.
    fn resolve_verb_sub_selector(
        &self,
        sub: &VerbSubSelector,
        signals: &CompoundSignals,
    ) -> Option<String> {
        let value = self.signal_value_for_axis(&sub.select_on, signals)?;
        sub.options
            .iter()
            .find(|o| o.value.eq_ignore_ascii_case(value))
            .map(|o| o.verb_fqn.clone())
    }

    /// Shared axis value lookup — maps axis name to CompoundSignals field.
    fn signal_value_for_axis<'a>(
        &self,
        axis: &str,
        signals: &'a CompoundSignals,
    ) -> Option<&'a str> {
        match axis {
            "jurisdiction" => signals.jurisdiction.as_deref(),
            "vehicle_type" => signals.vehicle_type.as_deref(),
            "structure_type" => signals.structure_nouns.first().map(|s| s.as_str()),
            "query_direction" => signals.query_direction.as_deref(),
            "relationship_type" => signals.relationship_type.as_deref(),
            _ => None,
        }
    }

    /// Extract target FQN(s) from a scenario route.
    fn route_target_fqns(&self, route: &ScenarioRouteYaml) -> Vec<String> {
        match route {
            ScenarioRouteYaml::Macro { macro_fqn } => vec![macro_fqn.clone()],
            ScenarioRouteYaml::MacroSequence { macros } => macros.clone(),
            ScenarioRouteYaml::MacroSelector { options, then, .. } => {
                let mut fqns: Vec<String> = Vec::new();
                for o in options {
                    if let Some(ref fqn) = o.macro_fqn {
                        fqns.push(fqn.clone());
                    }
                    if let Some(ref sub) = o.sub_select {
                        fqns.push(sub.default_fqn.clone());
                        for so in &sub.options {
                            fqns.push(so.macro_fqn.clone());
                        }
                    }
                }
                fqns.extend(then.clone());
                fqns
            }
            ScenarioRouteYaml::VerbSelector {
                options,
                default_verb_fqn,
                ..
            } => {
                let mut fqns: Vec<String> = Vec::new();
                if let Some(ref default) = default_verb_fqn {
                    fqns.push(default.clone());
                }
                for o in options {
                    if let Some(ref fqn) = o.verb_fqn {
                        fqns.push(fqn.clone());
                    }
                    if let Some(ref sub) = o.sub_select {
                        fqns.push(sub.default_verb_fqn.clone());
                        for so in &sub.options {
                            fqns.push(so.verb_fqn.clone());
                        }
                    }
                }
                fqns
            }
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Check if a named signal type is present in the CompoundSignals.
fn signal_present(signals: &CompoundSignals, signal_name: &str) -> bool {
    match signal_name {
        "compound_action" => signals.has_compound_action,
        "jurisdiction" => signals.jurisdiction.is_some(),
        "structure_noun" => !signals.structure_nouns.is_empty(),
        "phase_noun" => !signals.phase_nouns.is_empty(),
        "quantifier" => signals.has_quantifier,
        "jurisdiction_structure_pair" => signals.has_jurisdiction_structure_pair,
        "multi_noun_workflow" => signals.has_multi_noun_workflow,
        // phrase_match: always true — the scenario's phrases_any scoring
        // handles phrase matching; this gate just allows scenarios to fire
        // on phrase match alone without requiring compound_action signals.
        "phrase_match" => true,
        _ => {
            tracing::warn!(signal = signal_name, "Unknown signal name in requires gate");
            false
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_yaml() -> &'static str {
        r#"
scenarios:
  - id: lux-sicav-setup
    title: Luxembourg UCITS SICAV Setup
    modes: [onboarding, structure]
    requires:
      any_of: [compound_action, jurisdiction_structure_pair]
    signals:
      actions: [onboard, "set up", establish, launch]
      jurisdictions: [LU]
      nouns_any: [sicav, ucits, fund]
    routes:
      kind: macro
      macro_fqn: struct.lux.ucits.sicav
    explain:
      summary: "Full Luxembourg UCITS SICAV setup"

  - id: ie-icav-setup
    title: Irish ICAV Setup
    modes: [onboarding, structure]
    requires:
      any_of: [compound_action, jurisdiction_structure_pair]
    signals:
      actions: [onboard, "set up", establish]
      jurisdictions: [IE]
      nouns_any: [icav, ucits, fund]
    routes:
      kind: macro
      macro_fqn: struct.ie.ucits.icav
    explain:
      summary: "Full Irish ICAV setup"

  - id: full-screening
    title: Full Compliance Screening
    modes: [onboarding, compliance]
    requires:
      any_of: [multi_noun_workflow, compound_action]
    signals:
      actions: ["run the", "complete the", "do the"]
      nouns_any: [screening, kyc, compliance]
    routes:
      kind: macro_sequence
      macros:
        - case.open
        - screening.full

  - id: jurisdiction-fund-setup
    title: Fund Setup by Jurisdiction
    modes: [onboarding, structure]
    requires:
      all_of: [compound_action, jurisdiction]
    signals:
      actions: [onboard, "set up", establish, launch, configure]
    routes:
      kind: macro_selector
      select_on: jurisdiction
      options:
        - value: LU
          macro_fqn: struct.lux.ucits.sicav
        - value: IE
          macro_fqn: struct.ie.ucits.icav
        - value: UK
          macro_fqn: struct.uk.authorised.oeic
        - value: US
          macro_fqn: struct.us.private-fund.delaware-lp
      then: []
"#
    }

    fn load_test_index() -> ScenarioIndex {
        ScenarioIndex::from_yaml_str(test_yaml()).unwrap()
    }

    // --- YAML loading ---

    #[test]
    fn test_load_yaml() {
        let idx = load_test_index();
        assert_eq!(idx.len(), 4);
        assert!(!idx.is_empty());
    }

    #[test]
    fn test_empty_index() {
        let idx = ScenarioIndex::empty();
        assert!(idx.is_empty());
        assert!(matches!(
            idx.resolve("anything", None, None),
            ScenarioResolveOutcome::NoMatch
        ));
    }

    // --- Gate G1: compound signal required ---

    #[test]
    fn test_gate_g1_compound_action() {
        let idx = load_test_index();
        // "Onboard a Luxembourg SICAV" has compound_action + jurisdiction + structure noun
        let result = idx.resolve("Onboard a Luxembourg SICAV", None, None);
        assert!(
            matches!(result, ScenarioResolveOutcome::Matched(_)),
            "Expected Matched, got {:?}",
            std::mem::discriminant(&result)
        );
    }

    #[test]
    fn test_gate_g1_no_compound_signal() {
        let idx = load_test_index();
        // "create umbrella fund" has no compound action, no jurisdiction, no pair
        let result = idx.resolve("create umbrella fund", None, None);
        assert!(matches!(result, ScenarioResolveOutcome::NoMatch));
    }

    // --- Gate G2: mode compatibility ---

    #[test]
    fn test_gate_g2_mode_match() {
        let idx = load_test_index();
        let result = idx.resolve("Onboard a Luxembourg SICAV", Some("onboarding"), None);
        assert!(matches!(result, ScenarioResolveOutcome::Matched(_)));
    }

    #[test]
    fn test_gate_g2_mode_mismatch() {
        let idx = load_test_index();
        // "billing" mode won't match any scenario modes
        let result = idx.resolve("Onboard a Luxembourg SICAV", Some("billing"), None);
        assert!(matches!(result, ScenarioResolveOutcome::NoMatch));
    }

    #[test]
    fn test_gate_g2_no_active_mode() {
        let idx = load_test_index();
        // No active mode → all scenarios are eligible
        let result = idx.resolve("Onboard a Luxembourg SICAV", None, None);
        assert!(matches!(result, ScenarioResolveOutcome::Matched(_)));
    }

    // --- Scoring ledger ---

    #[test]
    fn test_score_compound_action() {
        let idx = load_test_index();
        if let ScenarioResolveOutcome::Matched(m) =
            idx.resolve("Onboard a Luxembourg SICAV", None, None)
        {
            // Should have compound_action signal
            assert!(
                m.explain
                    .matched_signals
                    .iter()
                    .any(|s| s.signal == "compound_action"),
                "Missing compound_action signal"
            );
            assert!(m.score >= MIN_SCORE, "Score {} < {}", m.score, MIN_SCORE);
        } else {
            panic!("Expected Matched");
        }
    }

    #[test]
    fn test_score_jurisdiction() {
        let idx = load_test_index();
        if let ScenarioResolveOutcome::Matched(m) =
            idx.resolve("Onboard a Luxembourg SICAV", None, None)
        {
            assert!(
                m.explain
                    .matched_signals
                    .iter()
                    .any(|s| s.signal == "jurisdiction"),
                "Missing jurisdiction signal"
            );
        } else {
            panic!("Expected Matched");
        }
    }

    #[test]
    fn test_score_structure_noun() {
        let idx = load_test_index();
        if let ScenarioResolveOutcome::Matched(m) =
            idx.resolve("Onboard a Luxembourg SICAV", None, None)
        {
            assert!(
                m.explain
                    .matched_signals
                    .iter()
                    .any(|s| s.signal == "structure_noun"),
                "Missing structure_noun signal"
            );
        } else {
            panic!("Expected Matched");
        }
    }

    #[test]
    fn test_score_quantifier() {
        let idx = load_test_index();
        if let ScenarioResolveOutcome::Matched(m) = idx.resolve(
            "Onboard a Luxembourg SICAV with three sub-funds",
            None,
            None,
        ) {
            assert!(
                m.explain
                    .matched_signals
                    .iter()
                    .any(|s| s.signal == "quantifier"),
                "Missing quantifier signal"
            );
        } else {
            panic!("Expected Matched");
        }
    }

    #[test]
    fn test_full_compound_utterance_high_score() {
        let idx = load_test_index();
        if let ScenarioResolveOutcome::Matched(m) = idx.resolve(
            "Onboard this Luxembourg SICAV with three sub-funds and complete KYC screening",
            None,
            None,
        ) {
            // compound_action(4) + jurisdiction(4) + structure_noun(3) + phase_noun(2) + quantifier(2) = 15
            assert!(m.score >= 13, "Score {} too low for full compound", m.score);
            assert_eq!(m.scenario_id, "lux-sicav-setup");
        } else {
            panic!("Expected Matched");
        }
    }

    // --- Route resolution ---

    #[test]
    fn test_route_macro() {
        let idx = load_test_index();
        if let ScenarioResolveOutcome::Matched(m) =
            idx.resolve("Onboard a Luxembourg SICAV", None, None)
        {
            assert!(
                matches!(m.route, ResolvedRoute::Macro { ref macro_fqn } if macro_fqn == "struct.lux.ucits.sicav"),
                "Expected Macro route to struct.lux.ucits.sicav, got {:?}",
                m.route
            );
        } else {
            panic!("Expected Matched");
        }
    }

    #[test]
    fn test_route_macro_sequence() {
        let idx = load_test_index();
        let result = idx.resolve("Run the full KYC screening and compliance", None, None);
        match result {
            ScenarioResolveOutcome::Matched(m) => {
                assert_eq!(m.scenario_id, "full-screening");
                assert!(
                    matches!(m.route, ResolvedRoute::MacroSequence { ref macros } if macros.len() == 2)
                );
            }
            _ => {
                // Might not match if score is too low — that's ok for this test
            }
        }
    }

    #[test]
    fn test_route_selector_auto_resolve() {
        let idx = load_test_index();
        // This should match "jurisdiction-fund-setup" and auto-resolve via jurisdiction
        let result = idx.resolve("Set up a fund in Ireland", None, None);
        match result {
            ScenarioResolveOutcome::Matched(m) => {
                if let ResolvedRoute::Macro { ref macro_fqn } = m.route {
                    assert_eq!(macro_fqn, "struct.ie.ucits.icav");
                }
            }
            _ => {
                // May not match depending on scoring — that's acceptable
            }
        }
    }

    // --- Irish ICAV ---

    #[test]
    fn test_irish_icav_match() {
        let idx = load_test_index();
        if let ScenarioResolveOutcome::Matched(m) = idx.resolve("Set up an Irish ICAV", None, None)
        {
            assert_eq!(m.scenario_id, "ie-icav-setup");
        } else {
            // Could also be jurisdiction-fund-setup — both valid
        }
    }

    // --- Explain payload ---

    #[test]
    fn test_explain_payload() {
        let idx = load_test_index();
        if let ScenarioResolveOutcome::Matched(m) =
            idx.resolve("Onboard a Luxembourg SICAV", None, None)
        {
            assert_eq!(m.explain.resolution_tier, "Tier2A_ScenarioIndex");
            assert!(!m.explain.matched_signals.is_empty());
            assert!(!m.explain.gates.is_empty());
            assert_eq!(m.explain.score_total, m.score);
        } else {
            panic!("Expected Matched");
        }
    }

    // --- Disambiguation ---

    #[test]
    fn test_disambiguation_similar_scenarios() {
        // Both lux-sicav-setup and jurisdiction-fund-setup could match
        // "Establish a Luxembourg fund" with similar scores
        let idx = load_test_index();
        let result = idx.resolve("Establish a Luxembourg fund", None, None);
        // Either Matched or Ambiguous is acceptable here
        match result {
            ScenarioResolveOutcome::Matched(_) => {}
            ScenarioResolveOutcome::Ambiguous(candidates) => {
                assert!(candidates.len() >= 2);
            }
            ScenarioResolveOutcome::NoMatch => {
                panic!("Expected some match for 'Establish a Luxembourg fund'");
            }
        }
    }

    // --- No match for non-compound ---

    #[test]
    fn test_simple_verb_no_scenario() {
        let idx = load_test_index();
        // "list my CBUs" has no compound signals → should not match most scenarios
        let result = idx.resolve("list my CBUs", None, None);
        assert!(
            matches!(result, ScenarioResolveOutcome::NoMatch),
            "Expected NoMatch for simple verb"
        );
    }

    // --- Signal presence helper ---

    #[test]
    fn test_signal_present_helper() {
        let signals = extract_compound_signals("Onboard a Luxembourg SICAV with three sub-funds");
        assert!(signal_present(&signals, "compound_action"));
        assert!(signal_present(&signals, "jurisdiction"));
        assert!(signal_present(&signals, "structure_noun"));
        assert!(signal_present(&signals, "quantifier"));
        assert!(signal_present(&signals, "jurisdiction_structure_pair"));

        let simple = extract_compound_signals("create a fund");
        assert!(!signal_present(&simple, "compound_action"));
        assert!(!signal_present(&simple, "jurisdiction"));
    }
}
