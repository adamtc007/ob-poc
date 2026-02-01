//! Galaxy View - Cluster-based visualization for large CBU portfolios
//!
//! Renders clusters (jurisdictions, ManCos, etc.) as glowing orbs with:
//! - Force-directed positioning (repulsion between clusters)
//! - Zoom-responsive compression (clusters collapse at low zoom)
//! - Click to drill into solar system view
//!
//! # EGUI-RULES Compliance
//! - Cluster metadata comes from server, positions are UI-local
//! - Actions return values (NavigationAction from shared types), no callbacks
//! - No server round-trips for animation/position state
//! - tick() called BEFORE ui() - widgets read interpolated values
//! - Service mutates, widget renders

use super::animation::SpringF32;
use super::astronomy::astronomy_colors;
use super::camera::Camera2D;
use super::force_sim::{ClusterNode as ForceClusterNode, ForceConfig, ForceSimulation};
use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};
use std::collections::HashMap;
use uuid::Uuid;

// Import shared types from ob-poc-types for server ↔ client contract
use ob_poc_types::galaxy::{
    AgentMode, AgentSpeech, AgentState, Anomaly, AnomalySeverity, AutopilotMission,
    AutopilotStatus, ClusterType as ApiClusterType, ExpansionState, ExpansionType, FocusFrame,
    FocusStack, LoiterState, NavigationAction, PrefetchHint, PreviewData, PreviewType, Route,
    SpeechUrgency, UniverseGraph,
};

// =============================================================================
// GALAXY DATA (from server)
// =============================================================================

/// Cluster data from server (read-only, positions computed client-side)
#[derive(Debug, Clone)]
pub struct ClusterData {
    /// Cluster identifier (e.g., jurisdiction code or ManCo ID)
    pub id: String,

    /// Display label
    pub label: String,

    /// Short label for compressed view
    pub short_label: String,

    /// Number of CBUs in this cluster
    pub cbu_count: usize,

    /// CBU IDs in this cluster (for drill-down)
    pub cbu_ids: Vec<Uuid>,

    /// Cluster type for styling
    pub cluster_type: ClusterType,

    /// Optional: aggregate risk distribution
    pub risk_summary: Option<RiskSummary>,
}

/// Type of cluster (affects rendering style)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClusterType {
    #[default]
    Jurisdiction,
    ManCo,
    ProductType,
    RiskBand,
    Custom,
}

/// Aggregate risk distribution for a cluster
#[derive(Debug, Clone, Default)]
pub struct RiskSummary {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub unrated: usize,
}

impl RiskSummary {
    /// Get dominant risk level
    pub fn dominant(&self) -> &'static str {
        let max = self.low.max(self.medium).max(self.high).max(self.unrated);
        if max == self.high {
            "HIGH"
        } else if max == self.medium {
            "MEDIUM"
        } else if max == self.low {
            "LOW"
        } else {
            "UNRATED"
        }
    }

    /// Get color based on dominant risk
    pub fn color(&self) -> Color32 {
        astronomy_colors::risk_color(self.dominant())
    }
}

// =============================================================================
// GALAXY VIEW
// =============================================================================

/// Galaxy view widget - renders clusters with force simulation
pub struct GalaxyView {
    /// Force simulation for cluster positioning
    simulation: ForceSimulation,

    /// Cluster metadata (from server)
    clusters: HashMap<String, ClusterData>,

    /// Hovered cluster ID
    hovered: Option<String>,

    /// Glow animation per cluster (for hover effects)
    glow_springs: HashMap<String, SpringF32>,

    /// Is data loaded?
    has_data: bool,

    /// Loiter state for hover-at-decision-point behavior (Phase 3)
    loiter_state: LoiterState,

    /// Branch fan-out animation springs (one per preview item)
    branch_springs: Vec<SpringF32>,

    // =========================================================================
    // PHASE 4: Focus and Expansion
    // =========================================================================
    /// Focus stack for soft focus within level (doesn't change navigation)
    focus_stack: FocusStack,

    /// Expansion states per node (keyed by node ID)
    expansions: HashMap<String, ExpansionState>,

    /// Expansion animation springs (keyed by node ID)
    expansion_springs: HashMap<String, SpringF32>,

    // =========================================================================
    // PHASE 5: Agent Intelligence
    // =========================================================================
    /// Agent state for intelligent assistance
    agent_state: AgentState,

    /// Anomalies per node (keyed by node ID)
    node_anomalies: HashMap<String, Vec<Anomaly>>,

    /// Anomaly pulse animation (continuous sine wave)
    anomaly_pulse: f32,

    /// Agent speech fade animation
    speech_opacity: SpringF32,

    /// When current speech started (for auto-dismiss based on duration_secs)
    speech_start_time: f32,

    /// Cooldown: don't show new speech until this time (prevents spam)
    speech_cooldown_until: f32,

    /// Elapsed time for speech timing (accumulates each tick)
    speech_elapsed: f32,

    /// Prefetch hints from last response
    prefetch_hints: Vec<PrefetchHint>,

    // =========================================================================
    // PHASE 6: Autopilot Navigation
    // =========================================================================
    /// Active autopilot mission (if any)
    autopilot_mission: Option<AutopilotMission>,

    /// Route path visualization (positions interpolated for smooth line)
    route_path_progress: SpringF32,

    /// Camera position spring for autopilot (separate from manual camera)
    autopilot_camera_x: SpringF32,
    autopilot_camera_y: SpringF32,

    /// Whether any user input occurred this frame (aborts autopilot)
    user_input_this_frame: bool,

    // =========================================================================
    // PHASE 8: Depth Atmosphere Particles
    // =========================================================================
    /// Background particles for depth atmosphere effect
    particles: Vec<AtmosphereParticle>,

    /// Current depth level (0=Universe, 1=Cluster, 2=CBU, 3+=Deep)
    /// Affects particle density and appearance
    depth_level: usize,

    // =========================================================================
    // PHASE 8: Accessibility (Keyboard Navigation)
    // =========================================================================
    /// Currently keyboard-selected cluster ID (for keyboard navigation)
    /// Separate from `hovered` which is mouse-driven
    keyboard_selected: Option<String>,

    // =========================================================================
    // PHASE 7: Viewport Scaling
    // =========================================================================
    /// Last known viewport size (for resize detection)
    last_viewport_size: Option<Vec2>,
}

// =============================================================================
// ATMOSPHERE PARTICLES (Phase 8 Polish)
// =============================================================================

/// A single atmosphere particle for depth visualization
///
/// Particles drift slowly in the background, creating a sense of
/// moving through space. Density and brightness vary with depth level.
#[derive(Debug, Clone)]
struct AtmosphereParticle {
    /// Position in world coordinates
    pos: Vec2,
    /// Velocity (drift direction and speed)
    vel: Vec2,
    /// Base opacity (0.0 to 1.0)
    opacity: f32,
    /// Size (radius in world units)
    size: f32,
    /// Phase offset for subtle shimmer
    phase: f32,
}

impl AtmosphereParticle {
    /// Create a new particle at a random position within bounds
    fn new_random(bounds: Vec2, rng_seed: u32) -> Self {
        // Simple pseudo-random using seed
        let hash = |n: u32| -> f32 {
            let x = n.wrapping_mul(0x9E3779B9);
            let x = x ^ (x >> 16);
            (x as f32) / (u32::MAX as f32)
        };

        let x = hash(rng_seed) * bounds.x * 2.0 - bounds.x;
        let y = hash(rng_seed.wrapping_add(1)) * bounds.y * 2.0 - bounds.y;

        // Slow drift velocity
        let vx = (hash(rng_seed.wrapping_add(2)) - 0.5) * 20.0;
        let vy = (hash(rng_seed.wrapping_add(3)) - 0.5) * 20.0;

        // Vary opacity and size
        let opacity = 0.1 + hash(rng_seed.wrapping_add(4)) * 0.3; // 0.1 to 0.4
        let size = 1.0 + hash(rng_seed.wrapping_add(5)) * 3.0; // 1 to 4

        let phase = hash(rng_seed.wrapping_add(6)) * std::f32::consts::TAU;

        Self {
            pos: Vec2::new(x, y),
            vel: Vec2::new(vx, vy),
            opacity,
            size,
            phase,
        }
    }

    /// Update particle position, wrapping at bounds
    fn tick(&mut self, dt: f32, bounds: Vec2) {
        self.pos += self.vel * dt;

        // Wrap around bounds
        if self.pos.x < -bounds.x {
            self.pos.x += bounds.x * 2.0;
        } else if self.pos.x > bounds.x {
            self.pos.x -= bounds.x * 2.0;
        }
        if self.pos.y < -bounds.y {
            self.pos.y += bounds.y * 2.0;
        } else if self.pos.y > bounds.y {
            self.pos.y -= bounds.y * 2.0;
        }

        // Advance shimmer phase
        self.phase += dt * 0.5;
        if self.phase > std::f32::consts::TAU {
            self.phase -= std::f32::consts::TAU;
        }
    }

    /// Get current opacity with shimmer effect
    fn current_opacity(&self) -> f32 {
        // Subtle shimmer: ±20% variation
        let shimmer = 1.0 + self.phase.sin() * 0.2;
        (self.opacity * shimmer).clamp(0.0, 1.0)
    }
}

/// Particle configuration per depth level (from TODO Part 3.1)
struct ParticleConfig {
    /// Number of particles
    count: usize,
    /// Base opacity multiplier
    opacity_mult: f32,
    /// Color tint
    color: Color32,
}

impl ParticleConfig {
    fn for_depth(depth: usize) -> Self {
        match depth {
            0 => Self {
                // Universe: Low density, bright, wide spread
                count: 30,
                opacity_mult: 0.6,
                color: Color32::from_rgb(100, 120, 180), // Cool blue
            },
            1 => Self {
                // Cluster: Medium density, warm
                count: 50,
                opacity_mult: 0.5,
                color: Color32::from_rgb(140, 120, 100), // Warm
            },
            2 => Self {
                // CBU: Higher density
                count: 70,
                opacity_mult: 0.4,
                color: Color32::from_rgb(120, 130, 150), // Neutral
            },
            _ => Self {
                // Deep: Highest density, dimmer
                count: 100,
                opacity_mult: 0.3,
                color: Color32::from_rgb(80, 90, 110), // Dark blue
            },
        }
    }
}

impl Default for GalaxyView {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy action type - DEPRECATED, use NavigationAction from ob-poc-types instead
///
/// Kept for backwards compatibility during transition
#[derive(Debug, Clone)]
#[deprecated(note = "Use NavigationAction from ob_poc_types::galaxy instead")]
pub enum GalaxyAction {
    /// No action
    None,
    /// Cluster was clicked - drill down to solar system
    DrillDown {
        cluster_id: String,
        cluster_label: String,
        cbu_ids: Vec<Uuid>,
    },
    /// Hover changed
    HoverChanged { cluster_id: Option<String> },
}

// Import spring configurations from animation module (Phase 8 Polish)
use super::animation::SpringConfig;

impl GalaxyView {
    pub fn new() -> Self {
        Self {
            simulation: ForceSimulation::with_config(ForceConfig::galaxy()),
            clusters: HashMap::new(),
            hovered: None,
            glow_springs: HashMap::new(),
            has_data: false,
            loiter_state: LoiterState::default(),
            branch_springs: Vec::new(),
            focus_stack: FocusStack::new(),
            expansions: HashMap::new(),
            expansion_springs: HashMap::new(),
            // Phase 5: Agent Intelligence
            agent_state: AgentState::default(),
            node_anomalies: HashMap::new(),
            anomaly_pulse: 0.0,
            speech_opacity: SpringF32::with_config(0.0, SpringConfig::from_preset("agent_ui")),
            speech_start_time: 0.0,
            speech_cooldown_until: 0.0,
            speech_elapsed: 0.0,
            prefetch_hints: Vec::new(),
            // Phase 6: Autopilot Navigation
            autopilot_mission: None,
            route_path_progress: SpringF32::with_config(0.0, SpringConfig::from_preset("gentle")),
            autopilot_camera_x: SpringF32::with_config(0.0, SpringConfig::from_preset("autopilot")),
            autopilot_camera_y: SpringF32::with_config(0.0, SpringConfig::from_preset("autopilot")),
            user_input_this_frame: false,
            // Phase 8: Depth Atmosphere Particles
            particles: Self::create_particles(0), // Start at universe depth
            depth_level: 0,
            // Phase 8: Accessibility (Keyboard Navigation)
            keyboard_selected: None,
            // Phase 7: Viewport Scaling
            last_viewport_size: None,
        }
    }

    /// Create particles for a given depth level
    fn create_particles(depth: usize) -> Vec<AtmosphereParticle> {
        let config = ParticleConfig::for_depth(depth);
        let bounds = Vec2::new(2000.0, 1500.0); // Large world bounds

        (0..config.count)
            .map(|i| AtmosphereParticle::new_random(bounds, i as u32 * 7919)) // Prime for variety
            .collect()
    }

    /// Set the depth level (affects particle density and appearance)
    pub fn set_depth_level(&mut self, depth: usize) {
        if depth != self.depth_level {
            self.depth_level = depth;
            self.particles = Self::create_particles(depth);
        }
    }

    // =========================================================================
    // DATA LOADING
    // =========================================================================

    /// Load cluster data from server (internal ClusterData format)
    pub fn set_clusters(&mut self, clusters: Vec<ClusterData>) {
        self.simulation.clear();
        self.clusters.clear();
        self.glow_springs.clear();

        for cluster in clusters {
            // Create simulation node (ForceClusterNode for physics)
            let node = ForceClusterNode::new(&cluster.id, &cluster.label, cluster.cbu_count)
                .with_color(self.color_for_cluster(&cluster));

            self.simulation.add_node(node);
            self.glow_springs
                .insert(cluster.id.clone(), SpringF32::new(0.0));
            self.clusters.insert(cluster.id.clone(), cluster);
        }

        self.has_data = !self.clusters.is_empty();

        // Give initial kick to spread out
        if self.has_data {
            self.simulation.kick();
        }
    }

    /// Load from server response (UniverseGraph from ob-poc-types)
    ///
    /// Converts API types to internal rendering types
    pub fn load_from_universe_graph(&mut self, graph: &UniverseGraph) {
        let clusters: Vec<ClusterData> = graph
            .clusters
            .iter()
            .map(|api_cluster| {
                // Generate short label from ID (e.g., "jurisdiction:LU" -> "LU")
                let short_label = api_cluster
                    .id
                    .split(':')
                    .next_back()
                    .unwrap_or(&api_cluster.id)
                    .to_string();

                ClusterData {
                    id: api_cluster.id.clone(),
                    label: api_cluster.label.clone(),
                    short_label,
                    cbu_count: api_cluster.cbu_count as usize,
                    cbu_ids: vec![], // CBU IDs loaded on drill-down
                    cluster_type: Self::convert_cluster_type(&api_cluster.cluster_type),
                    risk_summary: Some(RiskSummary {
                        low: api_cluster.risk_summary.low as usize,
                        medium: api_cluster.risk_summary.medium as usize,
                        high: api_cluster.risk_summary.high as usize,
                        unrated: api_cluster.risk_summary.unrated as usize,
                    }),
                }
            })
            .collect();

        self.set_clusters(clusters);
    }

    /// Convert API ClusterType to internal ClusterType
    fn convert_cluster_type(api_type: &ApiClusterType) -> ClusterType {
        match api_type {
            ApiClusterType::Jurisdiction => ClusterType::Jurisdiction,
            ApiClusterType::Client => ClusterType::ManCo, // Client maps to ManCo
            ApiClusterType::Risk => ClusterType::RiskBand,
            ApiClusterType::Product => ClusterType::ProductType,
        }
    }

    /// Generate mock data for testing
    pub fn load_mock_data(&mut self) {
        let mock_clusters = vec![
            ClusterData {
                id: "LU".into(),
                label: "Luxembourg".into(),
                short_label: "LU".into(),
                cbu_count: 177,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 150,
                    medium: 20,
                    high: 5,
                    unrated: 2,
                }),
            },
            ClusterData {
                id: "IE".into(),
                label: "Ireland".into(),
                short_label: "IE".into(),
                cbu_count: 150,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 120,
                    medium: 25,
                    high: 3,
                    unrated: 2,
                }),
            },
            ClusterData {
                id: "DE".into(),
                label: "Germany".into(),
                short_label: "DE".into(),
                cbu_count: 200,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 180,
                    medium: 15,
                    high: 3,
                    unrated: 2,
                }),
            },
            ClusterData {
                id: "FR".into(),
                label: "France".into(),
                short_label: "FR".into(),
                cbu_count: 80,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 70,
                    medium: 8,
                    high: 1,
                    unrated: 1,
                }),
            },
            ClusterData {
                id: "UK".into(),
                label: "United Kingdom".into(),
                short_label: "UK".into(),
                cbu_count: 45,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 35,
                    medium: 8,
                    high: 1,
                    unrated: 1,
                }),
            },
            ClusterData {
                id: "CH".into(),
                label: "Switzerland".into(),
                short_label: "CH".into(),
                cbu_count: 19,
                cbu_ids: vec![],
                cluster_type: ClusterType::Jurisdiction,
                risk_summary: Some(RiskSummary {
                    low: 15,
                    medium: 3,
                    high: 1,
                    unrated: 0,
                }),
            },
        ];

        self.set_clusters(mock_clusters);
    }

    /// Check if data is loaded
    pub fn has_data(&self) -> bool {
        self.has_data
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.simulation.clear();
        self.clusters.clear();
        self.glow_springs.clear();
        self.hovered = None;
        self.has_data = false;
        self.loiter_state.reset();
        self.branch_springs.clear();
        self.focus_stack.clear();
        self.expansions.clear();
        self.expansion_springs.clear();
        // Phase 5: Clear agent state
        self.agent_state = AgentState::default();
        self.node_anomalies.clear();
        self.anomaly_pulse = 0.0;
        self.speech_opacity.set_target(0.0);
        self.prefetch_hints.clear();
    }

    // =========================================================================
    // STYLING
    // =========================================================================

    fn color_for_cluster(&self, cluster: &ClusterData) -> Color32 {
        if let Some(ref risk) = cluster.risk_summary {
            risk.color()
        } else {
            match cluster.cluster_type {
                ClusterType::Jurisdiction => Color32::from_rgb(100, 149, 237), // Cornflower
                ClusterType::ManCo => Color32::from_rgb(147, 112, 219),        // Medium purple
                ClusterType::ProductType => Color32::from_rgb(255, 167, 38),   // Orange
                ClusterType::RiskBand => Color32::from_rgb(76, 175, 80),       // Green
                ClusterType::Custom => Color32::from_rgb(158, 158, 158),       // Grey
            }
        }
    }

    // =========================================================================
    // UPDATE & RENDER (egui-compliant - tick() BEFORE ui())
    // =========================================================================

    /// Tick animations - call BEFORE ui()
    ///
    /// Updates force simulation, glow springs, and loiter state.
    /// Returns true if loiter threshold was crossed (trigger preview fetch).
    pub fn tick(&mut self, dt: f32, zoom: f32) -> bool {
        // Update simulation
        self.simulation.set_zoom(zoom);
        self.simulation.tick(dt);

        // Update glow springs
        for (id, spring) in self.glow_springs.iter_mut() {
            let target = if Some(id) == self.hovered.as_ref() {
                1.0
            } else {
                0.0
            };
            spring.set_target(target);
            spring.tick(dt);
        }

        // Update loiter state (returns true when threshold crossed)
        let threshold_crossed = self.loiter_state.update(dt);

        // Update branch fan-out springs
        let branches_visible = self.loiter_state.branches_visible;
        for (i, spring) in self.branch_springs.iter_mut().enumerate() {
            let target = if branches_visible { 1.0 } else { 0.0 };
            // Stagger animation - each branch starts slightly later
            let delay = i as f32 * 0.05;
            if self.loiter_state.duration > delay {
                spring.set_target(target);
            }
            spring.tick(dt);
        }

        // Update expansion animations (Phase 4)
        self.tick_expansions(dt);

        // Update agent animations (Phase 5)
        self.tick_agent(dt);

        // Update autopilot (Phase 6)
        self.tick_autopilot(dt);

        // Update atmosphere particles (Phase 8)
        self.tick_particles(dt);

        // Reset user input flag for next frame
        self.user_input_this_frame = false;

        threshold_crossed
    }

    /// Tick atmosphere particles (Phase 8)
    fn tick_particles(&mut self, dt: f32) {
        let bounds = Vec2::new(2000.0, 1500.0);
        for particle in &mut self.particles {
            particle.tick(dt, bounds);
        }
    }

    /// Tick agent-related animations (Phase 5)
    fn tick_agent(&mut self, dt: f32) {
        // Track elapsed time for speech auto-dismiss
        self.speech_elapsed += dt;

        // Anomaly pulse: continuous 2000ms sine wave per TODO spec
        // Phase advances at 2*PI per 2 seconds = PI per second
        self.anomaly_pulse += dt * std::f32::consts::PI;
        // Keep in [0, 2*PI] range to avoid float overflow
        if self.anomaly_pulse > std::f32::consts::TAU {
            self.anomaly_pulse -= std::f32::consts::TAU;
        }

        // Speech opacity spring
        self.speech_opacity.tick(dt);

        // Auto-dismiss speech after duration_secs (Phase 8: verbosity tuning)
        // Per spec: "Fade in over 300ms, hold 3s, fade out"
        if let Some(ref speech) = self.agent_state.speech {
            let elapsed_since_start = self.speech_elapsed - self.speech_start_time;
            // Only auto-dismiss if duration_secs > 0 (0 = stay until dismissed)
            if speech.duration_secs > 0.0 && elapsed_since_start >= speech.duration_secs {
                self.speech_opacity.set_target(0.0);
            }
        }

        // Clear speech when fully faded out
        if self.speech_opacity.get() < 0.01 && self.agent_state.speech.is_some() {
            self.agent_state.speech = None;
        }
    }

    /// Get the anomaly pulse value (0.0 to 1.0) for rendering
    fn anomaly_pulse_value(&self) -> f32 {
        // Sine wave from 0.3 to 1.0 for subtler pulse
        let raw = (self.anomaly_pulse.sin() + 1.0) / 2.0; // 0 to 1
        0.3 + raw * 0.7 // 0.3 to 1.0
    }

    // =========================================================================
    // PHASE 6: AUTOPILOT
    // =========================================================================

    /// Tick autopilot mission - advances along route
    fn tick_autopilot(&mut self, dt: f32) {
        // Collect deferred actions to avoid borrow conflicts
        let mut deferred_speech: Option<AgentSpeech> = None;
        let mut should_update_camera = false;

        // Check for user input abort
        if self.user_input_this_frame {
            if let Some(ref mut mission) = self.autopilot_mission {
                if mission.status == AutopilotStatus::Flying {
                    mission.abort();
                    deferred_speech = Some(AgentSpeech {
                        text: "Autopilot disengaged.".to_string(),
                        urgency: SpeechUrgency::Info,
                        duration_secs: 2.0,
                        anchor_node_id: None,
                        started_at: 0.0,
                    });
                }
            }
        }

        // Update route path visualization spring
        self.route_path_progress.tick(dt);

        // Update camera position springs
        self.autopilot_camera_x.tick(dt);
        self.autopilot_camera_y.tick(dt);

        // Advance mission if flying
        if let Some(ref mut mission) = self.autopilot_mission {
            match mission.status {
                AutopilotStatus::Flying => {
                    // Advance leg progress based on speed
                    let leg_speed = 0.5 * mission.speed; // Complete a leg in ~2 seconds at speed 1.0
                    mission.leg_progress += dt * leg_speed;

                    // Check if we've completed this leg
                    if mission.leg_progress >= 1.0 {
                        // Check if next waypoint is a fork and we should pause
                        let next_label = mission.next().map(|w| (w.label.clone(), w.is_fork));

                        if let Some((label, is_fork)) = next_label {
                            if is_fork && mission.pause_at_forks {
                                mission.status = AutopilotStatus::PausedAtFork;
                                deferred_speech = Some(AgentSpeech {
                                    text: format!("Approaching {}. Which way?", label),
                                    urgency: SpeechUrgency::Info,
                                    duration_secs: 5.0,
                                    anchor_node_id: None,
                                    started_at: 0.0,
                                });
                            } else {
                                // Advance to next waypoint
                                mission.advance();
                                should_update_camera = true;
                            }
                        } else {
                            // No next waypoint - we've arrived
                            // Get destination label before advancing
                            let dest_label = mission
                                .route
                                .waypoints
                                .last()
                                .map(|w| w.label.clone())
                                .unwrap_or_else(|| "destination".to_string());
                            mission.advance(); // This sets status to Arrived
                            deferred_speech = Some(AgentSpeech {
                                text: format!("Arrived at {}.", dest_label),
                                urgency: SpeechUrgency::Info,
                                duration_secs: 3.0,
                                anchor_node_id: None,
                                started_at: 0.0,
                            });
                        }
                    }
                }
                AutopilotStatus::PausedAtFork => {
                    // Waiting for user decision - do nothing
                }
                AutopilotStatus::Paused => {
                    // Manually paused - do nothing
                }
                AutopilotStatus::Arrived | AutopilotStatus::Aborted => {
                    // Mission complete - could clear after delay
                }
            }
        }

        // Execute deferred actions after borrows are released
        if should_update_camera {
            self.update_autopilot_camera_target();
        }
        if let Some(speech) = deferred_speech {
            self.show_speech(speech);
        }
    }

    /// Update the autopilot camera target based on current waypoint
    fn update_autopilot_camera_target(&mut self) {
        if let Some(ref mission) = self.autopilot_mission {
            if let Some(waypoint) = mission.current() {
                self.autopilot_camera_x.set_target(waypoint.position.0);
                self.autopilot_camera_y.set_target(waypoint.position.1);
            }
        }
    }

    /// Start an autopilot mission with the given route
    pub fn start_autopilot(&mut self, route: Route) {
        let mission = AutopilotMission::new(route);

        // Set initial camera target
        if let Some(waypoint) = mission.current() {
            self.autopilot_camera_x.set_target(waypoint.position.0);
            self.autopilot_camera_y.set_target(waypoint.position.1);
        }

        // Animate route path appearing
        self.route_path_progress.set_target(1.0);

        // Show agent message
        let destination = mission
            .route
            .waypoints
            .last()
            .map(|w| w.label.clone())
            .unwrap_or_else(|| "destination".to_string());

        self.show_speech(AgentSpeech {
            text: format!("Engaging autopilot to {}.", destination),
            urgency: SpeechUrgency::Info,
            duration_secs: 3.0,
            anchor_node_id: None,
            started_at: 0.0,
        });

        self.autopilot_mission = Some(mission);
    }

    /// Abort the current autopilot mission
    pub fn abort_autopilot(&mut self) {
        if let Some(ref mut mission) = self.autopilot_mission {
            mission.abort();
            self.route_path_progress.set_target(0.0);
        }
    }

    /// Resume autopilot from a fork pause
    pub fn resume_autopilot(&mut self) {
        if let Some(ref mut mission) = self.autopilot_mission {
            if mission.status == AutopilotStatus::PausedAtFork
                || mission.status == AutopilotStatus::Paused
            {
                mission.resume();
                mission.advance();
                self.update_autopilot_camera_target();
            }
        }
    }

    /// Check if autopilot is active
    pub fn is_autopilot_active(&self) -> bool {
        self.autopilot_mission
            .as_ref()
            .map(|m| {
                m.status == AutopilotStatus::Flying || m.status == AutopilotStatus::PausedAtFork
            })
            .unwrap_or(false)
    }

    /// Get the current autopilot mission (if any)
    pub fn autopilot_mission(&self) -> Option<&AutopilotMission> {
        self.autopilot_mission.as_ref()
    }

    /// Clear completed/aborted autopilot mission
    pub fn clear_autopilot(&mut self) {
        if let Some(ref mission) = self.autopilot_mission {
            if mission.status == AutopilotStatus::Arrived
                || mission.status == AutopilotStatus::Aborted
            {
                self.autopilot_mission = None;
                self.route_path_progress.set_target(0.0);
            }
        }
    }

    /// Get interpolated autopilot camera position (for external camera control)
    pub fn autopilot_camera_position(&self) -> Option<(f32, f32)> {
        if self.is_autopilot_active() {
            Some((self.autopilot_camera_x.get(), self.autopilot_camera_y.get()))
        } else {
            None
        }
    }

    /// Signal that user input occurred (will abort autopilot)
    pub fn signal_user_input(&mut self) {
        self.user_input_this_frame = true;
    }

    /// Render the autopilot route path
    pub fn render_route_path(&self, painter: &Painter, camera: &Camera2D, screen_rect: Rect) {
        let Some(ref mission) = self.autopilot_mission else {
            return;
        };

        let progress = self.route_path_progress.get();
        if progress < 0.01 {
            return;
        }

        let waypoints = &mission.route.waypoints;
        if waypoints.len() < 2 {
            return;
        }

        // Draw path segments up to current progress
        let total_segments = waypoints.len() - 1;
        let segments_to_draw = (total_segments as f32 * progress).ceil() as usize;

        for i in 0..segments_to_draw.min(total_segments) {
            let from = &waypoints[i];
            let to = &waypoints[i + 1];

            let from_screen =
                camera.world_to_screen(Pos2::new(from.position.0, from.position.1), screen_rect);
            let to_screen =
                camera.world_to_screen(Pos2::new(to.position.0, to.position.1), screen_rect);

            // Determine if this segment is completed, current, or upcoming
            let segment_alpha = if i < mission.current_waypoint {
                0.3 // Completed segment - dim
            } else if i == mission.current_waypoint {
                0.8 // Current segment - bright
            } else {
                0.5 // Upcoming segment - medium
            };

            // Route color based on view level transition
            let color = if from.view_level != to.view_level {
                // Level transition - use gold
                Color32::from_rgba_unmultiplied(255, 215, 0, (segment_alpha * 255.0) as u8)
            } else {
                // Same level - use cyan
                Color32::from_rgba_unmultiplied(0, 255, 255, (segment_alpha * 255.0) as u8)
            };

            // Draw the segment
            painter.line_segment([from_screen, to_screen], Stroke::new(2.0, color));

            // Draw waypoint marker
            let marker_color = if waypoints[i + 1].is_fork {
                Color32::from_rgba_unmultiplied(255, 165, 0, (segment_alpha * 255.0) as u8)
            // Orange for forks
            } else {
                color
            };
            painter.circle_filled(to_screen, 4.0, marker_color);
        }

        // Draw current position indicator (animated dot along current segment)
        if let (Some(current), Some(next)) = (mission.current(), mission.next()) {
            let from_pos = Pos2::new(current.position.0, current.position.1);
            let to_pos = Pos2::new(next.position.0, next.position.1);
            let lerp_pos = from_pos + (to_pos - from_pos) * mission.leg_progress;
            let screen_pos = camera.world_to_screen(lerp_pos, screen_rect);

            // Pulsing indicator
            let pulse = (self.anomaly_pulse * 2.0).sin() * 0.3 + 0.7;
            let radius = 6.0 * pulse;
            painter.circle_filled(screen_pos, radius, Color32::WHITE);
            painter.circle_stroke(
                screen_pos,
                radius + 2.0,
                Stroke::new(1.5, Color32::from_rgb(0, 255, 255)),
            );
        }
    }

    /// Render clusters - call AFTER tick()
    ///
    /// Renders clusters and returns NavigationAction if user interacted
    pub fn render(&self, painter: &Painter, camera: &Camera2D, screen_rect: Rect) {
        // Render atmosphere particles first (background layer) - Phase 8
        self.render_particles(painter, camera, screen_rect);

        let compression = self.simulation.compression();

        for node in self.simulation.nodes() {
            let screen_pos = camera.world_to_screen(node.position, screen_rect);
            let radius = node.display_radius(compression) * camera.zoom();

            // Phase 8: Frustum culling - skip clusters entirely outside visible area
            let cull_margin = radius + 20.0; // Include glow radius
            if screen_pos.x < screen_rect.left() - cull_margin
                || screen_pos.x > screen_rect.right() + cull_margin
                || screen_pos.y < screen_rect.top() - cull_margin
                || screen_pos.y > screen_rect.bottom() + cull_margin
            {
                continue;
            }

            let glow = self
                .glow_springs
                .get(&node.id)
                .map(|s| s.get())
                .unwrap_or(0.0);

            self.render_cluster(painter, screen_pos, radius, node, glow, compression);

            // Render branch fan-out if this is the loitering node
            if Some(&node.id) == self.loiter_state.node_id.as_ref() {
                self.render_branches(painter, screen_pos, radius, camera);
            }

            // Render expansion if this node is expanded (Phase 4)
            if let Some(expansion) = self.expansions.get(&node.id) {
                if expansion.progress > 0.01 {
                    self.render_expansion(painter, screen_pos, radius, &node.id, expansion);
                }
            }

            // Render focus indicator if this node is focused
            if self.is_focused(&node.id) {
                self.render_focus_indicator(painter, screen_pos, radius);
            }

            // Render keyboard selection indicator (Phase 8: Accessibility)
            if Some(&node.id) == self.keyboard_selected.as_ref() {
                self.render_keyboard_selection(painter, screen_pos, radius);
            }

            // Render anomaly badge if this node has anomalies (Phase 5)
            if let Some(anomalies) = self.node_anomalies.get(&node.id) {
                if !anomalies.is_empty() {
                    self.render_anomaly_badge(painter, screen_pos, radius, anomalies);
                }
            }
        }

        // Render focus breadcrumbs if we have focus
        if !self.focus_stack.is_empty() {
            self.render_focus_breadcrumbs(painter, screen_rect);
        }

        // Render agent speech bubble if present (Phase 5)
        if let Some(ref speech) = self.agent_state.speech {
            self.render_agent_speech(painter, screen_rect, speech, camera);
        }

        // Render title
        self.render_title(painter, screen_rect);
    }

    /// Handle input and return NavigationAction
    ///
    /// Returns Some(NavigationAction) if user clicked or hovered a cluster.
    /// Also updates loiter state for hover-at-decision-point behavior.
    pub fn handle_input_v2(
        &mut self,
        response: &egui::Response,
        camera: &Camera2D,
        screen_rect: Rect,
    ) -> Option<NavigationAction> {
        // Hit test for hover
        if let Some(pointer_pos) = response.hover_pos() {
            let world_pos = camera.screen_to_world(pointer_pos, screen_rect);
            let new_hover = self.simulation.node_id_at(world_pos).map(|s| s.to_string());

            if new_hover != self.hovered {
                self.hovered = new_hover.clone();

                // Update loiter state when hover changes
                if let Some(ref node_id) = new_hover {
                    // Start loitering on new node
                    self.loiter_state = LoiterState::new(node_id.clone());
                } else {
                    // Clear loiter state when leaving all nodes
                    self.loiter_state.reset();
                    self.branch_springs.clear();
                }
            }

            // Update branch highlighting based on mouse direction from node center
            if self.loiter_state.branches_visible {
                if let Some(ref node_id) = self.hovered {
                    if let Some(node) = self.simulation.get_node(node_id) {
                        let node_screen_pos = camera.world_to_screen(node.position, screen_rect);
                        let delta = pointer_pos - node_screen_pos;
                        let angle = delta.y.atan2(delta.x);
                        let branch_count = self.branch_springs.len();
                        if branch_count > 0 {
                            self.loiter_state.highlight_from_angle(angle, branch_count);
                        }
                    }
                }
            }
        } else if self.hovered.is_some() {
            self.hovered = None;
            self.loiter_state.reset();
            self.branch_springs.clear();
        }

        // Click to drill down into cluster OR select highlighted branch
        if response.clicked() {
            // If branches are visible and one is highlighted, select that branch
            if self.loiter_state.branches_visible {
                if let Some(branch_idx) = self.loiter_state.highlighted_branch {
                    if let Some(ref preview) = self.loiter_state.preview {
                        if let Some(item) = preview.items.get(branch_idx) {
                            return Some(item.action.clone());
                        }
                    }
                }
            }

            // Otherwise, drill into the hovered cluster
            if let Some(ref hovered_id) = self.hovered {
                if let Some(cluster) = self.clusters.get(hovered_id) {
                    return Some(NavigationAction::DrillIntoCluster {
                        cluster_id: cluster.id.clone(),
                    });
                }
            }
        }

        // Drag to move cluster (UI-local, no NavigationAction)
        if response.dragged() {
            // Cancel loiter state while dragging
            self.loiter_state.reset();
            self.branch_springs.clear();

            if let Some(ref hovered_id) = self.hovered {
                self.simulation.pin(hovered_id);
                if let Some(pointer_pos) = response.hover_pos() {
                    let world_pos = camera.screen_to_world(pointer_pos, screen_rect);
                    self.simulation.move_node(hovered_id, world_pos);
                }
            }
        }

        if response.drag_stopped() {
            if let Some(ref hovered_id) = self.hovered {
                self.simulation.unpin(hovered_id);
            }
        }

        // =====================================================================
        // KEYBOARD NAVIGATION (Phase 8: Accessibility)
        // =====================================================================
        // Handle keyboard input for accessibility
        if let Some(action) = self.handle_keyboard(response, camera, screen_rect) {
            return Some(action);
        }

        None
    }

    /// Handle keyboard input for accessibility navigation
    ///
    /// Supports:
    /// - Tab / Shift+Tab: Cycle through clusters
    /// - Arrow keys: Spatial navigation to nearest cluster in direction
    /// - Enter / Space: Drill into selected cluster
    /// - Escape: Clear keyboard selection
    /// - Home: Select first cluster
    fn handle_keyboard(
        &mut self,
        response: &egui::Response,
        _camera: &Camera2D,
        _screen_rect: Rect,
    ) -> Option<NavigationAction> {
        // Only handle keyboard when widget has focus
        if !response.has_focus() && !response.gained_focus() {
            return None;
        }

        let mut action: Option<NavigationAction> = None;
        let mut needs_camera_pan = false;

        response.ctx.input(|input| {
            // Tab / Shift+Tab: Cycle through clusters
            if input.key_pressed(egui::Key::Tab) {
                let cluster_ids: Vec<String> = self.clusters.keys().cloned().collect();
                if !cluster_ids.is_empty() {
                    let reverse = input.modifiers.shift;
                    self.keyboard_selected = self.cycle_selection(&cluster_ids, reverse);
                    needs_camera_pan = true;
                }
            }

            // Arrow keys: Spatial navigation
            if input.key_pressed(egui::Key::ArrowUp) {
                self.keyboard_selected = self.find_nearest_in_direction(Vec2::new(0.0, -1.0));
                needs_camera_pan = true;
            }
            if input.key_pressed(egui::Key::ArrowDown) {
                self.keyboard_selected = self.find_nearest_in_direction(Vec2::new(0.0, 1.0));
                needs_camera_pan = true;
            }
            if input.key_pressed(egui::Key::ArrowLeft) {
                self.keyboard_selected = self.find_nearest_in_direction(Vec2::new(-1.0, 0.0));
                needs_camera_pan = true;
            }
            if input.key_pressed(egui::Key::ArrowRight) {
                self.keyboard_selected = self.find_nearest_in_direction(Vec2::new(1.0, 0.0));
                needs_camera_pan = true;
            }

            // Enter / Space: Drill into selected cluster
            if input.key_pressed(egui::Key::Enter) || input.key_pressed(egui::Key::Space) {
                if let Some(ref selected_id) = self.keyboard_selected {
                    if let Some(cluster) = self.clusters.get(selected_id) {
                        action = Some(NavigationAction::DrillIntoCluster {
                            cluster_id: cluster.id.clone(),
                        });
                    }
                }
            }

            // Escape: Clear keyboard selection
            if input.key_pressed(egui::Key::Escape) {
                self.keyboard_selected = None;
            }

            // Home: Select first cluster
            if input.key_pressed(egui::Key::Home) {
                if let Some(first_id) = self.clusters.keys().next() {
                    self.keyboard_selected = Some(first_id.clone());
                    needs_camera_pan = true;
                }
            }
        });

        // Pan camera to selected cluster (deferred to avoid borrow issues)
        if needs_camera_pan {
            if let Some(ref selected_id) = self.keyboard_selected {
                if let Some(node) = self.simulation.get_node(selected_id) {
                    // Note: Camera panning would need to be handled by caller
                    // since we don't have mutable access to camera here.
                    // The caller should check keyboard_selected and pan accordingly.
                    let _ = node.position; // Position is available for caller
                }
            }
        }

        action
    }

    /// Cycle through cluster selection (Tab / Shift+Tab)
    fn cycle_selection(&self, cluster_ids: &[String], reverse: bool) -> Option<String> {
        match &self.keyboard_selected {
            None => {
                // No selection: select first or last
                if reverse {
                    cluster_ids.last().cloned()
                } else {
                    cluster_ids.first().cloned()
                }
            }
            Some(current) => {
                // Find current index and move to next/prev
                if let Some(idx) = cluster_ids.iter().position(|id| id == current) {
                    let new_idx = if reverse {
                        if idx == 0 {
                            cluster_ids.len() - 1
                        } else {
                            idx - 1
                        }
                    } else {
                        (idx + 1) % cluster_ids.len()
                    };
                    cluster_ids.get(new_idx).cloned()
                } else {
                    // Current not found, select first
                    cluster_ids.first().cloned()
                }
            }
        }
    }

    /// Find nearest cluster in the given direction from current selection
    fn find_nearest_in_direction(&self, direction: Vec2) -> Option<String> {
        // Get current position (or center if nothing selected)
        let current_pos = self
            .keyboard_selected
            .as_ref()
            .and_then(|id| self.simulation.get_node(id))
            .map(|n| n.position)
            .unwrap_or(Pos2::ZERO);

        let mut best_id: Option<String> = None;
        let mut best_score = f32::MAX;

        for id in self.clusters.keys() {
            // Skip current selection
            if Some(id) == self.keyboard_selected.as_ref() {
                continue;
            }

            if let Some(node) = self.simulation.get_node(id) {
                // Convert Pos2 delta to Vec2 for direction math
                let delta = node.position - current_pos;

                // Check if node is in the right direction (dot product > 0)
                let dot = delta.normalized().dot(direction);
                if dot <= 0.1 {
                    continue; // Not in the right direction
                }

                // Score: prefer nodes more aligned with direction and closer
                let distance = delta.length();
                let alignment = 1.0 - dot; // 0 = perfectly aligned, 1 = perpendicular
                let score = distance * (1.0 + alignment * 2.0);

                if score < best_score {
                    best_score = score;
                    best_id = Some(id.clone());
                }
            }
        }

        // If no node found in direction, keep current selection
        best_id.or_else(|| self.keyboard_selected.clone())
    }

    /// Get the currently keyboard-selected cluster ID (for external use)
    pub fn keyboard_selected(&self) -> Option<&String> {
        self.keyboard_selected.as_ref()
    }

    /// Get the position of the keyboard-selected cluster (for camera panning)
    pub fn keyboard_selected_position(&self) -> Option<Pos2> {
        self.keyboard_selected
            .as_ref()
            .and_then(|id| self.simulation.get_node(id))
            .map(|n| Pos2::new(n.position.x, n.position.y))
    }

    // =========================================================================
    // SCREEN READER SUPPORT (Phase 8: Accessibility)
    // =========================================================================

    /// Get accessibility label for screen readers
    ///
    /// Returns a description of the current state suitable for screen readers.
    /// This should be used by the caller to set appropriate ARIA labels.
    pub fn accessibility_label(&self) -> String {
        let cluster_count = self.clusters.len();

        if cluster_count == 0 {
            return "Galaxy view: No clusters loaded".to_string();
        }

        let selection_info = if let Some(ref id) = self.keyboard_selected {
            if let Some(cluster) = self.clusters.get(id) {
                format!("Selected: {}, {} CBUs", cluster.label, cluster.cbu_count)
            } else {
                "Selection invalid".to_string()
            }
        } else if let Some(ref id) = self.hovered {
            if let Some(cluster) = self.clusters.get(id) {
                format!("Hovering: {}, {} CBUs", cluster.label, cluster.cbu_count)
            } else {
                "No selection".to_string()
            }
        } else {
            "No selection".to_string()
        };

        format!(
            "Galaxy view: {} clusters. {}. Use Tab to navigate, Enter to drill down, Escape to clear.",
            cluster_count,
            selection_info
        )
    }

    /// Get accessibility description for the currently selected/hovered cluster
    pub fn focused_cluster_description(&self) -> Option<String> {
        // Prefer keyboard selection, fall back to hover
        let cluster_id = self.keyboard_selected.as_ref().or(self.hovered.as_ref())?;
        let cluster = self.clusters.get(cluster_id)?;

        let risk_info = if let Some(ref risk) = cluster.risk_summary {
            format!(
                " Risk: {} high, {} medium, {} low.",
                risk.high, risk.medium, risk.low
            )
        } else {
            String::new()
        };

        Some(format!(
            "{}: {} client business units.{}",
            cluster.label, cluster.cbu_count, risk_info
        ))
    }

    /// Get list of all clusters for accessibility navigation announcement
    pub fn cluster_list_description(&self) -> String {
        if self.clusters.is_empty() {
            return "No clusters".to_string();
        }

        let mut labels: Vec<&str> = self.clusters.values().map(|c| c.label.as_str()).collect();
        labels.sort();

        format!("{} clusters: {}", labels.len(), labels.join(", "))
    }

    // =========================================================================
    // LEGACY METHODS (deprecated - kept for backwards compatibility)
    // =========================================================================

    /// Update simulation and render
    ///
    /// DEPRECATED: Use tick() + render() + handle_input_v2() instead
    #[deprecated(note = "Use tick() + render() + handle_input_v2() instead")]
    #[allow(deprecated)]
    pub fn ui(
        &mut self,
        painter: &Painter,
        camera: &Camera2D,
        screen_rect: Rect,
        dt: f32,
    ) -> GalaxyAction {
        // Phase 7: Handle viewport resize - scale simulation bounds
        let current_size = screen_rect.size();
        let size_changed = self.last_viewport_size.is_none_or(|prev| {
            (prev.x - current_size.x).abs() > 10.0 || (prev.y - current_size.y).abs() > 10.0
        });
        if size_changed {
            self.simulation
                .set_viewport_size(current_size.x, current_size.y);
            self.last_viewport_size = Some(current_size);
        }

        // Update simulation
        self.simulation.set_zoom(camera.zoom());
        self.simulation.tick(dt);

        // Update glow springs
        for (id, spring) in self.glow_springs.iter_mut() {
            let target = if Some(id) == self.hovered.as_ref() {
                1.0
            } else {
                0.0
            };
            spring.set_target(target);
            spring.tick(dt);
        }

        // Render clusters
        let compression = self.simulation.compression();

        for node in self.simulation.nodes() {
            let screen_pos = camera.world_to_screen(node.position, screen_rect);
            let radius = node.display_radius(compression) * camera.zoom();
            let glow = self
                .glow_springs
                .get(&node.id)
                .map(|s| s.get())
                .unwrap_or(0.0);

            self.render_cluster(painter, screen_pos, radius, node, glow, compression);
        }

        // Render title
        self.render_title(painter, screen_rect);

        GalaxyAction::None
    }

    /// Handle input (call before ui)
    ///
    /// DEPRECATED: Use handle_input_v2() instead
    #[deprecated(note = "Use handle_input_v2() instead")]
    #[allow(deprecated)]
    pub fn handle_input(
        &mut self,
        response: &egui::Response,
        camera: &Camera2D,
        screen_rect: Rect,
    ) -> GalaxyAction {
        let mut action = GalaxyAction::None;

        // Hit test for hover
        if let Some(pointer_pos) = response.hover_pos() {
            let world_pos = camera.screen_to_world(pointer_pos, screen_rect);
            let new_hover = self.simulation.node_id_at(world_pos).map(|s| s.to_string());

            if new_hover != self.hovered {
                self.hovered = new_hover.clone();
                action = GalaxyAction::HoverChanged {
                    cluster_id: new_hover,
                };
            }
        } else if self.hovered.is_some() {
            self.hovered = None;
            action = GalaxyAction::HoverChanged { cluster_id: None };
        }

        // Click to drill down
        if response.clicked() {
            if let Some(ref hovered_id) = self.hovered {
                if let Some(cluster) = self.clusters.get(hovered_id) {
                    action = GalaxyAction::DrillDown {
                        cluster_id: cluster.id.clone(),
                        cluster_label: cluster.label.clone(),
                        cbu_ids: cluster.cbu_ids.clone(),
                    };
                }
            }
        }

        // Drag to move cluster
        if response.dragged() {
            if let Some(ref hovered_id) = self.hovered {
                self.simulation.pin(hovered_id);
                if let Some(pointer_pos) = response.hover_pos() {
                    let world_pos = camera.screen_to_world(pointer_pos, screen_rect);
                    self.simulation.move_node(hovered_id, world_pos);
                }
            }
        }

        if response.drag_stopped() {
            if let Some(ref hovered_id) = self.hovered {
                self.simulation.unpin(hovered_id);
            }
        }

        action
    }

    // =========================================================================
    // RENDERING
    // =========================================================================

    fn render_cluster(
        &self,
        painter: &Painter,
        pos: Pos2,
        radius: f32,
        node: &ForceClusterNode,
        glow: f32,
        compression: f32,
    ) {
        let is_hovered = Some(&node.id) == self.hovered.as_ref();

        // Phase 8: LOD-based rendering
        // Micro (< 8px): just a dot
        // Icon (8-20px): core + border only
        // Compact (20-40px): + single glow layer + short label
        // Standard (40+): full effects

        if radius < 8.0 {
            // Micro LOD: just a colored dot
            painter.circle_filled(pos, radius.max(4.0), node.color);
            return;
        }

        if radius < 20.0 {
            // Icon LOD: core + border, no glow layers
            let core_color = if is_hovered {
                astronomy_colors::brighten(node.color, 1.2)
            } else {
                node.color
            };
            painter.circle_filled(pos, radius, core_color);
            let border_color = if is_hovered {
                Color32::WHITE
            } else {
                Color32::from_rgba_unmultiplied(255, 255, 255, 80)
            };
            painter.circle_stroke(pos, radius, Stroke::new(1.0, border_color));
            return;
        }

        // Compact and above: add glow effects
        if radius >= 40.0 {
            // Standard LOD: outer glow (only for larger clusters)
            let glow_radius = radius * (1.3 + glow * 0.3);
            let glow_alpha = (0.15 + glow * 0.2) as u8;
            let glow_color = Color32::from_rgba_unmultiplied(
                node.color.r(),
                node.color.g(),
                node.color.b(),
                (glow_alpha as f32 * 255.0) as u8,
            );
            painter.circle_filled(pos, glow_radius, glow_color);
        }

        // Middle glow layer (compact and above)
        let mid_glow_radius = radius * 1.15;
        let mid_glow_color = Color32::from_rgba_unmultiplied(
            node.color.r(),
            node.color.g(),
            node.color.b(),
            60 + (glow * 40.0) as u8,
        );
        painter.circle_filled(pos, mid_glow_radius, mid_glow_color);

        // Core circle
        let core_color = if is_hovered {
            astronomy_colors::brighten(node.color, 1.2)
        } else {
            node.color
        };
        painter.circle_filled(pos, radius, core_color);

        // Border
        let border_color = if is_hovered {
            Color32::WHITE
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 100)
        };
        painter.circle_stroke(pos, radius, Stroke::new(1.5, border_color));

        // Label (switch between short and full based on compression/zoom)
        let label = if compression > 0.5 || radius < 30.0 {
            &node.short_label
        } else {
            &node.label
        };

        let font_size = (12.0 + radius * 0.15).min(18.0);
        let text_color = if is_hovered {
            Color32::WHITE
        } else {
            Color32::from_rgb(220, 220, 220)
        };

        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(font_size),
            text_color,
        );

        // Count badge (below label, if not too compressed)
        if compression < 0.7 && radius > 25.0 {
            if let Some(cluster) = self.clusters.get(&node.id) {
                let count_text = format!("{}", cluster.cbu_count);
                painter.text(
                    pos + Vec2::new(0.0, font_size * 0.8),
                    egui::Align2::CENTER_CENTER,
                    count_text,
                    egui::FontId::proportional(10.0),
                    Color32::from_rgb(180, 180, 180),
                );
            }
        }
    }

    /// Render atmosphere particles (Phase 8 Polish)
    ///
    /// Particles drift in the background, creating depth atmosphere.
    /// Density and appearance vary by depth level.
    fn render_particles(&self, painter: &Painter, camera: &Camera2D, screen_rect: Rect) {
        let config = ParticleConfig::for_depth(self.depth_level);

        for particle in &self.particles {
            let screen_pos =
                camera.world_to_screen(Pos2::new(particle.pos.x, particle.pos.y), screen_rect);

            // Skip particles outside visible area (with margin)
            let margin = 50.0;
            if screen_pos.x < screen_rect.left() - margin
                || screen_pos.x > screen_rect.right() + margin
                || screen_pos.y < screen_rect.top() - margin
                || screen_pos.y > screen_rect.bottom() + margin
            {
                continue;
            }

            // Calculate opacity with depth config and shimmer
            let opacity = particle.current_opacity() * config.opacity_mult;
            let alpha = (opacity * 255.0) as u8;

            // Apply color tint from config
            let [r, g, b, _] = config.color.to_array();
            let color = Color32::from_rgba_unmultiplied(r, g, b, alpha);

            // Scale size by camera zoom
            let size = particle.size * camera.zoom();

            // Draw particle as a soft circle (glow effect)
            if size > 0.5 {
                // Outer glow (larger, more transparent)
                let glow_color = Color32::from_rgba_unmultiplied(r, g, b, alpha / 3);
                painter.circle_filled(screen_pos, size * 2.0, glow_color);

                // Core (smaller, brighter)
                painter.circle_filled(screen_pos, size, color);
            }
        }
    }

    fn render_title(&self, painter: &Painter, screen_rect: Rect) {
        let total_cbus: usize = self.clusters.values().map(|c| c.cbu_count).sum();
        let title = format!("Client Universe ({} CBUs)", total_cbus);

        painter.text(
            screen_rect.center_top() + Vec2::new(0.0, 30.0),
            egui::Align2::CENTER_TOP,
            title,
            egui::FontId::proportional(20.0),
            Color32::from_rgb(200, 200, 200),
        );
    }

    /// Render branch fan-out around a hovered node
    ///
    /// Displays up to 6 preview items arranged in a fan pattern around the parent node.
    /// Each branch animates in with staggered timing using branch_springs.
    fn render_branches(
        &self,
        painter: &Painter,
        parent_pos: Pos2,
        parent_radius: f32,
        _camera: &Camera2D,
    ) {
        // Only render if we have preview data
        let preview = match &self.loiter_state.preview {
            Some(p) => p,
            None => {
                // Show loading indicator if still fetching
                if self.loiter_state.loading {
                    self.render_loading_indicator(painter, parent_pos, parent_radius);
                }
                return;
            }
        };

        let items = &preview.items;
        if items.is_empty() {
            return;
        }

        // Limit to 6 branches for visual clarity
        let branch_count = items.len().min(6);
        let highlighted = self.loiter_state.highlighted_branch;

        // Fan angle configuration
        // Branches fan out above the node in a 180° arc
        let fan_start = -std::f32::consts::PI; // -180° (left)
        let fan_end = 0.0; // 0° (right)
        let fan_range = fan_end - fan_start;

        // Distance from parent center to branch center
        let branch_distance = parent_radius * 2.5;
        let branch_radius = parent_radius * 0.5;

        for (i, item) in items.iter().take(branch_count).enumerate() {
            // Get animation progress for this branch
            let progress = self.branch_progress(i);
            if progress < 0.01 {
                continue; // Not yet visible
            }

            // Calculate angle for this branch (evenly distributed in fan)
            let angle = if branch_count == 1 {
                -std::f32::consts::FRAC_PI_2 // Single branch points up
            } else {
                fan_start + fan_range * (i as f32 / (branch_count - 1) as f32)
            };

            // Calculate position with animation (starts at parent, moves outward)
            let target_offset = Vec2::new(angle.cos(), angle.sin()) * branch_distance;
            let current_offset = target_offset * progress;
            let branch_pos = parent_pos + current_offset;

            // Determine if this branch is highlighted
            let is_highlighted = highlighted == Some(i);

            // Render connecting line (fades in with progress)
            let line_alpha = (progress * 0.6 * 255.0) as u8;
            let line_color = if is_highlighted {
                Color32::from_rgba_unmultiplied(255, 255, 255, line_alpha)
            } else {
                Color32::from_rgba_unmultiplied(150, 150, 150, line_alpha)
            };
            painter.line_segment(
                [parent_pos, branch_pos],
                Stroke::new(1.5 * progress, line_color),
            );

            // Get color based on preview type
            let base_color = self.color_for_preview_type(&item.preview_type);
            let branch_color = if is_highlighted {
                astronomy_colors::brighten(base_color, 1.3)
            } else {
                base_color
            };

            // Render branch node (scales up with progress)
            let current_radius = branch_radius * progress;
            let node_alpha = (progress * 255.0) as u8;

            // Outer glow
            let glow_radius = current_radius * 1.4;
            let glow_color = Color32::from_rgba_unmultiplied(
                branch_color.r(),
                branch_color.g(),
                branch_color.b(),
                (node_alpha as f32 * 0.3) as u8,
            );
            painter.circle_filled(branch_pos, glow_radius, glow_color);

            // Core circle
            let core_alpha_color = Color32::from_rgba_unmultiplied(
                branch_color.r(),
                branch_color.g(),
                branch_color.b(),
                node_alpha,
            );
            painter.circle_filled(branch_pos, current_radius, core_alpha_color);

            // Border (highlighted branches get white border)
            let border_color = if is_highlighted {
                Color32::from_rgba_unmultiplied(255, 255, 255, node_alpha)
            } else {
                Color32::from_rgba_unmultiplied(200, 200, 200, (node_alpha as f32 * 0.5) as u8)
            };
            painter.circle_stroke(branch_pos, current_radius, Stroke::new(1.0, border_color));

            // Label (only show when sufficiently visible)
            if progress > 0.5 {
                let label_alpha = ((progress - 0.5) * 2.0 * 255.0) as u8;
                let label_color = if is_highlighted {
                    Color32::from_rgba_unmultiplied(255, 255, 255, label_alpha)
                } else {
                    Color32::from_rgba_unmultiplied(200, 200, 200, label_alpha)
                };

                // Truncate label if too long
                let label = if item.label.len() > 15 {
                    format!("{}…", &item.label[..14])
                } else {
                    item.label.clone()
                };

                painter.text(
                    branch_pos + Vec2::new(0.0, current_radius + 8.0),
                    egui::Align2::CENTER_TOP,
                    label,
                    egui::FontId::proportional(10.0),
                    label_color,
                );

                // Show count if available
                if let Some(count) = item.count {
                    painter.text(
                        branch_pos,
                        egui::Align2::CENTER_CENTER,
                        format!("{}", count),
                        egui::FontId::proportional(9.0),
                        label_color,
                    );
                }
            }
        }

        // Show "more" indicator if there are additional items beyond 6
        if items.len() > 6 {
            let more_count = items.len() - 6;
            let more_text = format!("+{} more", more_count);
            let more_pos = parent_pos + Vec2::new(0.0, parent_radius + branch_distance + 20.0);
            painter.text(
                more_pos,
                egui::Align2::CENTER_CENTER,
                more_text,
                egui::FontId::proportional(9.0),
                Color32::from_rgb(150, 150, 150),
            );
        }
    }

    /// Render a loading indicator while preview is being fetched
    fn render_loading_indicator(&self, painter: &Painter, pos: Pos2, radius: f32) {
        // Simple pulsing ring
        let pulse = (self.loiter_state.duration * 4.0).sin() * 0.5 + 0.5;
        let ring_radius = radius * (1.5 + pulse * 0.3);
        let ring_alpha = (pulse * 100.0) as u8;
        let ring_color = Color32::from_rgba_unmultiplied(200, 200, 255, ring_alpha);
        painter.circle_stroke(pos, ring_radius, Stroke::new(2.0, ring_color));
    }

    /// Get color for a preview type
    fn color_for_preview_type(&self, preview_type: &PreviewType) -> Color32 {
        match preview_type {
            PreviewType::Cluster => Color32::from_rgb(100, 149, 237), // Cornflower blue
            PreviewType::Cbu => Color32::from_rgb(144, 238, 144),     // Light green
            PreviewType::Entity => Color32::from_rgb(255, 182, 193),  // Light pink
            PreviewType::Document => Color32::from_rgb(255, 218, 185), // Peach
            PreviewType::Workflow => Color32::from_rgb(221, 160, 221), // Plum
            PreviewType::Product => Color32::from_rgb(135, 206, 250), // Light sky blue
            PreviewType::Anomaly => Color32::from_rgb(255, 99, 71),   // Tomato red
        }
    }

    // =========================================================================
    // PHASE 4: EXPANSION RENDERING
    // =========================================================================

    /// Render expanded children for a node
    ///
    /// Uses animation phases for organic growth:
    /// - Budding (0-20%): Small dot appears
    /// - Sprouting (20-50%): Growing outward
    /// - Unfurling (50-80%): Reaching full size
    /// - Settling (80-100%): Micro-adjustments
    fn render_expansion(
        &self,
        painter: &Painter,
        parent_pos: Pos2,
        parent_radius: f32,
        _node_id: &str,
        expansion: &ExpansionState,
    ) {
        let progress = expansion.progress;
        let children = &expansion.children;

        if children.is_empty() {
            return;
        }

        let child_count = children.len().min(8); // Max 8 children in expansion view
        let expansion_distance = parent_radius * 3.0 * progress;
        let child_radius = parent_radius * 0.4 * progress.sqrt(); // Sqrt for organic growth

        // Color based on expansion type
        let base_color = self.color_for_expansion_type(&expansion.expansion_type);

        // Arrange children in a circle around parent
        for (i, child_id) in children.iter().take(child_count).enumerate() {
            // Calculate angle for this child (full circle distribution)
            let angle = (i as f32 / child_count as f32) * std::f32::consts::TAU
                - std::f32::consts::FRAC_PI_2; // Start at top

            // Apply stagger based on animation phase
            let stagger = 1.0 - (i as f32 * 0.1).min(0.5);
            let child_progress = (progress * stagger).min(1.0);

            if child_progress < 0.05 {
                continue; // Not visible yet
            }

            // Calculate position
            let offset = Vec2::new(angle.cos(), angle.sin()) * expansion_distance * child_progress;
            let child_pos = parent_pos + offset;

            // Draw connecting line (fades in during Sprouting phase)
            let line_alpha = ((progress - 0.2) * 2.5).clamp(0.0, 1.0);
            if line_alpha > 0.0 {
                let line_color = Color32::from_rgba_unmultiplied(
                    base_color.r(),
                    base_color.g(),
                    base_color.b(),
                    (line_alpha * 100.0) as u8,
                );
                painter.line_segment([parent_pos, child_pos], Stroke::new(1.5, line_color));
            }

            // Draw child node with growth phases
            let current_radius = child_radius * child_progress;

            // Glow (appears in Unfurling phase)
            let glow_alpha = ((progress - 0.5) * 3.0).clamp(0.0, 0.4);
            if glow_alpha > 0.0 {
                let glow_color = Color32::from_rgba_unmultiplied(
                    base_color.r(),
                    base_color.g(),
                    base_color.b(),
                    (glow_alpha * 80.0) as u8,
                );
                painter.circle_filled(child_pos, current_radius * 1.5, glow_color);
            }

            // Main circle
            let fill_alpha = (child_progress * 255.0) as u8;
            let fill_color = Color32::from_rgba_unmultiplied(
                base_color.r(),
                base_color.g(),
                base_color.b(),
                fill_alpha,
            );
            painter.circle_filled(child_pos, current_radius, fill_color);

            // Border (appears in Settling phase)
            let border_alpha = ((progress - 0.8) * 5.0).clamp(0.0, 1.0);
            if border_alpha > 0.0 {
                let border_color =
                    Color32::from_rgba_unmultiplied(255, 255, 255, (border_alpha * 200.0) as u8);
                painter.circle_stroke(child_pos, current_radius, Stroke::new(1.0, border_color));
            }

            // Label (appears when Visible)
            if progress > 0.9 {
                let label_alpha = ((progress - 0.9) * 10.0).clamp(0.0, 1.0);
                let label_color =
                    Color32::from_rgba_unmultiplied(255, 255, 255, (label_alpha * 200.0) as u8);

                // Truncate child_id for display
                let label = if child_id.len() > 12 {
                    format!("{}...", &child_id[..10])
                } else {
                    child_id.clone()
                };

                painter.text(
                    child_pos + Vec2::new(0.0, current_radius + 8.0),
                    egui::Align2::CENTER_TOP,
                    label,
                    egui::FontId::proportional(10.0),
                    label_color,
                );
            }
        }

        // Show "+N more" if more than 8 children
        if children.len() > 8 && progress > 0.9 {
            let more_count = children.len() - 8;
            let label_alpha = ((progress - 0.9) * 10.0).clamp(0.0, 1.0);
            let label_color =
                Color32::from_rgba_unmultiplied(200, 200, 200, (label_alpha * 180.0) as u8);

            painter.text(
                parent_pos + Vec2::new(0.0, parent_radius + expansion_distance + 20.0),
                egui::Align2::CENTER_TOP,
                format!("+{} more", more_count),
                egui::FontId::proportional(11.0),
                label_color,
            );
        }
    }

    /// Render focus indicator around a focused node
    fn render_focus_indicator(&self, painter: &Painter, pos: Pos2, radius: f32) {
        // Animated dashed ring
        let ring_radius = radius * 1.3;
        let focus_color = Color32::from_rgb(255, 215, 0); // Gold

        // Draw multiple arcs to create dashed effect
        let dash_count = 8;
        let dash_length = std::f32::consts::TAU / (dash_count as f32 * 2.0);

        for i in 0..dash_count {
            let start_angle = (i as f32 / dash_count as f32) * std::f32::consts::TAU;
            let end_angle = start_angle + dash_length;

            let start = pos + Vec2::new(start_angle.cos(), start_angle.sin()) * ring_radius;
            let mid = pos
                + Vec2::new(
                    ((start_angle + end_angle) / 2.0).cos(),
                    ((start_angle + end_angle) / 2.0).sin(),
                ) * ring_radius;
            let end = pos + Vec2::new(end_angle.cos(), end_angle.sin()) * ring_radius;

            // Draw arc as line segments
            painter.line_segment([start, mid], Stroke::new(2.0, focus_color));
            painter.line_segment([mid, end], Stroke::new(2.0, focus_color));
        }
    }

    /// Render keyboard selection indicator (Phase 8: Accessibility)
    ///
    /// Shows a high-contrast ring with corner brackets for keyboard-driven selection.
    /// Distinct from mouse hover (glow) and soft focus (dashed gold ring).
    fn render_keyboard_selection(&self, painter: &Painter, pos: Pos2, radius: f32) {
        let ring_radius = radius * 1.4;
        // High-contrast cyan for accessibility (visible against dark backgrounds)
        let selection_color = Color32::from_rgb(0, 255, 255);

        // Solid ring
        painter.circle_stroke(pos, ring_radius, Stroke::new(3.0, selection_color));

        // Corner brackets for additional visual distinction
        let bracket_size = radius * 0.4;
        let bracket_offset = ring_radius + 8.0;

        // Four corners: top-left, top-right, bottom-left, bottom-right
        let corners = [
            (
                Vec2::new(-1.0, -1.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0),
            ),
            (
                Vec2::new(1.0, -1.0),
                Vec2::new(-1.0, 0.0),
                Vec2::new(0.0, 1.0),
            ),
            (
                Vec2::new(-1.0, 1.0),
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, -1.0),
            ),
            (
                Vec2::new(1.0, 1.0),
                Vec2::new(-1.0, 0.0),
                Vec2::new(0.0, -1.0),
            ),
        ];

        for (dir, h_dir, v_dir) in corners {
            let corner = pos + dir * bracket_offset;
            let h_end = corner + h_dir * bracket_size;
            let v_end = corner + v_dir * bracket_size;

            painter.line_segment([corner, h_end], Stroke::new(2.0, selection_color));
            painter.line_segment([corner, v_end], Stroke::new(2.0, selection_color));
        }
    }

    /// Render focus breadcrumbs at top of screen
    fn render_focus_breadcrumbs(&self, painter: &Painter, screen_rect: Rect) {
        let breadcrumbs = self.focus_stack.breadcrumbs();
        if breadcrumbs.is_empty() {
            return;
        }

        let bg_color = Color32::from_rgba_unmultiplied(0, 0, 0, 180);
        let text_color = Color32::from_rgb(255, 255, 255);
        let separator_color = Color32::from_rgb(150, 150, 150);

        // Build breadcrumb string
        let breadcrumb_text = breadcrumbs.join(" > ");

        // Position at top center
        let pos = Pos2::new(screen_rect.center().x, screen_rect.top() + 30.0);

        // Measure text
        let font = egui::FontId::proportional(12.0);

        // Draw background pill
        let text_width = breadcrumb_text.len() as f32 * 7.0; // Approximate
        let padding = 12.0;
        let pill_rect = Rect::from_center_size(pos, Vec2::new(text_width + padding * 2.0, 24.0));
        painter.rect_filled(pill_rect, 12.0, bg_color);

        // Draw text
        painter.text(
            pos,
            egui::Align2::CENTER_CENTER,
            &breadcrumb_text,
            font,
            text_color,
        );

        // Draw "< Back" hint on left
        let back_pos = Pos2::new(pill_rect.left() - 50.0, pos.y);
        painter.text(
            back_pos,
            egui::Align2::CENTER_CENTER,
            "< Back",
            egui::FontId::proportional(11.0),
            separator_color,
        );
    }

    /// Get color for expansion type
    fn color_for_expansion_type(&self, expansion_type: &ExpansionType) -> Color32 {
        match expansion_type {
            ExpansionType::Ownership => Color32::from_rgb(100, 200, 100), // Green
            ExpansionType::Control => Color32::from_rgb(200, 100, 100),   // Red
            ExpansionType::Documents => Color32::from_rgb(255, 218, 185), // Peach
            ExpansionType::Workflow => Color32::from_rgb(221, 160, 221),  // Plum
            ExpansionType::Roles => Color32::from_rgb(100, 149, 237),     // Cornflower
            ExpansionType::Children => Color32::from_rgb(200, 200, 200),  // Gray
        }
    }

    // =========================================================================
    // PHASE 5: Agent Intelligence Rendering
    // =========================================================================

    /// Render anomaly badge at node position (Phase 5)
    ///
    /// Badge appears in top-right with pulsing animation based on severity
    fn render_anomaly_badge(
        &self,
        painter: &Painter,
        pos: Pos2,
        radius: f32,
        anomalies: &[Anomaly],
    ) {
        // Find highest severity anomaly for badge color
        let max_severity = anomalies
            .iter()
            .map(|a| &a.severity)
            .max_by_key(|s| match s {
                AnomalySeverity::Info => 0,
                AnomalySeverity::Low => 1,
                AnomalySeverity::Medium => 2,
                AnomalySeverity::High => 3,
                AnomalySeverity::Critical => 4,
            })
            .unwrap_or(&AnomalySeverity::Info);

        // Badge position: top-right of node
        let badge_radius = 8.0;
        let badge_pos = pos + Vec2::new(radius * 0.7, -radius * 0.7);

        // Color based on severity with pulse
        let pulse = self.anomaly_pulse_value();
        let base_color = match max_severity {
            AnomalySeverity::Info => Color32::from_rgb(100, 149, 237), // Blue
            AnomalySeverity::Low => Color32::from_rgb(76, 175, 80),    // Green
            AnomalySeverity::Medium => Color32::from_rgb(255, 193, 7), // Amber
            AnomalySeverity::High => Color32::from_rgb(244, 67, 54),   // Red
            AnomalySeverity::Critical => Color32::from_rgb(183, 28, 28), // Dark red
        };

        // Apply pulse to alpha
        let pulsed_alpha = (pulse * 255.0) as u8;
        let badge_color = Color32::from_rgba_unmultiplied(
            base_color.r(),
            base_color.g(),
            base_color.b(),
            pulsed_alpha,
        );

        // Outer glow
        let glow_radius = badge_radius * (1.0 + pulse * 0.3);
        let glow_color = Color32::from_rgba_unmultiplied(
            base_color.r(),
            base_color.g(),
            base_color.b(),
            (pulse * 100.0) as u8,
        );
        painter.circle_filled(badge_pos, glow_radius + 4.0, glow_color);

        // Badge circle
        painter.circle_filled(badge_pos, badge_radius, badge_color);
        painter.circle_stroke(badge_pos, badge_radius, Stroke::new(1.5, Color32::WHITE));

        // Count indicator
        let count = anomalies.len();
        if count > 1 {
            painter.text(
                badge_pos,
                egui::Align2::CENTER_CENTER,
                format!("{}", count.min(9)),
                egui::FontId::proportional(10.0),
                Color32::WHITE,
            );
        } else {
            // Single anomaly: show "!" icon
            painter.text(
                badge_pos,
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );
        }
    }

    /// Render agent speech bubble (Phase 5)
    fn render_agent_speech(
        &self,
        painter: &Painter,
        screen_rect: Rect,
        speech: &AgentSpeech,
        camera: &Camera2D,
    ) {
        let opacity = self.speech_opacity.get();
        if opacity < 0.01 {
            return;
        }

        // Determine position
        let pos = if let Some(ref anchor_id) = speech.anchor_node_id {
            // Position near anchor node
            if let Some(node) = self.simulation.get_node(anchor_id) {
                let node_screen = camera.world_to_screen(node.position, screen_rect);
                // Position above and to the right
                Pos2::new(node_screen.x + 60.0, node_screen.y - 60.0)
            } else {
                // Fallback to bottom-right corner
                Pos2::new(screen_rect.right() - 200.0, screen_rect.bottom() - 100.0)
            }
        } else {
            // Default: bottom-right corner
            Pos2::new(screen_rect.right() - 200.0, screen_rect.bottom() - 100.0)
        };

        // Bubble dimensions
        let max_width = 280.0;
        let padding = 12.0;
        let line_height = 16.0;

        // Word wrap text
        let words: Vec<&str> = speech.text.split_whitespace().collect();
        let mut lines: Vec<String> = Vec::new();
        let mut current_line = String::new();

        for word in words {
            let test_line = if current_line.is_empty() {
                word.to_string()
            } else {
                format!("{} {}", current_line, word)
            };
            let test_width = test_line.len() as f32 * 7.0; // Approximate
            if test_width > max_width - padding * 2.0 {
                if !current_line.is_empty() {
                    lines.push(current_line);
                }
                current_line = word.to_string();
            } else {
                current_line = test_line;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        let text_height = lines.len() as f32 * line_height;
        let bubble_width = max_width;
        let bubble_height = text_height + padding * 2.0;

        // Colors based on urgency
        let (bg_color, border_color, text_color) = match speech.urgency {
            SpeechUrgency::Info => (
                Color32::from_rgba_unmultiplied(40, 40, 60, (200.0 * opacity) as u8),
                Color32::from_rgba_unmultiplied(100, 149, 237, (255.0 * opacity) as u8),
                Color32::from_rgba_unmultiplied(220, 220, 255, (255.0 * opacity) as u8),
            ),
            SpeechUrgency::Suggestion => (
                Color32::from_rgba_unmultiplied(40, 60, 40, (200.0 * opacity) as u8),
                Color32::from_rgba_unmultiplied(100, 200, 100, (255.0 * opacity) as u8),
                Color32::from_rgba_unmultiplied(220, 255, 220, (255.0 * opacity) as u8),
            ),
            SpeechUrgency::Important => (
                Color32::from_rgba_unmultiplied(60, 50, 20, (200.0 * opacity) as u8),
                Color32::from_rgba_unmultiplied(255, 193, 7, (255.0 * opacity) as u8),
                Color32::from_rgba_unmultiplied(255, 255, 220, (255.0 * opacity) as u8),
            ),
            SpeechUrgency::Warning => (
                Color32::from_rgba_unmultiplied(60, 30, 30, (200.0 * opacity) as u8),
                Color32::from_rgba_unmultiplied(244, 67, 54, (255.0 * opacity) as u8),
                Color32::from_rgba_unmultiplied(255, 220, 220, (255.0 * opacity) as u8),
            ),
        };

        // Draw bubble
        let bubble_rect = Rect::from_min_size(pos, Vec2::new(bubble_width, bubble_height));
        painter.rect_filled(bubble_rect, 8.0, bg_color);
        painter.rect_stroke(bubble_rect, 8.0, Stroke::new(2.0, border_color));

        // Draw tail (small triangle pointing left)
        let tail_points = [
            Pos2::new(pos.x - 8.0, pos.y + bubble_height / 2.0),
            Pos2::new(pos.x, pos.y + bubble_height / 2.0 - 6.0),
            Pos2::new(pos.x, pos.y + bubble_height / 2.0 + 6.0),
        ];
        painter.add(egui::Shape::convex_polygon(
            tail_points.to_vec(),
            bg_color,
            Stroke::new(2.0, border_color),
        ));

        // Draw text lines
        let font = egui::FontId::proportional(13.0);
        for (i, line) in lines.iter().enumerate() {
            let line_pos = Pos2::new(
                pos.x + padding,
                pos.y + padding + (i as f32 * line_height) + line_height / 2.0,
            );
            painter.text(
                line_pos,
                egui::Align2::LEFT_CENTER,
                line,
                font.clone(),
                text_color,
            );
        }
    }

    // =========================================================================
    // QUERIES
    // =========================================================================

    /// Check if simulation needs repaint
    pub fn needs_repaint(&self) -> bool {
        !self.simulation.is_stable()
            || self.glow_springs.values().any(|s| s.is_animating())
            || self.loiter_needs_repaint()
            || self.expansion_needs_repaint()
            || self.agent_needs_repaint()
    }

    /// Check if agent animations need repaint (Phase 5)
    fn agent_needs_repaint(&self) -> bool {
        // Anomaly pulse is continuous when there are anomalies
        !self.node_anomalies.is_empty() || self.speech_opacity.is_animating()
    }

    /// Get cluster count
    pub fn cluster_count(&self) -> usize {
        self.clusters.len()
    }

    /// Get total CBU count
    pub fn total_cbu_count(&self) -> usize {
        self.clusters.values().map(|c| c.cbu_count).sum()
    }

    // =========================================================================
    // NAVIGATION CONVENIENCE METHODS (called from app.rs update loop)
    // =========================================================================

    /// Set universe data from API response
    /// Convenience wrapper around load_from_universe_graph
    pub fn set_universe_data(&mut self, graph: &UniverseGraph) {
        self.load_from_universe_graph(graph);
    }

    /// Handle drill into a cluster
    /// For now, just marks the cluster as selected (visual feedback)
    /// The actual data fetching is handled by app.rs via API calls
    pub fn drill_into_cluster(&mut self, cluster_id: &str) {
        // Set the hovered state to give visual feedback
        self.hovered = Some(cluster_id.to_string());
        // Could trigger animation here in future
    }

    /// Return to universe view (reset state)
    pub fn return_to_universe(&mut self) {
        // Clear selection state
        self.hovered = None;
        // Give simulation a kick to re-animate
        if self.has_data {
            self.simulation.kick();
        }
    }

    /// Get cluster by ID
    pub fn get_cluster(&self, cluster_id: &str) -> Option<&ClusterData> {
        self.clusters.get(cluster_id)
    }

    /// Get all cluster IDs
    pub fn cluster_ids(&self) -> Vec<String> {
        self.clusters.keys().cloned().collect()
    }

    // =========================================================================
    // LOITER STATE METHODS (Phase 3 - Fork Presentation)
    // =========================================================================

    /// Check if loiter threshold was crossed and preview fetch is needed
    ///
    /// Returns the node ID if we should fetch preview data
    pub fn needs_preview_fetch(&self) -> Option<&str> {
        if self.loiter_state.branches_visible
            && !self.loiter_state.loading
            && self.loiter_state.preview.is_none()
        {
            self.loiter_state.node_id.as_deref()
        } else {
            None
        }
    }

    /// Mark preview as loading (prevents duplicate fetches)
    pub fn set_preview_loading(&mut self) {
        self.loiter_state.loading = true;
    }

    /// Set preview data from API response
    ///
    /// Creates branch springs for animation
    pub fn set_preview_data(&mut self, preview: PreviewData) {
        // Create springs for each branch (start at 0, animate to 1)
        self.branch_springs = preview.items.iter().map(|_| SpringF32::new(0.0)).collect();

        self.loiter_state.preview = Some(preview);
        self.loiter_state.loading = false;
    }

    /// Clear preview data (e.g., on error)
    pub fn clear_preview(&mut self) {
        self.loiter_state.preview = None;
        self.loiter_state.loading = false;
        self.branch_springs.clear();
    }

    /// Check if branches are visible (for rendering)
    pub fn branches_visible(&self) -> bool {
        self.loiter_state.branches_visible
    }

    /// Get current preview data (for rendering)
    pub fn get_preview(&self) -> Option<&PreviewData> {
        self.loiter_state.preview.as_ref()
    }

    /// Get highlighted branch index (for rendering)
    pub fn highlighted_branch(&self) -> Option<usize> {
        self.loiter_state.highlighted_branch
    }

    /// Get branch animation progress for a specific branch
    pub fn branch_progress(&self, index: usize) -> f32 {
        self.branch_springs
            .get(index)
            .map(|s| s.get())
            .unwrap_or(0.0)
    }

    /// Get the node ID being loitered on
    pub fn loitering_node_id(&self) -> Option<&str> {
        self.loiter_state.node_id.as_deref()
    }

    /// Check if loiter state needs repaint
    pub fn loiter_needs_repaint(&self) -> bool {
        self.branch_springs.iter().any(|s| s.is_animating())
            || (self.loiter_state.node_id.is_some() && !self.loiter_state.branches_visible)
    }

    // =========================================================================
    // PHASE 4: FOCUS AND EXPANSION
    // =========================================================================

    /// Push a focus frame onto the stack (soft focus without navigation)
    ///
    /// Returns false if at max depth. Follows egui-rules: returns action result,
    /// caller handles side effects.
    pub fn push_focus(&mut self, node_id: String, node_type: String, label: String) -> bool {
        let frame = FocusFrame {
            node_id,
            node_type,
            label,
            expansion: None,
            focused_at: None,
        };
        self.focus_stack.push(frame)
    }

    /// Push a focus frame with expansion type
    pub fn push_focus_with_expansion(
        &mut self,
        node_id: String,
        node_type: String,
        label: String,
        expansion_type: ExpansionType,
    ) -> bool {
        let frame = FocusFrame {
            node_id,
            node_type,
            label,
            expansion: Some(expansion_type),
            focused_at: None,
        };
        self.focus_stack.push(frame)
    }

    /// Pop the top focus frame
    pub fn pop_focus(&mut self) -> Option<FocusFrame> {
        let frame = self.focus_stack.pop();

        // If we popped a frame with expansion, start collapsing it
        if let Some(ref f) = frame {
            if f.expansion.is_some() {
                if let Some(state) = self.expansions.get_mut(&f.node_id) {
                    state.collapse();
                }
            }
        }

        frame
    }

    /// Get the current focus (topmost frame)
    pub fn current_focus(&self) -> Option<&FocusFrame> {
        self.focus_stack.current()
    }

    /// Check if a node is in the focus stack
    pub fn is_focused(&self, node_id: &str) -> bool {
        self.focus_stack.contains(node_id)
    }

    /// Get focus depth
    pub fn focus_depth(&self) -> usize {
        self.focus_stack.depth()
    }

    /// Get breadcrumbs from focus stack
    pub fn focus_breadcrumbs(&self) -> Vec<&str> {
        self.focus_stack.breadcrumbs()
    }

    /// Clear focus stack
    pub fn clear_focus(&mut self) {
        // Collapse all expansions first
        for state in self.expansions.values_mut() {
            state.collapse();
        }
        self.focus_stack.clear();
    }

    /// Start expanding a node to show children/details
    ///
    /// Creates an ExpansionState and starts the animation.
    /// Returns true if expansion was started (not already expanding).
    pub fn expand_node(
        &mut self,
        node_id: String,
        expansion_type: ExpansionType,
        children: Vec<String>,
    ) -> bool {
        // Don't expand if already expanding
        if let Some(state) = self.expansions.get(&node_id) {
            if state.target_expanded {
                return false;
            }
        }

        // Create expansion state
        let state = ExpansionState::expanding(expansion_type, children);
        self.expansions.insert(node_id.clone(), state);

        // Create animation spring (starts at 0, animates to 1)
        self.expansion_springs.insert(node_id, SpringF32::new(0.0));

        true
    }

    /// Collapse a node's expansion
    pub fn collapse_node(&mut self, node_id: &str) {
        if let Some(state) = self.expansions.get_mut(node_id) {
            state.collapse();
        }
    }

    /// Toggle expansion state for a node
    pub fn toggle_expansion(
        &mut self,
        node_id: String,
        expansion_type: ExpansionType,
        children: Vec<String>,
    ) -> bool {
        if let Some(state) = self.expansions.get(&node_id) {
            if state.target_expanded {
                self.collapse_node(&node_id);
                return false; // Now collapsing
            }
        }
        self.expand_node(node_id, expansion_type, children)
    }

    /// Get expansion state for a node
    pub fn get_expansion(&self, node_id: &str) -> Option<&ExpansionState> {
        self.expansions.get(node_id)
    }

    /// Get expansion progress for a node (0.0 = collapsed, 1.0 = expanded)
    pub fn expansion_progress(&self, node_id: &str) -> f32 {
        self.expansion_springs
            .get(node_id)
            .map(|s| s.get())
            .unwrap_or(0.0)
    }

    /// Check if any expansions are animating
    pub fn expansions_animating(&self) -> bool {
        self.expansion_springs.values().any(|s| s.is_animating())
    }

    /// Check if expansion needs repaint
    pub fn expansion_needs_repaint(&self) -> bool {
        self.expansions_animating()
    }

    /// Update expansion animations (call in tick())
    fn tick_expansions(&mut self, dt: f32) {
        // Update each expansion spring
        for (node_id, spring) in self.expansion_springs.iter_mut() {
            if let Some(state) = self.expansions.get_mut(node_id) {
                // Set target based on expansion state
                let target = if state.target_expanded { 1.0 } else { 0.0 };
                spring.set_target(target);
                spring.tick(dt);

                // Update progress and phase
                state.progress = spring.get();
                state.update_phase();
            }
        }

        // Clean up completed collapses
        let collapsed_ids: Vec<String> = self
            .expansions
            .iter()
            .filter(|(_, state)| state.is_collapsed())
            .map(|(id, _)| id.clone())
            .collect();

        for id in collapsed_ids {
            self.expansions.remove(&id);
            self.expansion_springs.remove(&id);
        }
    }

    // =========================================================================
    // PHASE 5: Agent Intelligence API
    // =========================================================================

    /// Set agent state from API response
    pub fn set_agent_state(&mut self, state: AgentState) {
        // If speech changed, animate opacity
        let had_speech = self.agent_state.speech.is_some();
        let has_speech = state.speech.is_some();

        self.agent_state = state;

        if has_speech && !had_speech {
            // Speech appeared - fade in
            self.speech_opacity.set_target(1.0);
        } else if !has_speech && had_speech {
            // Speech disappeared - fade out
            self.speech_opacity.set_target(0.0);
        }
    }

    /// Get current agent state
    pub fn agent_state(&self) -> &AgentState {
        &self.agent_state
    }

    /// Set anomalies for a specific node
    pub fn set_node_anomalies(&mut self, node_id: String, anomalies: Vec<Anomaly>) {
        if anomalies.is_empty() {
            self.node_anomalies.remove(&node_id);
        } else {
            self.node_anomalies.insert(node_id, anomalies);
        }
    }

    /// Set anomalies for multiple nodes at once (from enriched response)
    pub fn set_all_anomalies(&mut self, anomalies_by_node: HashMap<String, Vec<Anomaly>>) {
        self.node_anomalies = anomalies_by_node;
    }

    /// Clear all anomalies
    pub fn clear_anomalies(&mut self) {
        self.node_anomalies.clear();
    }

    /// Get anomalies for a node
    pub fn get_node_anomalies(&self, node_id: &str) -> Option<&Vec<Anomaly>> {
        self.node_anomalies.get(node_id)
    }

    /// Check if any nodes have anomalies
    pub fn has_anomalies(&self) -> bool {
        !self.node_anomalies.is_empty()
    }

    /// Get total anomaly count across all nodes
    pub fn total_anomaly_count(&self) -> usize {
        self.node_anomalies.values().map(|v| v.len()).sum()
    }

    /// Set prefetch hints from API response
    pub fn set_prefetch_hints(&mut self, hints: Vec<PrefetchHint>) {
        self.prefetch_hints = hints;
    }

    /// Get current prefetch hints
    pub fn prefetch_hints(&self) -> &[PrefetchHint] {
        &self.prefetch_hints
    }

    /// Clear prefetch hints
    pub fn clear_prefetch_hints(&mut self) {
        self.prefetch_hints.clear();
    }

    /// Show agent speech bubble
    ///
    /// Phase 8 verbosity tuning:
    /// - Respects cooldown to prevent speech spam
    /// - Tracks start time for auto-dismiss based on duration_secs
    /// - Per spec: "Never interrupt active navigation"
    pub fn show_speech(&mut self, speech: AgentSpeech) {
        // Check cooldown - don't spam the user
        if self.speech_elapsed < self.speech_cooldown_until {
            return;
        }

        // Record when this speech started (for auto-dismiss timing)
        self.speech_start_time = self.speech_elapsed;

        // Set cooldown: minimum 1 second between speech bubbles
        // Use the greater of 1 second or the speech duration
        let cooldown_duration = speech.duration_secs.max(1.0);
        self.speech_cooldown_until = self.speech_elapsed + cooldown_duration + 0.5;

        self.agent_state.speech = Some(speech);
        self.speech_opacity.set_target(1.0);
    }

    /// Hide agent speech bubble (with fade out animation)
    pub fn hide_speech(&mut self) {
        self.speech_opacity.set_target(0.0);
        // Speech is cleared when opacity reaches 0 (in tick)
    }

    /// Set agent mode
    pub fn set_agent_mode(&mut self, mode: AgentMode) {
        self.agent_state.mode = mode;
    }

    /// Get current agent mode
    pub fn agent_mode(&self) -> AgentMode {
        self.agent_state.mode
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_mock_data() {
        let mut view = GalaxyView::new();
        view.load_mock_data();

        assert!(view.has_data());
        assert_eq!(view.cluster_count(), 6);
        assert!(view.total_cbu_count() > 600);
    }

    #[test]
    fn test_risk_summary_dominant() {
        let summary = RiskSummary {
            low: 100,
            medium: 20,
            high: 5,
            unrated: 0,
        };
        assert_eq!(summary.dominant(), "LOW");

        let summary = RiskSummary {
            low: 10,
            medium: 50,
            high: 5,
            unrated: 0,
        };
        assert_eq!(summary.dominant(), "MEDIUM");
    }

    #[test]
    fn test_clear() {
        let mut view = GalaxyView::new();
        view.load_mock_data();
        assert!(view.has_data());

        view.clear();
        assert!(!view.has_data());
        assert_eq!(view.cluster_count(), 0);
    }
}
