//! Observatory WASM — full egui/eframe application for SemOS visual rendering.
//!
//! Served in a separate browser tab at /observatory/:sessionId.
//! Consumes OrientationContract, ShowPacket, GraphSceneModel from REST API.
//! Depends on ob-poc-types only (no sem_os_core — avoids tokio/prost blockers).

pub mod actions;
pub mod app;
pub mod canvas;
pub mod fetch;
pub mod panels;
pub mod shell;
pub mod state;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// WASM entry point — called when the module loads.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    log::info!("Observatory WASM module loaded");
}

/// Start the Observatory application on the page.
/// Called from the hosting HTML after the WASM module is loaded.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn start_observatory() -> Result<(), wasm_bindgen::JsValue> {
    let web_options = eframe::WebOptions::default();

    let canvas = web_sys::window()
        .and_then(|w: web_sys::Window| w.document())
        .and_then(|d: web_sys::Document| d.get_element_by_id("observatory_canvas"))
        .and_then(|e: web_sys::Element| e.dyn_into::<web_sys::HtmlCanvasElement>().ok())
        .ok_or_else(|| JsValue::from_str("Canvas element 'observatory_canvas' not found"))?;

    eframe::WebRunner::new()
        .start(
            canvas,
            web_options,
            Box::new(|cc| Ok(Box::new(app::ObservatoryApp::new(cc)))),
        )
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to start: {e:?}")))
}
