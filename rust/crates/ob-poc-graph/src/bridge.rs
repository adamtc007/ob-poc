//! JS Bridge for WASM ↔ HTML communication
//!
//! Uses CustomEvents to communicate between the egui graph and HTML panels.
//! JS dispatches events to window, WASM listens and stores in global state.

use crate::graph::ViewMode;
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use web_sys::{CustomEvent, CustomEventInit, Window};

/// Global state for pending requests from JS (set by event listeners)
#[cfg(target_arch = "wasm32")]
static PENDING_CBU: Mutex<Option<String>> = Mutex::new(None);
#[cfg(target_arch = "wasm32")]
static PENDING_FOCUS: Mutex<Option<String>> = Mutex::new(None);
#[cfg(target_arch = "wasm32")]
static PENDING_VIEW_MODE: Mutex<Option<String>> = Mutex::new(None);

/// Bridge for communication between WASM and HTML
pub struct JsBridge {
    #[cfg(target_arch = "wasm32")]
    window: Window,
}

impl JsBridge {
    pub fn new() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            let window = web_sys::window().expect("No window");
            Self::setup_listeners(&window);
            Self { window }
        }

        #[cfg(not(target_arch = "wasm32"))]
        Self {}
    }

    #[cfg(target_arch = "wasm32")]
    fn setup_listeners(window: &Window) {
        use wasm_bindgen::closure::Closure;

        // Listen for load-cbu events
        let cbu_callback = Closure::<dyn Fn(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(detail) = event.detail().as_string() {
                if let Ok(mut pending) = PENDING_CBU.lock() {
                    *pending = Some(detail);
                }
            } else if let Ok(obj) = js_sys::Reflect::get(&event.detail(), &"id".into()) {
                if let Some(id) = obj.as_string() {
                    if let Ok(mut pending) = PENDING_CBU.lock() {
                        *pending = Some(id);
                    }
                }
            }
        });
        let _ = window
            .add_event_listener_with_callback("load-cbu", cbu_callback.as_ref().unchecked_ref());
        cbu_callback.forget();

        // Listen for focus-entity events
        let focus_callback = Closure::<dyn Fn(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(detail) = event.detail().as_string() {
                if let Ok(mut pending) = PENDING_FOCUS.lock() {
                    *pending = Some(detail);
                }
            } else if let Ok(obj) = js_sys::Reflect::get(&event.detail(), &"id".into()) {
                if let Some(id) = obj.as_string() {
                    if let Ok(mut pending) = PENDING_FOCUS.lock() {
                        *pending = Some(id);
                    }
                }
            }
        });
        let _ = window.add_event_listener_with_callback(
            "focus-entity",
            focus_callback.as_ref().unchecked_ref(),
        );
        focus_callback.forget();

        // Listen for set-view-mode events
        let mode_callback = Closure::<dyn Fn(CustomEvent)>::new(move |event: CustomEvent| {
            if let Ok(obj) = js_sys::Reflect::get(&event.detail(), &"mode".into()) {
                if let Some(mode) = obj.as_string() {
                    if let Ok(mut pending) = PENDING_VIEW_MODE.lock() {
                        *pending = Some(mode);
                    }
                }
            }
        });
        let _ = window.add_event_listener_with_callback(
            "set-view-mode",
            mode_callback.as_ref().unchecked_ref(),
        );
        mode_callback.forget();

        tracing::info!("JsBridge: event listeners registered on window");
    }

    /// Emit entity selected event to JS
    pub fn emit_entity_selected(&self, entity_id: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            let mut init = CustomEventInit::new();
            init.detail(&JsValue::from_str(entity_id));

            if let Ok(event) = CustomEvent::new_with_event_init_dict("egui-entity-selected", &init)
            {
                let _ = self.window.dispatch_event(&event);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            tracing::debug!("Entity selected: {}", entity_id);
        }
    }

    /// Emit CBU changed event to JS
    pub fn emit_cbu_changed(&self, cbu_id: &str) {
        #[cfg(target_arch = "wasm32")]
        {
            let mut init = CustomEventInit::new();
            init.detail(&JsValue::from_str(cbu_id));

            if let Ok(event) = CustomEvent::new_with_event_init_dict("egui-cbu-changed", &init) {
                let _ = self.window.dispatch_event(&event);
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            tracing::debug!("CBU changed: {}", cbu_id);
        }
    }

    /// Emit ready event to JS
    pub fn emit_ready(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(event) = CustomEvent::new("egui-ready") {
                let _ = self.window.dispatch_event(&event);
            }
        }
    }

    /// Poll for pending focus request
    pub fn poll_focus_request(&mut self) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(mut pending) = PENDING_FOCUS.lock() {
                pending.take()
            } else {
                None
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        None
    }

    /// Poll for pending CBU load request
    pub fn poll_cbu_request(&mut self) -> Option<String> {
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(mut pending) = PENDING_CBU.lock() {
                pending.take()
            } else {
                None
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        None
    }

    /// Poll for pending view mode change
    pub fn poll_view_mode_request(&mut self) -> Option<ViewMode> {
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(mut pending) = PENDING_VIEW_MODE.lock() {
                pending.take().and_then(|s| match s.as_str() {
                    "KYC_UBO" => Some(ViewMode::KycUbo),
                    "SERVICE_DELIVERY" => Some(ViewMode::ServiceDelivery),
                    "CUSTODY" => Some(ViewMode::Custody),
                    _ => None,
                })
            } else {
                None
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        None
    }
}

impl Default for JsBridge {
    fn default() -> Self {
        Self::new()
    }
}
