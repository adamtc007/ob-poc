//! SageContext — the session context visible to the Sage.
//!
//! This is the only information the Sage receives from the orchestrator.
//! It contains NO verb FQNs, NO entity UUIDs from entity linking
//! (those come from Stage 3, which runs AFTER the Sage per E-SAGE-1).
//!
//! ## What SageContext may contain
//! - Session ID (for logging/tracing only)
//! - stage_focus (the active workflow — semos-kyc, semos-data-management, etc.)
//! - goals (SemReg context goals)
//! - entity_kind (the kind of entity currently in focus, if set before Sage)
//! - dominant_entity_name (the name mentioned in the current utterance, NOT UUID-resolved)
//! - last_intents (recent (plane, domain) pairs for carry-forward context)
//!
//! ## What SageContext must NOT contain (E-SAGE-2)
//! - Verb FQNs
//! - Verb scores or search results
//! - Entity UUIDs (those come from entity linking at Stage 3)
//! - SemOsContextEnvelope (that's post-Sem OS, also Stage 2)

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Minimal carry-forward record from recent Sage turns.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentIntent {
    /// Prior plane label.
    pub plane: String,
    /// Prior domain concept.
    pub domain_concept: String,
    /// Prior action label.
    pub action: String,
    /// Prior confidence label.
    pub confidence: String,
}

/// Session context visible to the Sage engine.
///
/// Constructed from `OrchestratorContext` in Stage 1.5, before entity linking.
#[derive(Debug, Clone, Default)]
pub struct SageContext {
    /// Session ID for logging and telemetry (not used for classification).
    pub session_id: Option<Uuid>,

    /// The active workflow stage focus (e.g., "semos-data-management", "semos-kyc").
    /// This is the primary plane classification signal.
    pub stage_focus: Option<String>,

    /// SemReg resolution goals (e.g., ["kyc", "data-management"]).
    pub goals: Vec<String>,

    /// The entity kind currently in focus (set before Sage from session state, NOT from entity linking).
    /// Example: "cbu", "deal", "entity". If set, this is an instance targeting signal.
    pub entity_kind: Option<String>,

    /// The dominant entity name from the current utterance (the raw text mention, NOT a UUID).
    /// Example: "Allianz", "the Lux SICAV". Used for domain hint extraction.
    pub dominant_entity_name: Option<String>,

    /// Recent intent records from the last N turns (for carry-forward).
    pub last_intents: Vec<RecentIntent>,
}

impl SageContext {
    /// Build a SageContext from the minimal fields available at Stage 1.5.
    pub fn from_stage_focus(stage_focus: Option<String>) -> Self {
        SageContext {
            stage_focus,
            ..Default::default()
        }
    }
}
