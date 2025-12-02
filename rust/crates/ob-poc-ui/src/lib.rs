//! OB-POC UI - CBU Visualization

#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::should_implement_trait)]

pub mod api;
pub mod app;
pub mod graph;
pub mod graph_view;

pub use app::ObPocApp;
pub use graph::CbuGraphWidget;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "ob_poc_canvas",
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(ObPocApp::new(cc)))),
            )
            .await
            .expect("Failed to start eframe");
    });
}
