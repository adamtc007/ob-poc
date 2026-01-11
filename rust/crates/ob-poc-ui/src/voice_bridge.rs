//! Voice Bridge - Connects JavaScript voice events to egui
//!
//! This module sets up event listeners for voice commands dispatched
//! by the JavaScript VoiceService. Commands are stored in a shared
//! queue and processed in the egui update loop.
//!
//! Architecture:
//! ```text
//! JS VoiceService ──CustomEvent──► DOM ──Closure──► VoiceBridge ──► AsyncState
//!                                                                        │
//!                                                                        ▼
//!                                                            egui update() loop
//! ```

use crate::command::{CommandSource, VoiceProvider};
use std::cell::RefCell;
use std::collections::VecDeque;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Maximum number of pending voice commands to prevent memory issues
const MAX_PENDING_COMMANDS: usize = 10;

/// Shared voice command queue accessible from both JS callbacks and egui
#[derive(Default)]
pub struct VoiceCommandQueue {
    commands: VecDeque<VoiceCommand>,
}

/// A voice command received from JavaScript
#[derive(Debug, Clone)]
pub struct VoiceCommand {
    pub command: String,
    pub transcript: String,
    pub confidence: f32,
    pub provider: String,
    pub params: serde_json::Value,
}

impl VoiceCommandQueue {
    /// Push a command to the queue
    pub fn push(&mut self, cmd: VoiceCommand) {
        if self.commands.len() >= MAX_PENDING_COMMANDS {
            // Drop oldest command
            self.commands.pop_front();
        }
        self.commands.push_back(cmd);
    }

    /// Take all pending commands
    pub fn take_all(&mut self) -> Vec<VoiceCommand> {
        self.commands.drain(..).collect()
    }

    /// Check if there are pending commands
    pub fn has_pending(&self) -> bool {
        !self.commands.is_empty()
    }
}

// Global voice command queue (thread-local for WASM)
thread_local! {
    static VOICE_QUEUE: RefCell<VoiceCommandQueue> = RefCell::new(VoiceCommandQueue::default());
    static LISTENER_INSTALLED: RefCell<bool> = RefCell::new(false);
}

/// Install the voice command event listener
/// Called once during app initialization
pub fn install_voice_listener() -> Result<(), JsValue> {
    // Check if already installed
    let already_installed = LISTENER_INSTALLED.with(|installed| *installed.borrow());
    if already_installed {
        web_sys::console::log_1(&"Voice listener already installed".into());
        return Ok(());
    }

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("No document"))?;

    // Create closure for voice-command events
    let voice_callback = Closure::wrap(Box::new(move |event: web_sys::CustomEvent| {
        let detail = event.detail();
        if detail.is_object() {
            let command = js_sys::Reflect::get(&detail, &"command".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();

            let transcript = js_sys::Reflect::get(&detail, &"transcript".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();

            let confidence = js_sys::Reflect::get(&detail, &"confidence".into())
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;

            let provider = js_sys::Reflect::get(&detail, &"provider".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();

            let params = js_sys::Reflect::get(&detail, &"params".into())
                .ok()
                .map(|v| {
                    // Convert JsValue to serde_json::Value
                    serde_wasm_bindgen::from_value(v).unwrap_or(serde_json::Value::Null)
                })
                .unwrap_or(serde_json::Value::Null);

            web_sys::console::log_1(
                &format!(
                    "[VoiceBridge] Received command: {} (confidence: {:.2})",
                    command, confidence
                )
                .into(),
            );

            // Push to queue
            VOICE_QUEUE.with(|queue| {
                queue.borrow_mut().push(VoiceCommand {
                    command,
                    transcript,
                    confidence,
                    provider,
                    params,
                });
            });
        }
    }) as Box<dyn FnMut(web_sys::CustomEvent)>);

    // Add event listener for voice-command
    document.add_event_listener_with_callback(
        "voice-command",
        voice_callback.as_ref().unchecked_ref(),
    )?;

    // Keep the closure alive
    voice_callback.forget();

    // Also listen for raw transcripts (for chat input)
    let transcript_callback = Closure::wrap(Box::new(move |event: web_sys::CustomEvent| {
        let transcript = js_sys::Reflect::get(&event.detail(), &"transcript".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();

        let confidence = js_sys::Reflect::get(&event.detail(), &"confidence".into())
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as f32;

        web_sys::console::log_1(
            &format!(
                "[VoiceBridge] Raw transcript: {} (confidence: {:.2})",
                transcript, confidence
            )
            .into(),
        );

        // Push as chat input command
        VOICE_QUEUE.with(|queue| {
            queue.borrow_mut().push(VoiceCommand {
                command: "SendChat".to_string(),
                transcript: transcript.clone(),
                confidence,
                provider: "voice".to_string(),
                params: serde_json::json!({ "message": transcript }),
            });
        });
    }) as Box<dyn FnMut(web_sys::CustomEvent)>);

    document.add_event_listener_with_callback(
        "voice-transcript",
        transcript_callback.as_ref().unchecked_ref(),
    )?;

    transcript_callback.forget();

    // Mark as installed
    LISTENER_INSTALLED.with(|installed| {
        *installed.borrow_mut() = true;
    });

    web_sys::console::log_1(&"[VoiceBridge] Voice listeners installed".into());
    Ok(())
}

/// Take all pending voice commands
/// Called from egui update() loop
pub fn take_pending_voice_commands() -> Vec<VoiceCommand> {
    VOICE_QUEUE.with(|queue| queue.borrow_mut().take_all())
}

/// Check if there are pending voice commands
pub fn has_pending_voice_commands() -> bool {
    VOICE_QUEUE.with(|queue| queue.borrow().has_pending())
}

/// Convert a VoiceCommand to a CommandSource for unified dispatch
pub fn voice_command_to_source(cmd: &VoiceCommand) -> CommandSource {
    let provider = match cmd.provider.as_str() {
        "deepgram" | "Deepgram" => VoiceProvider::Deepgram,
        "webspeech" | "WebSpeech" => VoiceProvider::WebSpeech,
        _ => VoiceProvider::Unknown,
    };

    CommandSource::Voice {
        transcript: cmd.transcript.clone(),
        confidence: cmd.confidence,
        provider,
    }
}

/// Dispatch context update to JavaScript voice service
/// Called when context changes (entity selected, CBU changed, etc.)
pub fn dispatch_context_update(
    focused_entity_id: Option<&str>,
    current_cbu_id: Option<&str>,
    view_mode: &str,
) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    // Create context object
    let context = js_sys::Object::new();
    if let Some(id) = focused_entity_id {
        js_sys::Reflect::set(&context, &"focusedEntityId".into(), &id.into()).ok();
    }
    if let Some(id) = current_cbu_id {
        js_sys::Reflect::set(&context, &"currentCbuId".into(), &id.into()).ok();
    }
    js_sys::Reflect::set(&context, &"viewMode".into(), &view_mode.into()).ok();

    // Create and dispatch event
    let event_init = web_sys::CustomEventInit::new();
    event_init.set_detail(&context);

    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("esper-context", &event_init)
    {
        document.dispatch_event(&event).ok();
    }
}

// =============================================================================
// VOICE CONTROL - Start/Stop listening
// =============================================================================

/// Start voice listening
/// Dispatches a custom event to JavaScript to start the voice service
pub fn start_voice_listening(mode: VoiceMode) {
    dispatch_voice_control("voice-start", mode);
    web_sys::console::log_1(&format!("[VoiceBridge] Start listening, mode: {:?}", mode).into());
}

/// Stop voice listening
/// Dispatches a custom event to JavaScript to stop the voice service
pub fn stop_voice_listening() {
    dispatch_voice_control("voice-stop", VoiceMode::Chat);
    web_sys::console::log_1(&"[VoiceBridge] Stop listening".into());
}

/// Voice input mode - determines how transcripts are routed
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VoiceMode {
    /// Normal chat mode - transcripts go to agent chat
    Chat,
    /// Resolution mode - transcripts go to resolution refinement
    Resolution,
    /// Command mode - transcripts are parsed as navigation commands
    Command,
}

/// Dispatch voice control event to JavaScript
fn dispatch_voice_control(event_name: &str, mode: VoiceMode) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };

    let document = match window.document() {
        Some(d) => d,
        None => return,
    };

    // Create control object with mode
    let control = js_sys::Object::new();
    let mode_str = match mode {
        VoiceMode::Chat => "chat",
        VoiceMode::Resolution => "resolution",
        VoiceMode::Command => "command",
    };
    js_sys::Reflect::set(&control, &"mode".into(), &mode_str.into()).ok();

    // Create and dispatch event
    let event_init = web_sys::CustomEventInit::new();
    event_init.set_detail(&control);

    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict(event_name, &event_init) {
        document.dispatch_event(&event).ok();
    }
}

// =============================================================================
// RESOLUTION-SPECIFIC VOICE QUEUE
// =============================================================================

/// Resolution voice transcript - separate from command queue
#[derive(Debug, Clone)]
pub struct ResolutionTranscript {
    pub transcript: String,
    pub confidence: f32,
}

thread_local! {
    static RESOLUTION_QUEUE: RefCell<VecDeque<ResolutionTranscript>> = RefCell::new(VecDeque::new());
}

/// Push a transcript specifically for resolution refinement
pub fn push_resolution_transcript(transcript: String, confidence: f32) {
    RESOLUTION_QUEUE.with(|queue| {
        let mut q = queue.borrow_mut();
        if q.len() >= MAX_PENDING_COMMANDS {
            q.pop_front();
        }
        q.push_back(ResolutionTranscript {
            transcript,
            confidence,
        });
    });
}

/// Take all pending resolution transcripts
pub fn take_resolution_transcripts() -> Vec<ResolutionTranscript> {
    RESOLUTION_QUEUE.with(|queue| queue.borrow_mut().drain(..).collect())
}

/// Check if there are pending resolution transcripts
pub fn has_pending_resolution_transcripts() -> bool {
    RESOLUTION_QUEUE.with(|queue| !queue.borrow().is_empty())
}
