//! Observatory WASM — egui constellation canvas, embedded in React shell.
//!
//! React owns structural UI (viewports, headers, dashboard).
//! egui owns the constellation canvas (60fps, same Rust types, no translation).
//! Communication: React pushes scene via set_scene(), egui fires actions via callback.

pub mod actions;
pub mod canvas;
pub mod state;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// Start the constellation canvas on the given HTML canvas element.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start_canvas(canvas_id: &str) -> Result<(), JsValue> {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();

    let web_options = eframe::WebOptions::default();

    let canvas = web_sys::window()
        .and_then(|w: web_sys::Window| w.document())
        .and_then(|d: web_sys::Document| d.get_element_by_id(canvas_id))
        .and_then(|e: web_sys::Element| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .ok_or_else(|| JsValue::from_str(&format!("Canvas '{}' not found", canvas_id)))?;

    eframe::WebRunner::new()
        .start(
            canvas,
            web_options,
            Box::new(|_cc| Ok(Box::new(crate::state::CanvasApp::default()))),
        )
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to start: {e:?}")))
}

/// Push a new GraphSceneModel to the canvas (called by React).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_scene(json: &str) {
    if let Ok(scene) = serde_json::from_str::<ob_poc_types::graph_scene::GraphSceneModel>(json) {
        state::SCENE_MAILBOX.with(|m| {
            *m.borrow_mut() = Some(scene);
        });
        state::EGUI_CTX.with(|c| {
            if let Some(ctx) = c.borrow().as_ref() {
                ctx.request_repaint();
            }
        });
    }
}

/// Push the current view level (called by React when orientation changes).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn set_view_level(level: &str) {
    if let Ok(vl) =
        serde_json::from_str::<ob_poc_types::galaxy::ViewLevel>(&format!("\"{level}\""))
    {
        state::LEVEL_MAILBOX.with(|m| {
            *m.borrow_mut() = Some(vl);
        });
        state::EGUI_CTX.with(|c| {
            if let Some(ctx) = c.borrow().as_ref() {
                ctx.request_repaint();
            }
        });
    }
}

/// Register a JS callback for canvas actions (drill, select, zoom, etc.).
/// React calls this once after start_canvas(). The callback receives JSON-serialized ObservatoryAction.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn on_action(callback: js_sys::Function) {
    state::ACTION_CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(callback);
    });
}
