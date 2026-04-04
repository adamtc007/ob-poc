//! Canvas-only state — scene, camera, interaction.
//!
//! Scene data pushed from React via set_scene().
//! Actions sent to React via on_action() callback.

use std::cell::RefCell;

use ob_poc_types::galaxy::ViewLevel;
use ob_poc_types::graph_scene::GraphSceneModel;

// ── Thread-local mailboxes for React↔egui communication ──

thread_local! {
    pub static SCENE_MAILBOX: RefCell<Option<GraphSceneModel>> = RefCell::new(None);
    pub static LEVEL_MAILBOX: RefCell<Option<ViewLevel>> = RefCell::new(None);
    pub static ACTION_CALLBACK: RefCell<Option<js_sys::Function>> = RefCell::new(None);
}

// ── Observation Frame (client-owned) ──

/// Client-owned camera state. NO semantic meaning.
#[derive(Debug, Clone)]
pub struct ObservationFrame {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub target_zoom: f32,
    pub target_pan_x: f32,
    pub target_pan_y: f32,
    pub anchor_node_id: Option<String>,
    pub focus_lock_node_id: Option<String>,
}

impl Default for ObservationFrame {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            target_zoom: 1.0,
            target_pan_x: 0.0,
            target_pan_y: 0.0,
            anchor_node_id: None,
            focus_lock_node_id: None,
        }
    }
}

impl ObservationFrame {
    pub fn is_animating(&self) -> bool {
        let eps = 0.01;
        (self.zoom - self.target_zoom).abs() > eps
            || (self.pan_x - self.target_pan_x).abs() > eps
            || (self.pan_y - self.target_pan_y).abs() > eps
    }
}

// ── Interaction State (ephemeral) ──

#[derive(Debug, Clone, Default)]
pub struct InteractionState {
    pub hovered_node: Option<String>,
    pub selected_node: Option<String>,
}

// ── Canvas App (eframe::App) ──

/// Minimal eframe::App — just the constellation canvas.
/// All structural UI (headers, viewports, dashboard) lives in React.
pub struct CanvasApp {
    pub scene: Option<GraphSceneModel>,
    pub current_level: ViewLevel,
    pub camera: ObservationFrame,
    pub interaction: InteractionState,
}

impl Default for CanvasApp {
    fn default() -> Self {
        Self {
            scene: None,
            current_level: ViewLevel::default(),
            camera: ObservationFrame::default(),
            interaction: InteractionState::default(),
        }
    }
}

impl eframe::App for CanvasApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── 1. Process mailbox (React → egui) ──
        SCENE_MAILBOX.with(|m| {
            if let Some(scene) = m.borrow_mut().take() {
                self.scene = Some(scene);
            }
        });
        LEVEL_MAILBOX.with(|m| {
            if let Some(level) = m.borrow_mut().take() {
                self.current_level = level;
            }
        });

        // ── 2. Tick camera animation ──
        let dt = ctx.input(|i| i.predicted_dt);
        let lerp = 8.0 * dt;
        self.camera.pan_x += (self.camera.target_pan_x - self.camera.pan_x) * lerp;
        self.camera.pan_y += (self.camera.target_pan_y - self.camera.pan_y) * lerp;
        self.camera.zoom += (self.camera.target_zoom - self.camera.zoom) * lerp;

        if self.camera.is_animating() {
            ctx.request_repaint();
        }

        // ── 3. Render canvas (full panel, no shell chrome) ──
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                if let Some(action) = crate::canvas::render(ui, self) {
                    self.process_action(action, ctx);
                }
            });
    }
}

impl CanvasApp {
    fn process_action(&mut self, action: crate::actions::ObservatoryAction, ctx: &egui::Context) {
        use crate::actions::ObservatoryAction;

        match &action {
            // ── Observation frame (local only) ──
            ObservatoryAction::VisualZoom { delta } => {
                let factor = (delta * 0.002).exp();
                self.camera.target_zoom = (self.camera.target_zoom * factor).clamp(0.05, 10.0);
                ctx.request_repaint();
            }
            ObservatoryAction::Pan { dx, dy } => {
                let z = self.camera.zoom;
                self.camera.target_pan_x -= dx / z;
                self.camera.target_pan_y -= dy / z;
                ctx.request_repaint();
            }
            ObservatoryAction::SelectNode { node_id } => {
                self.interaction.selected_node = Some(node_id.clone());
            }
            ObservatoryAction::DeselectNode => {
                self.interaction.selected_node = None;
            }
            ObservatoryAction::AnchorNode { node_id } => {
                self.camera.anchor_node_id = Some(node_id.clone());
            }
            ObservatoryAction::ClearAnchor => {
                self.camera.anchor_node_id = None;
            }
            ObservatoryAction::ResetView => {
                self.camera = ObservationFrame::default();
                ctx.request_repaint();
            }
            // ── Semantic actions → forward to React via callback ──
            _ => {}
        }

        // Forward ALL actions to React via JS callback (React decides what's semantic)
        #[cfg(target_arch = "wasm32")]
        {
            ACTION_CALLBACK.with(|cb| {
                if let Some(ref func) = *cb.borrow() {
                    if let Ok(json) = serde_json::to_string(&action) {
                        let _ = func.call1(
                            &wasm_bindgen::JsValue::NULL,
                            &wasm_bindgen::JsValue::from_str(&json),
                        );
                    }
                }
            });
        }
    }
}
