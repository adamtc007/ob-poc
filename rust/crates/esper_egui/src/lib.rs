//! ESPER egui Renderer
//!
//! This crate provides the egui rendering layer for the ESPER navigation system.
//! It follows egui's immediate mode paradigm with strict separation of concerns:
//!
//! - **Update phase**: Physics, animation, input processing (mutates state)
//! - **Render phase**: Pure drawing, read-only state access
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                         ESPER egui Renderer                              │
//! │                                                                          │
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                 │
//! │  │   Camera    │    │   Entity    │    │  Overlay    │                 │
//! │  │  Renderer   │    │  Renderer   │    │  Renderer   │                 │
//! │  └─────────────┘    └─────────────┘    └─────────────┘                 │
//! │         │                  │                  │                         │
//! │         └──────────────────┴──────────────────┘                         │
//! │                            │                                            │
//! │                     ┌──────┴──────┐                                     │
//! │                     │   Painter   │                                     │
//! │                     └─────────────┘                                     │
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```ignore
//! use esper_egui::{EsperRenderer, RenderConfig};
//! use esper_core::DroneState;
//! use esper_snapshot::WorldSnapshot;
//!
//! // In your egui app's update function:
//! fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
//!     let dt = frame.info().cpu_usage.unwrap_or(1.0 / 60.0);
//!
//!     // 1. Update phase: process input, tick physics
//!     self.renderer.update(dt, &mut self.drone_state, &self.world);
//!
//!     // 2. Render phase: draw to egui
//!     egui::CentralPanel::default().show(ctx, |ui| {
//!         self.renderer.render(ui, &self.drone_state, &self.world);
//!     });
//! }
//! ```

pub mod camera;
pub mod config;
pub mod entity;
pub mod error;
pub mod input;
pub mod overlay;
pub mod painter;
pub mod renderer;
pub mod style;

pub use camera::CameraState;
pub use config::RenderConfig;
pub use entity::EntityRenderer;
pub use error::{RenderError, RenderResult};
pub use input::InputBridge;
pub use overlay::OverlayRenderer;
pub use painter::EsperPainter;
pub use renderer::EsperRenderer;
pub use style::RenderStyle;
