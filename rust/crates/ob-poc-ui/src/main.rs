#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(dead_code)]
#![allow(clippy::collapsible_if)]

mod api;
mod app;
mod graph;
mod modals;
mod panels;
mod state;

use app::ObPocApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "OB-POC Visualization",
        native_options,
        Box::new(|cc| Ok(Box::new(ObPocApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
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
