//! Unified Command Dispatch System
//!
//! ALL user inputs (voice, chat, egui clicks) flow through this single dispatcher.
//! This ensures consistent behavior regardless of input modality.
//!
//! # Two Categories of Commands
//!
//! 1. **NavigationVerb** - Local UI commands (zoom, pan, filter, view mode)
//!    - Handled immediately in the UI
//!    - No server round-trip required
//!
//! 2. **AgentPrompt** - Server-bound commands that need agent processing
//!    - Chat messages requiring LLM response
//!    - DSL execution requests
//!    - Investigation queries ("who controls X?")
//!    - ALL flow through the single `AgentPromptConduit`
//!
//! # Architecture
//!
//! ```text
//! Voice Transcript  ─┐                              ┌─► NavigationVerb ─► UI handles locally
//! Chat Message      ─┼─► CommandSource ─► dispatch ─┤
//! Egui Click        ─┘                              └─► AgentPrompt ────► AgentPromptConduit ─► Server
//! ```
//!
//! The `AgentPromptConduit` is the SINGLE entry point for all agent-bound commands.
//! This ensures consistent handling regardless of whether the command came from
//! voice, chat input, or a UI button click.

use ob_poc_graph::ViewMode;
use ob_poc_types::PanDirection;

// =============================================================================
// COMMAND SOURCE - Where the command originated
// =============================================================================

/// Unified command source - all inputs funnel through here
#[derive(Debug, Clone)]
pub enum CommandSource {
    /// Voice command from speech recognition
    Voice {
        transcript: String,
        confidence: f32,
        provider: VoiceProvider,
    },
    /// Chat message (may be parsed by agent or raw)
    Chat { message: String, agent_parsed: bool },
    /// Direct egui widget interaction
    Egui {
        widget_id: String,
        action: EguiAction,
    },
}

/// Voice provider that captured the transcript
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceProvider {
    Deepgram,
    WebSpeech,
    Unknown,
}

impl VoiceProvider {
    /// Parse provider name from string (case-insensitive)
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "deepgram" => VoiceProvider::Deepgram,
            "webspeech" | "web_speech" | "web-speech" => VoiceProvider::WebSpeech,
            _ => VoiceProvider::Unknown,
        }
    }
}

/// Actions from egui widgets
#[derive(Debug, Clone)]
pub enum EguiAction {
    Click,
    DoubleClick,
    Drag { delta_x: f32, delta_y: f32 },
    Scroll { delta: f32 },
    KeyPress { key: String, modifiers: Modifiers },
    Select { value: String },
}

/// Keyboard modifiers
#[derive(Debug, Clone, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub command: bool,
}

// =============================================================================
// NAVIGATION VERB - The canonical command representation
// =============================================================================

/// Unified navigation verb - the result of parsing any input source
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationVerb {
    // =========================================================================
    // Basic Navigation
    // =========================================================================
    ZoomIn {
        factor: Option<f32>,
    },
    ZoomOut {
        factor: Option<f32>,
    },
    ZoomFit,
    ZoomTo {
        level: f32,
    },
    Pan {
        direction: PanDirection,
        amount: Option<f32>,
    },
    Center,
    Stop,
    FocusEntity {
        entity_id: String,
    },
    ResetLayout,

    // =========================================================================
    // View Mode
    // =========================================================================
    SetViewMode {
        mode: ViewMode,
    },

    // =========================================================================
    // Type Filtering
    // =========================================================================
    FilterByType {
        type_codes: Vec<String>,
    },
    HighlightType {
        type_code: String,
    },
    ClearFilter,

    // =========================================================================
    // Scale Navigation (Astronomical Metaphor)
    // =========================================================================
    ScaleUniverse,
    /// View all CBUs for a commercial client (the "galaxy" / book view)
    /// Maps to `view.book :client <id>` DSL verb
    ScaleBook {
        client_name: String,
    },
    ScaleGalaxy {
        segment: Option<String>,
    },
    ScaleSystem {
        cbu_id: Option<String>,
    },
    ScalePlanet {
        entity_id: Option<String>,
    },
    ScaleSurface,
    ScaleCore,

    // =========================================================================
    // Depth Navigation (Z-Axis)
    // =========================================================================
    DrillThrough,
    SurfaceReturn,
    Xray,
    Peel,
    CrossSection,
    DepthIndicator,

    // =========================================================================
    // Orbital Navigation
    // =========================================================================
    Orbit {
        entity_id: Option<String>,
    },
    RotateLayer {
        layer: String,
    },
    Flip,
    Tilt {
        dimension: String,
    },

    // =========================================================================
    // Temporal Navigation
    // =========================================================================
    TimeRewind {
        target_date: Option<String>,
    },
    TimePlay {
        from: Option<String>,
        to: Option<String>,
    },
    TimeFreeze,
    TimeSlice {
        date1: Option<String>,
        date2: Option<String>,
    },
    TimeTrail {
        entity_id: Option<String>,
    },

    // =========================================================================
    // Investigation Patterns (Matrix-themed)
    // =========================================================================
    /// "Follow the white rabbit" - trace ownership chain to terminus (find the humans)
    FollowRabbit {
        from_entity: Option<String>,
    },
    /// "Dive into" - explore/examine an entity's structure
    DiveInto {
        entity_id: Option<String>,
    },
    WhoControls {
        entity_id: Option<String>,
    },
    Illuminate {
        aspect: String,
    },
    Shadow,
    RedFlagScan,
    /// "Show me the black holes" - find data gaps/opacity
    BlackHole,

    // =========================================================================
    // Context Intentions
    // =========================================================================
    SetContext {
        context: String,
    },

    // =========================================================================
    // No-op (input not recognized as command)
    // =========================================================================
    None,
}

// =============================================================================
// AGENT PROMPT - Commands that need server/agent processing
// =============================================================================

/// Agent-bound prompts - ALL of these flow through the AgentPromptConduit.
///
/// These are NOT handled locally in the UI. They require a server round-trip
/// and potentially LLM processing.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentPrompt {
    /// Free-form chat message requiring agent response
    Chat {
        message: String,
        /// Original source for attribution/logging
        source: PromptSource,
    },

    /// Execute DSL directly (may have been generated by agent or user)
    ExecuteDsl {
        dsl: String,
        /// If true, show preview before executing
        preview: bool,
        source: PromptSource,
    },

    /// Investigation query - agent determines how to answer
    /// Examples: "who controls X?", "follow the money from Y"
    Investigate {
        query: String,
        /// Entity context for the investigation
        context_entity_id: Option<String>,
        source: PromptSource,
    },

    /// Request document or information
    RequestInfo {
        request_type: InfoRequestType,
        entity_id: Option<String>,
        source: PromptSource,
    },

    /// Trigger a workflow action
    WorkflowAction {
        action: String,
        params: std::collections::HashMap<String, String>,
        source: PromptSource,
    },
}

/// Where the agent prompt originated (for logging/attribution)
#[derive(Debug, Clone, PartialEq)]
pub enum PromptSource {
    /// Voice command with confidence and provider
    Voice {
        transcript: String,
        confidence: f32,
        provider: VoiceProvider,
    },
    /// Chat input from text field
    ChatInput,
    /// Egui button/widget click
    EguiWidget { widget_id: String },
    /// System-generated (e.g., workflow trigger)
    System,
}

/// Types of information requests
#[derive(Debug, Clone, PartialEq)]
pub enum InfoRequestType {
    UboChain,
    OwnershipStructure,
    DocumentList,
    ScreeningStatus,
    KycProgress,
    RiskAssessment,
    Custom(String),
}

// =============================================================================
// COMMAND RESULT - The output of dispatch
// =============================================================================

/// Result of dispatching a command - either local navigation or agent-bound
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// Handle locally in the UI (no server round-trip)
    Navigation(NavigationVerb),
    /// Send to server via AgentPromptConduit
    Agent(AgentPrompt),
    /// No action needed (unrecognized input or below confidence threshold)
    None,
}

impl CommandResult {
    /// Check if this is a navigation command
    pub fn is_navigation(&self) -> bool {
        matches!(self, CommandResult::Navigation(_))
    }

    /// Check if this is an agent-bound command
    pub fn is_agent(&self) -> bool {
        matches!(self, CommandResult::Agent(_))
    }

    /// Check if this is a no-op
    pub fn is_none(&self) -> bool {
        matches!(self, CommandResult::None)
    }

    /// Extract navigation verb if present
    pub fn as_navigation(&self) -> Option<&NavigationVerb> {
        match self {
            CommandResult::Navigation(verb) => Some(verb),
            _ => None,
        }
    }

    /// Extract agent prompt if present
    pub fn as_agent(&self) -> Option<&AgentPrompt> {
        match self {
            CommandResult::Agent(prompt) => Some(prompt),
            _ => None,
        }
    }
}

// =============================================================================
// AGENT PROMPT CONDUIT - The single entry point for all agent-bound commands
// =============================================================================

/// The single conduit through which ALL agent-bound commands flow.
///
/// This trait is implemented by the App to handle agent prompts uniformly,
/// regardless of their source (voice, chat, egui click).
///
/// # Design Rationale
///
/// By funneling all agent commands through one conduit:
/// 1. Consistent logging/telemetry across all input modalities
/// 2. Single point for rate limiting, queueing, retry logic
/// 3. Uniform error handling and user feedback
/// 4. Easy to add new input sources without changing agent integration
///
/// # Usage
///
/// ```ignore
/// // In App, after dispatching a command:
/// match dispatch_command(source, &ctx) {
///     CommandResult::Navigation(verb) => self.execute_navigation_verb(verb),
///     CommandResult::Agent(prompt) => self.send_to_agent(prompt),
///     CommandResult::None => {},
/// }
/// ```
pub trait AgentPromptConduit {
    /// Send an agent prompt for processing.
    ///
    /// This is the SINGLE entry point for all agent-bound commands.
    /// Implementations should:
    /// 1. Log the prompt with source attribution
    /// 2. Queue if necessary (e.g., if another request is in flight)
    /// 3. Send to server via appropriate API
    /// 4. Handle response and update UI
    fn send_to_agent(&mut self, prompt: AgentPrompt);

    /// Check if the conduit is ready to accept new prompts.
    /// Returns false if a request is currently in flight.
    fn is_ready(&self) -> bool;

    /// Get the last error from the conduit, if any.
    fn last_error(&self) -> Option<&str>;
}

/// Helper to convert AgentPrompt to a chat message string for the API.
impl AgentPrompt {
    /// Convert to a message string suitable for the chat API.
    pub fn to_chat_message(&self) -> String {
        match self {
            AgentPrompt::Chat { message, .. } => message.clone(),
            AgentPrompt::ExecuteDsl { dsl, preview, .. } => {
                if *preview {
                    format!("Preview DSL: {}", dsl)
                } else {
                    format!("Execute: {}", dsl)
                }
            }
            AgentPrompt::Investigate {
                query,
                context_entity_id,
                ..
            } => {
                if let Some(entity_id) = context_entity_id {
                    format!("{} (context: {})", query, entity_id)
                } else {
                    query.clone()
                }
            }
            AgentPrompt::RequestInfo {
                request_type,
                entity_id,
                ..
            } => {
                let type_str = match request_type {
                    InfoRequestType::UboChain => "Show UBO chain",
                    InfoRequestType::OwnershipStructure => "Show ownership structure",
                    InfoRequestType::DocumentList => "List documents",
                    InfoRequestType::ScreeningStatus => "Show screening status",
                    InfoRequestType::KycProgress => "Show KYC progress",
                    InfoRequestType::RiskAssessment => "Show risk assessment",
                    InfoRequestType::Custom(s) => s.as_str(),
                };
                if let Some(entity_id) = entity_id {
                    format!("{} for {}", type_str, entity_id)
                } else {
                    type_str.to_string()
                }
            }
            AgentPrompt::WorkflowAction { action, params, .. } => {
                if params.is_empty() {
                    action.clone()
                } else {
                    format!("{} with {:?}", action, params)
                }
            }
        }
    }

    /// Get the source of this prompt.
    pub fn source(&self) -> &PromptSource {
        match self {
            AgentPrompt::Chat { source, .. } => source,
            AgentPrompt::ExecuteDsl { source, .. } => source,
            AgentPrompt::Investigate { source, .. } => source,
            AgentPrompt::RequestInfo { source, .. } => source,
            AgentPrompt::WorkflowAction { source, .. } => source,
        }
    }

    /// Check if this prompt came from voice input.
    pub fn is_voice(&self) -> bool {
        matches!(self.source(), PromptSource::Voice { .. })
    }

    /// Get voice confidence if this was a voice command.
    pub fn voice_confidence(&self) -> Option<f32> {
        match self.source() {
            PromptSource::Voice { confidence, .. } => Some(*confidence),
            _ => None,
        }
    }
}

// =============================================================================
// INVESTIGATION CONTEXT - State available during command dispatch
// =============================================================================

/// Context available when dispatching commands
#[derive(Debug, Clone, Default)]
pub struct InvestigationContext {
    /// Currently focused entity
    pub focused_entity_id: Option<String>,
    /// Current CBU ID
    pub current_cbu_id: Option<String>,
    /// Current view mode
    pub current_view_mode: ViewMode,
    /// Current zoom level
    pub current_zoom: f32,
    /// Selected entity IDs
    pub selected_entities: Vec<String>,
}

// =============================================================================
// COMMAND DISPATCHER - Central routing
// =============================================================================

/// Dispatch a command from any source to a canonical verb
/// Dispatch a command from any source to either a NavigationVerb or AgentPrompt.
///
/// This is the CENTRAL routing function. All inputs flow through here.
/// Returns:
/// - `CommandResult::Navigation` for local UI commands
/// - `CommandResult::Agent` for server-bound commands (via AgentPromptConduit)
/// - `CommandResult::None` for unrecognized or low-confidence input
pub fn dispatch_command(source: CommandSource, ctx: &InvestigationContext) -> CommandResult {
    match source {
        CommandSource::Voice {
            ref transcript,
            confidence,
            ref provider,
        } => {
            // Only process if confidence is above threshold
            if confidence < 0.5 {
                return CommandResult::None;
            }

            // FIRST: Check if it's an investigation query (agent-bound)
            // These take priority because they need deep agent analysis
            if let Some(prompt) =
                try_match_investigation_query(transcript, ctx, *provider, confidence)
            {
                return CommandResult::Agent(prompt);
            }

            // SECOND: Try to match as a local navigation command
            let nav_verb = match_verb_from_transcript(transcript, ctx);
            if nav_verb != NavigationVerb::None {
                return CommandResult::Navigation(nav_verb);
            }

            // Default: treat as chat message to agent
            CommandResult::Agent(AgentPrompt::Chat {
                message: transcript.clone(),
                source: PromptSource::Voice {
                    transcript: transcript.clone(),
                    confidence,
                    provider: *provider,
                },
            })
        }
        CommandSource::Chat {
            ref message,
            agent_parsed,
        } => {
            if agent_parsed {
                // Agent already parsed - extract verb from structured response
                let nav_verb = parse_agent_verb(message);
                if nav_verb != NavigationVerb::None {
                    return CommandResult::Navigation(nav_verb);
                }
                CommandResult::None
            } else {
                // FIRST: Check if it's an investigation query (agent-bound)
                // These take priority because they need deep agent analysis
                if let Some(prompt) =
                    try_match_investigation_query(message, ctx, VoiceProvider::Unknown, 1.0)
                {
                    return CommandResult::Agent(prompt);
                }

                // SECOND: Try to match as a local navigation command
                let nav_verb = match_verb_from_transcript(message, ctx);
                if nav_verb != NavigationVerb::None {
                    return CommandResult::Navigation(nav_verb);
                }

                // Default: send to agent as chat
                CommandResult::Agent(AgentPrompt::Chat {
                    message: message.clone(),
                    source: PromptSource::ChatInput,
                })
            }
        }
        CommandSource::Egui {
            ref action,
            widget_id: _,
        } => {
            let nav_verb = map_egui_action_to_verb(action, ctx);
            if nav_verb != NavigationVerb::None {
                CommandResult::Navigation(nav_verb)
            } else {
                CommandResult::None
            }
        }
    }
}

/// Try to match an investigation query that should go to the agent.
/// Returns Some(AgentPrompt) if matched, None otherwise.
fn try_match_investigation_query(
    text: &str,
    ctx: &InvestigationContext,
    provider: VoiceProvider,
    confidence: f32,
) -> Option<AgentPrompt> {
    let text_lower = text.to_lowercase();

    let source = if provider != VoiceProvider::Unknown {
        PromptSource::Voice {
            transcript: text.to_string(),
            confidence,
            provider,
        }
    } else {
        PromptSource::ChatInput
    };

    // =========================================================================
    // Investigation queries that need agent processing
    // Matrix-themed vocabulary: "follow the white rabbit" = trace to terminus
    // =========================================================================

    // Control chain investigation
    if text_lower.contains("who controls") || text_lower.contains("ultimate controller") {
        return Some(AgentPrompt::Investigate {
            query: text.to_string(),
            context_entity_id: ctx.focused_entity_id.clone(),
            source,
        });
    }

    // Matrix-themed: Follow the white rabbit = trace ownership to terminus (find the humans)
    // Also supports legacy "follow the money" for backwards compatibility
    if text_lower.contains("follow the rabbit")
        || text_lower.contains("white rabbit")
        || text_lower.contains("rabbit hole")
        || text_lower.contains("how deep does this go")
        || text_lower.contains("how far down")
        || text_lower.contains("trace to terminus")
        || text_lower.contains("find the humans")
        || text_lower.contains("follow the money")
        || text_lower.contains("trace funds")
    {
        return Some(AgentPrompt::Investigate {
            query: text.to_string(),
            context_entity_id: ctx.focused_entity_id.clone(),
            source,
        });
    }

    // Dive into = explore/examine an entity's structure (exploration-focused)
    if text_lower.contains("dive into")
        || text_lower.contains("dive in")
        || text_lower.contains("deep dive")
        || text_lower.contains("go deep")
        || text_lower.contains("dig into")
        || text_lower.contains("explore this")
        || text_lower.contains("examine closely")
    {
        return Some(AgentPrompt::Investigate {
            query: text.to_string(),
            context_entity_id: ctx.focused_entity_id.clone(),
            source,
        });
    }

    if text_lower.contains("show ubo") || text_lower.contains("beneficial owner") {
        return Some(AgentPrompt::RequestInfo {
            request_type: InfoRequestType::UboChain,
            entity_id: ctx.focused_entity_id.clone(),
            source,
        });
    }

    if text_lower.contains("ownership structure") || text_lower.contains("ownership chain") {
        return Some(AgentPrompt::RequestInfo {
            request_type: InfoRequestType::OwnershipStructure,
            entity_id: ctx.focused_entity_id.clone(),
            source,
        });
    }

    if text_lower.contains("kyc status") || text_lower.contains("kyc progress") {
        return Some(AgentPrompt::RequestInfo {
            request_type: InfoRequestType::KycProgress,
            entity_id: ctx.focused_entity_id.clone(),
            source,
        });
    }

    if text_lower.contains("risk")
        && (text_lower.contains("assessment") || text_lower.contains("rating"))
    {
        return Some(AgentPrompt::RequestInfo {
            request_type: InfoRequestType::RiskAssessment,
            entity_id: ctx.focused_entity_id.clone(),
            source,
        });
    }

    if text_lower.starts_with("execute") || text_lower.starts_with("run dsl") {
        // Extract DSL after the command
        let dsl = if text_lower.starts_with("execute ") {
            text[8..].trim().to_string()
        } else if text_lower.starts_with("run dsl ") {
            text[8..].trim().to_string()
        } else {
            text.to_string()
        };

        if !dsl.is_empty() {
            return Some(AgentPrompt::ExecuteDsl {
                dsl,
                preview: false,
                source,
            });
        }
    }

    None
}

/// Match a transcript (voice or chat) to a navigation verb
fn match_verb_from_transcript(transcript: &str, ctx: &InvestigationContext) -> NavigationVerb {
    let text = transcript.to_lowercase().trim().to_string();

    // =========================================================================
    // Scale Navigation (Universe/Galaxy/System/Planet)
    // =========================================================================
    if matches_any(
        &text,
        &[
            "universe",
            "full book",
            "god view",
            "show all",
            "everything",
            "bird's eye",
        ],
    ) {
        return NavigationVerb::ScaleUniverse;
    }

    // Book view - all CBUs for a commercial client (the "galaxy" / book view)
    // "book allianz", "client allianz", "show book blackrock", "allianz book"
    if let Some(client) = extract_after_any(&text, &["book", "client book", "show book"]) {
        if !client.is_empty() {
            return NavigationVerb::ScaleBook {
                client_name: client,
            };
        }
    }
    // Also match "allianz book" pattern (client name first)
    if text.ends_with(" book") || text.ends_with(" books") {
        let client = text
            .trim_end_matches(" books")
            .trim_end_matches(" book")
            .trim();
        if !client.is_empty() {
            return NavigationVerb::ScaleBook {
                client_name: client.to_string(),
            };
        }
    }

    if let Some(segment) = extract_after_any(&text, &["galaxy", "segment", "cluster", "sector"]) {
        return NavigationVerb::ScaleGalaxy {
            segment: if segment.is_empty() {
                None
            } else {
                Some(segment)
            },
        };
    }

    if let Some(cbu) = extract_after_any(&text, &["system", "focus cbu", "client unit", "cbu"]) {
        return NavigationVerb::ScaleSystem {
            cbu_id: if cbu.is_empty() {
                ctx.current_cbu_id.clone()
            } else {
                Some(cbu)
            },
        };
    }

    if let Some(entity) = extract_after_any(&text, &["planet", "focus entity", "entity", "node"]) {
        return NavigationVerb::ScalePlanet {
            entity_id: if entity.is_empty() {
                ctx.focused_entity_id.clone()
            } else {
                Some(entity)
            },
        };
    }

    if matches_any(&text, &["surface", "surface view", "top level"]) {
        return NavigationVerb::ScaleSurface;
    }

    if matches_any(&text, &["core", "deep dive", "innermost"]) {
        return NavigationVerb::ScaleCore;
    }

    // =========================================================================
    // Depth Navigation (Z-Axis)
    // =========================================================================
    if matches_any(
        &text,
        &["drill", "drill through", "dig deeper", "go deeper"],
    ) {
        return NavigationVerb::DrillThrough;
    }

    if matches_any(
        &text,
        &["surface return", "back to surface", "emerge", "rise up"],
    ) {
        return NavigationVerb::SurfaceReturn;
    }

    if matches_any(&text, &["x-ray", "xray", "see through", "transparency"]) {
        return NavigationVerb::Xray;
    }

    if matches_any(
        &text,
        &["peel", "peel layer", "strip layer", "remove layer"],
    ) {
        return NavigationVerb::Peel;
    }

    if matches_any(
        &text,
        &["cross section", "cross-section", "slice", "cut through"],
    ) {
        return NavigationVerb::CrossSection;
    }

    if matches_any(&text, &["depth indicator", "show depth", "depth gauge"]) {
        return NavigationVerb::DepthIndicator;
    }

    // =========================================================================
    // Orbital Navigation
    // =========================================================================
    if let Some(entity) = extract_after_any(&text, &["orbit", "circle around", "revolve"]) {
        return NavigationVerb::Orbit {
            entity_id: if entity.is_empty() {
                ctx.focused_entity_id.clone()
            } else {
                Some(entity)
            },
        };
    }

    if let Some(layer) = extract_after_any(&text, &["rotate layer", "spin layer"]) {
        return NavigationVerb::RotateLayer {
            layer: if layer.is_empty() {
                "default".to_string()
            } else {
                layer
            },
        };
    }

    if matches_any(&text, &["flip", "flip view", "invert", "mirror"]) {
        return NavigationVerb::Flip;
    }

    if let Some(dim) = extract_after_any(&text, &["tilt", "angle", "perspective"]) {
        return NavigationVerb::Tilt {
            dimension: if dim.is_empty() { "x".to_string() } else { dim },
        };
    }

    // =========================================================================
    // Temporal Navigation
    // =========================================================================
    if let Some(date) =
        extract_after_any(&text, &["rewind", "go back to", "time travel to", "as of"])
    {
        return NavigationVerb::TimeRewind {
            target_date: if date.is_empty() { None } else { Some(date) },
        };
    }

    if matches_any(&text, &["play", "animate", "show history", "timeline"]) {
        // Could extract from/to dates if present
        return NavigationVerb::TimePlay {
            from: None,
            to: None,
        };
    }

    if matches_any(&text, &["freeze", "pause", "stop time", "halt"]) {
        return NavigationVerb::TimeFreeze;
    }

    if matches_any(&text, &["compare", "diff", "time slice", "before after"]) {
        return NavigationVerb::TimeSlice {
            date1: None,
            date2: None,
        };
    }

    if let Some(entity) = extract_after_any(&text, &["trail", "trace history", "evolution of"]) {
        return NavigationVerb::TimeTrail {
            entity_id: if entity.is_empty() {
                ctx.focused_entity_id.clone()
            } else {
                Some(entity)
            },
        };
    }

    // =========================================================================
    // Investigation Patterns (Matrix-themed)
    // =========================================================================

    // Follow the white rabbit = trace ownership chain to terminus
    if let Some(entity) = extract_after_any(
        &text,
        &[
            "follow the rabbit",
            "follow the white rabbit",
            "white rabbit",
            "rabbit hole",
            "down the rabbit hole",
            "follow the money", // Legacy support
            "trace funds",
            "trace to terminus",
            "find the humans",
        ],
    ) {
        return NavigationVerb::FollowRabbit {
            from_entity: if entity.is_empty() {
                ctx.focused_entity_id.clone()
            } else {
                Some(entity)
            },
        };
    }

    // Dive into = explore/examine an entity's structure
    if let Some(entity) = extract_after_any(
        &text,
        &[
            "dive into",
            "dive in",
            "deep dive",
            "go deep",
            "dig into",
            "explore",
            "examine closely",
        ],
    ) {
        return NavigationVerb::DiveInto {
            entity_id: if entity.is_empty() {
                ctx.focused_entity_id.clone()
            } else {
                Some(entity)
            },
        };
    }

    if let Some(entity) = extract_after_any(
        &text,
        &[
            "who controls",
            "control chain",
            "controlling party",
            "ultimate controller",
        ],
    ) {
        return NavigationVerb::WhoControls {
            entity_id: if entity.is_empty() {
                ctx.focused_entity_id.clone()
            } else {
                Some(entity)
            },
        };
    }

    if let Some(aspect) =
        extract_after_any(&text, &["illuminate", "highlight", "show me", "emphasize"])
    {
        return NavigationVerb::Illuminate {
            aspect: if aspect.is_empty() {
                "ownership".to_string()
            } else {
                aspect
            },
        };
    }

    if matches_any(
        &text,
        &["shadow", "dim others", "fade background", "isolate"],
    ) {
        return NavigationVerb::Shadow;
    }

    if matches_any(
        &text,
        &["red flag", "scan for issues", "risk scan", "problems"],
    ) {
        return NavigationVerb::RedFlagScan;
    }

    if matches_any(&text, &["black hole", "missing data", "gaps", "unknowns"]) {
        return NavigationVerb::BlackHole;
    }

    // =========================================================================
    // Context Intentions
    // =========================================================================
    if let Some(context) = extract_after_any(&text, &["context", "mode", "switch to"]) {
        if matches_any(
            &context,
            &[
                "review",
                "investigation",
                "onboarding",
                "audit",
                "compliance",
            ],
        ) {
            return NavigationVerb::SetContext { context };
        }
    }

    // =========================================================================
    // Basic Navigation
    // =========================================================================
    if matches_any(&text, &["zoom in", "closer", "magnify", "enlarge"]) {
        let factor = extract_number(&text).map(|n| n / 100.0);
        return NavigationVerb::ZoomIn { factor };
    }

    if matches_any(&text, &["zoom out", "farther", "shrink", "reduce"]) {
        let factor = extract_number(&text).map(|n| n / 100.0);
        return NavigationVerb::ZoomOut { factor };
    }

    if matches_any(
        &text,
        &["fit", "zoom fit", "fit all", "show all", "fit to screen"],
    ) {
        return NavigationVerb::ZoomFit;
    }

    if matches_any(&text, &["center", "recenter", "center view"]) {
        return NavigationVerb::Center;
    }

    if matches_any(&text, &["stop", "halt", "freeze animation"]) {
        return NavigationVerb::Stop;
    }

    if matches_any(&text, &["reset", "reset layout", "default layout"]) {
        return NavigationVerb::ResetLayout;
    }

    // Pan directions
    if matches_any(&text, &["pan left", "move left", "left"]) {
        return NavigationVerb::Pan {
            direction: PanDirection::Left,
            amount: extract_number(&text),
        };
    }
    if matches_any(&text, &["pan right", "move right", "right"]) {
        return NavigationVerb::Pan {
            direction: PanDirection::Right,
            amount: extract_number(&text),
        };
    }
    if matches_any(&text, &["pan up", "move up", "up"]) {
        return NavigationVerb::Pan {
            direction: PanDirection::Up,
            amount: extract_number(&text),
        };
    }
    if matches_any(&text, &["pan down", "move down", "down"]) {
        return NavigationVerb::Pan {
            direction: PanDirection::Down,
            amount: extract_number(&text),
        };
    }

    // =========================================================================
    // View Mode
    // =========================================================================
    if matches_any(&text, &["kyc view", "kyc ubo", "ownership view", "ubo"]) {
        return NavigationVerb::SetViewMode {
            mode: ViewMode::KycUbo,
        };
    }
    if matches_any(&text, &["service view", "services", "service delivery"]) {
        return NavigationVerb::SetViewMode {
            mode: ViewMode::ServiceDelivery,
        };
    }
    if matches_any(&text, &["products view", "products only", "products"]) {
        return NavigationVerb::SetViewMode {
            mode: ViewMode::ProductsOnly,
        };
    }
    if matches_any(&text, &["trading view", "trading", "trading matrix"]) {
        return NavigationVerb::SetViewMode {
            mode: ViewMode::Trading,
        };
    }

    // =========================================================================
    // Type Filtering (Enhance pattern from Blade Runner)
    // =========================================================================
    if text.starts_with("enhance") || text.starts_with("show") || text.starts_with("filter") {
        // Extract entity types mentioned
        let mut types = Vec::new();
        if text.contains("person") || text.contains("people") {
            types.push("PROPER_PERSON".to_string());
        }
        if text.contains("company") || text.contains("companies") {
            types.push("LIMITED_COMPANY".to_string());
        }
        if text.contains("fund") || text.contains("funds") {
            types.push("FUND".to_string());
        }
        if text.contains("trust") || text.contains("trusts") {
            types.push("TRUST_DISCRETIONARY".to_string());
        }
        if text.contains("director") || text.contains("directors") {
            types.push("DIRECTOR".to_string());
        }
        if text.contains("ubo") || text.contains("owner") {
            types.push("UBO".to_string());
        }

        if !types.is_empty() {
            return NavigationVerb::FilterByType { type_codes: types };
        }
    }

    if matches_any(
        &text,
        &["clear filter", "show all types", "reset filter", "unfilter"],
    ) {
        return NavigationVerb::ClearFilter;
    }

    // Not recognized as a navigation command
    NavigationVerb::None
}

/// Parse agent-structured verb response
fn parse_agent_verb(message: &str) -> NavigationVerb {
    // Agent responses should already be structured
    // This is a fallback for plain text agent responses
    match_verb_from_transcript(message, &InvestigationContext::default())
}

/// Map egui widget action to navigation verb
fn map_egui_action_to_verb(action: &EguiAction, ctx: &InvestigationContext) -> NavigationVerb {
    match action {
        EguiAction::Scroll { delta } => {
            if *delta > 0.0 {
                NavigationVerb::ZoomIn {
                    factor: Some(delta.abs() / 100.0),
                }
            } else {
                NavigationVerb::ZoomOut {
                    factor: Some(delta.abs() / 100.0),
                }
            }
        }
        EguiAction::DoubleClick => {
            // Double-click on entity focuses it
            if let Some(ref entity_id) = ctx.focused_entity_id {
                NavigationVerb::FocusEntity {
                    entity_id: entity_id.clone(),
                }
            } else {
                NavigationVerb::ZoomFit
            }
        }
        EguiAction::KeyPress { key, modifiers } => match key.as_str() {
            "+" | "=" if modifiers.ctrl || modifiers.command => {
                NavigationVerb::ZoomIn { factor: None }
            }
            "-" | "_" if modifiers.ctrl || modifiers.command => {
                NavigationVerb::ZoomOut { factor: None }
            }
            "0" if modifiers.ctrl || modifiers.command => NavigationVerb::ZoomFit,
            "ArrowLeft" => NavigationVerb::Pan {
                direction: PanDirection::Left,
                amount: None,
            },
            "ArrowRight" => NavigationVerb::Pan {
                direction: PanDirection::Right,
                amount: None,
            },
            "ArrowUp" => NavigationVerb::Pan {
                direction: PanDirection::Up,
                amount: None,
            },
            "ArrowDown" => NavigationVerb::Pan {
                direction: PanDirection::Down,
                amount: None,
            },
            "Escape" => NavigationVerb::Stop,
            "r" if modifiers.ctrl || modifiers.command => NavigationVerb::ResetLayout,
            "1" => NavigationVerb::SetViewMode {
                mode: ViewMode::KycUbo,
            },
            "2" => NavigationVerb::SetViewMode {
                mode: ViewMode::ServiceDelivery,
            },
            "3" => NavigationVerb::SetViewMode {
                mode: ViewMode::ProductsOnly,
            },
            "4" => NavigationVerb::SetViewMode {
                mode: ViewMode::Trading,
            },
            _ => NavigationVerb::None,
        },
        _ => NavigationVerb::None,
    }
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Check if text matches any of the trigger phrases
fn matches_any(text: &str, triggers: &[&str]) -> bool {
    triggers.iter().any(|t| text.contains(t))
}

/// Extract text after any of the trigger phrases
fn extract_after_any(text: &str, triggers: &[&str]) -> Option<String> {
    for trigger in triggers {
        if let Some(pos) = text.find(trigger) {
            let after = text[pos + trigger.len()..].trim().to_string();
            return Some(after);
        }
    }
    None
}

/// Extract a number from the text
fn extract_number(text: &str) -> Option<f32> {
    // Find first sequence of digits
    let mut num_str = String::new();
    let mut in_number = false;

    for c in text.chars() {
        if c.is_ascii_digit() || (c == '.' && in_number) {
            num_str.push(c);
            in_number = true;
        } else if in_number {
            break;
        }
    }

    num_str.parse().ok()
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_command_zoom() {
        let ctx = InvestigationContext::default();
        let source = CommandSource::Voice {
            transcript: "zoom in".to_string(),
            confidence: 0.9,
            provider: VoiceProvider::Deepgram,
        };
        let result = dispatch_command(source, &ctx);
        assert!(matches!(
            result,
            CommandResult::Navigation(NavigationVerb::ZoomIn { .. })
        ));
    }

    #[test]
    fn test_voice_command_follow_money() {
        // "follow the money" (legacy) is now an agent-bound investigation query
        let ctx = InvestigationContext {
            focused_entity_id: Some("entity-123".to_string()),
            ..Default::default()
        };
        let source = CommandSource::Voice {
            transcript: "follow the money".to_string(),
            confidence: 0.85,
            provider: VoiceProvider::WebSpeech,
        };
        let result = dispatch_command(source, &ctx);
        // This is now an AgentPrompt::Investigate, not NavigationVerb
        assert!(matches!(
            result,
            CommandResult::Agent(AgentPrompt::Investigate { .. })
        ));
    }

    #[test]
    fn test_voice_command_follow_rabbit() {
        // Matrix-themed: "follow the white rabbit" traces to terminus
        let ctx = InvestigationContext {
            focused_entity_id: Some("entity-123".to_string()),
            ..Default::default()
        };
        let source = CommandSource::Voice {
            transcript: "follow the white rabbit".to_string(),
            confidence: 0.9,
            provider: VoiceProvider::Deepgram,
        };
        let result = dispatch_command(source, &ctx);
        assert!(matches!(
            result,
            CommandResult::Agent(AgentPrompt::Investigate { .. })
        ));
    }

    #[test]
    fn test_voice_command_dive_into() {
        // "Dive into" is exploration-focused investigation
        let ctx = InvestigationContext {
            focused_entity_id: Some("entity-456".to_string()),
            ..Default::default()
        };
        let source = CommandSource::Voice {
            transcript: "dive into this entity".to_string(),
            confidence: 0.88,
            provider: VoiceProvider::Deepgram,
        };
        let result = dispatch_command(source, &ctx);
        assert!(matches!(
            result,
            CommandResult::Agent(AgentPrompt::Investigate { .. })
        ));
    }

    #[test]
    fn test_low_confidence_ignored() {
        let ctx = InvestigationContext::default();
        let source = CommandSource::Voice {
            transcript: "zoom in".to_string(),
            confidence: 0.3, // Below threshold
            provider: VoiceProvider::Deepgram,
        };
        let result = dispatch_command(source, &ctx);
        assert!(matches!(result, CommandResult::None));
    }

    #[test]
    fn test_egui_scroll() {
        let ctx = InvestigationContext::default();
        let source = CommandSource::Egui {
            widget_id: "graph".to_string(),
            action: EguiAction::Scroll { delta: 50.0 },
        };
        let result = dispatch_command(source, &ctx);
        assert!(matches!(
            result,
            CommandResult::Navigation(NavigationVerb::ZoomIn { .. })
        ));
    }

    #[test]
    fn test_enhance_filter() {
        let ctx = InvestigationContext::default();
        let source = CommandSource::Chat {
            message: "enhance and show only people".to_string(),
            agent_parsed: false,
        };
        let result = dispatch_command(source, &ctx);
        assert!(matches!(
            result,
            CommandResult::Navigation(NavigationVerb::FilterByType { ref type_codes })
                if type_codes.contains(&"PROPER_PERSON".to_string())
        ));
    }

    #[test]
    fn test_chat_goes_to_agent() {
        // Unrecognized chat messages go to agent as Chat
        // Note: messages matching investigation patterns go as RequestInfo/Investigate
        let ctx = InvestigationContext::default();
        let source = CommandSource::Chat {
            message: "hello, can you help me?".to_string(),
            agent_parsed: false,
        };
        let result = dispatch_command(source, &ctx);
        assert!(matches!(
            result,
            CommandResult::Agent(AgentPrompt::Chat { .. })
        ));
    }

    #[test]
    fn test_investigation_query_to_agent() {
        // Investigation queries go to agent
        let ctx = InvestigationContext {
            focused_entity_id: Some("entity-456".to_string()),
            ..Default::default()
        };
        let source = CommandSource::Chat {
            message: "who controls this entity?".to_string(),
            agent_parsed: false,
        };
        let result = dispatch_command(source, &ctx);
        assert!(matches!(
            result,
            CommandResult::Agent(AgentPrompt::Investigate { .. })
        ));
    }
}
