//! ob-poc-ui - Full egui/WASM UI for OB-POC
//!
//! This crate implements the complete UI in egui, following the patterns
//! documented in CLAUDE.md:
//!
//! 1. Server is the ONLY source of truth
//! 2. TextBuffers are the ONLY local mutable state
//! 3. Action -> Server -> Refetch -> Render
//! 4. No callbacks, use return values
//! 5. AsyncState coordinates all async operations

mod api;
mod app;
pub mod command;
mod panels;
pub mod state;
pub mod tokens;
#[cfg(target_arch = "wasm32")]
mod voice_bridge;
mod widgets;

pub use app::App;

// =============================================================================
// WASM Entry Points (only compiled for wasm32)
// =============================================================================

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use wasm_bindgen::prelude::*;

    /// Initialize the UI app - does NOT use #[wasm_bindgen(start)] to avoid
    /// conflict with ob-poc-graph's start function.
    #[wasm_bindgen]
    pub fn init_ui() -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        tracing_wasm::set_as_global_default();

        web_sys::console::log_1(&"=== ob-poc-ui WASM initialized ===".into());
        Ok(())
    }

    /// Start the full egui application
    ///
    /// Called from JavaScript after WASM is loaded.
    /// Canvas ID should be the ID of an HTML canvas element.
    #[wasm_bindgen]
    pub fn start_app(canvas_id: &str) -> Result<(), JsValue> {
        web_sys::console::log_1(
            &format!("=== start_app called with canvas_id={} ===", canvas_id).into(),
        );
        // Initialize if not already done
        console_error_panic_hook::set_once();

        let web_options = eframe::WebOptions::default();

        let document = web_sys::window()
            .ok_or_else(|| JsValue::from_str("No window"))?
            .document()
            .ok_or_else(|| JsValue::from_str("No document"))?;

        let canvas: web_sys::HtmlCanvasElement = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("Canvas '{}' not found", canvas_id)))?
            .dyn_into()
            .map_err(|_| JsValue::from_str("Element is not a canvas"))?;

        wasm_bindgen_futures::spawn_local(async move {
            eframe::WebRunner::new()
                .start(
                    canvas,
                    web_options,
                    Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
                )
                .await
                .expect("Failed to start eframe");
        });

        Ok(())
    }
}

// Re-export for wasm_bindgen
#[cfg(target_arch = "wasm32")]
pub use wasm::*;
