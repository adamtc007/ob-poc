//! OB-POC UI - CBU Graph Visualization
//!
//! A WASM-based egui application for visualizing CBU data.

pub mod agent_panel;
pub mod api;
pub mod app;
pub mod graph_view;

pub use app::ObPocApp;

/// WASM entry point - called from JavaScript
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "ob_poc_canvas",
                web_options,
                Box::new(|cc| Ok(Box::new(ObPocApp::new(cc)))),
            )
            .await
            .expect("Failed to start eframe");
    });
}
