//! OB-POC UI - CBU Visualization & Agent Interface
//!
//! 4-panel layout:
//! - Graph (top-left): CBU entity visualization with drag/zoom
//! - DSL (top-right): Generated DSL source
//! - Chat (bottom-left): Agent chat interface
//! - AST (bottom-right): Interactive AST with EntityRef resolution

#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::should_implement_trait)]

pub mod api;
pub mod app;
pub mod graph;
pub mod modals;
pub mod panels;
pub mod state;

pub use app::ObPocApp;
pub use graph::CbuGraphWidget;
pub use modals::{EntityFinderModal, EntityFinderResult};
pub use panels::{AstPanel, ChatPanel, DslPanel};
pub use state::{ChatMessage, PendingState, SessionContext};

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    web_sys::console::log_1(&"=== OB-POC UI WASM LOADED (v2) ===".into());

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
