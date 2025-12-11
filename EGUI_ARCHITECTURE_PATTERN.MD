Not as an actual file on disk, no — I generated the full markdown content for you to save as a .md file in your repo.

Here it is again, cleanly, so you can just drop it into Zed as e.g.
./.agents/EGUI_ARCHITECTURE_PATTERN.md:

# egui Architecture Pattern & State Management
_for the CBU / KYC / UBO DSL Explorer_

This document defines how we structure **egui** UI code in this project.

It exists to prevent:
- flag soup,
- random local state,
- UI glitches (panels resetting, “double toggles”, etc.),
especially when agents are editing the code.

The **goal** is: predictable, testable UI state that plays nicely with our **DSL + CBU graph + agent** architecture.

---

## 1. Immediate-Mode Mental Model

egui is **immediate-mode**:

- Every frame, `App::update(&mut self, &Context, &mut Frame)` runs.
- We **rebuild** the entire UI tree each frame.
- The only persistent state is what **we** store in long-lived structs (our app state), or in `ctx.memory()` / `Id`-keyed storage.

**RULE:**  
> Persistent UI state MUST live in our own `AppState` (or dedicated state structs), NOT in locals inside `update()`/UI functions.

---

## 2. Central `AppState` – the Single Source of Truth

We use a **single, central application state struct** as the source of truth for UI and domain-related UI state.

Example pattern (simplified):

```rust
pub struct MyApp {
    state: AppState,
}

pub struct AppState {
    // Domain state
    current_cbu: Option<CbuId>,
    current_case: Option<CaseId>,
    cbu_graph: CbuGraph,

    // UI state
    screen: Screen,
    agent_panel_open: bool,
    case_form: CaseFormState,
}

pub enum Screen {
    Dashboard,
    Cbu(CbuScreen),
    Case(CaseScreen),
    AgentChat,
}

pub enum CbuScreen {
    Overview,
    Editing(CbuId),
    CreatingNew,
}

pub enum CaseScreen {
    Overview(CaseId),
    Editing(CaseId),
}

#[derive(Default, Clone)]
pub struct CaseFormState {
    pub name: String,
    pub jurisdiction: String,
    pub error: Option<String>,
}

All UI rendering functions take &mut AppState (or a focused subset). No other long-lived state is allowed.

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            render_main_ui(ui, &mut self.state);
        });
    }
}


⸻

3. NO Flag Soup – Use Enums for Modes

We DO NOT accumulate dozens of booleans like:

// ❌ anti-pattern
struct AppState {
    show_cbu: bool,
    show_case: bool,
    editing_cbu: bool,
    editing_case: bool,
    creating_new: bool,
    // ...
}

This leads to impossible / conflicting states (e.g. editing CBU + editing case + creating new).

Instead, we use enums to encode mutually exclusive modes:

// ✅ pattern
pub enum Screen {
    Dashboard,
    Cbu(CbuScreen),
    Case(CaseScreen),
    AgentChat,
}

pub enum CbuScreen {
    Overview,
    Editing(CbuId),
    CreatingNew,
}

RULE:
If a set of boolean flags are mutually exclusive UI modes, replace them with an enum on AppState.

⸻

4. NO Local Persistent State in UI Functions

We never re-initialise UI state inside update or a UI function.

Anti-pattern

fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
    let mut show_cbu_panel = true; // ❌ re-created every frame

    if ui.button("Toggle CBU").clicked() {
        show_cbu_panel = !show_cbu_panel;
    }

    if show_cbu_panel {
        draw_cbu_panel(ui);
    }
}

	•	show_cbu_panel is re-created every frame → it never persists.
	•	This causes flicker / panels not staying open / weird resets.

Correct pattern

pub struct AppState {
    pub show_cbu_panel: bool,
    // ...
}

fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
    egui::CentralPanel::default().show(ctx, |ui| {
        top_bar(ui, &mut self.state);
        if self.state.show_cbu_panel {
            cbu_panel(ui, &mut self.state);
        }
    });
}

fn top_bar(ui: &mut egui::Ui, state: &mut AppState) {
    if ui.button("Toggle CBU").clicked() {
        state.show_cbu_panel = !state.show_cbu_panel;
    }
}

RULE:
Any UI element whose state must persist across frames (open/closed, selected values, form contents) MUST be stored on AppState (or a struct owned by it), not as a local variable.

⸻

5. Event-Driven Updates (Recommended)

For more complex flows (CBU viewer, case editor, agent panel), prefer an event style over ad-hoc mutation scattered around.

Example:

pub enum AppEvent {
    SelectNode(NodeId),
    OpenCaseEditor(CaseId),
    SubmitCaseForm,
    ToggleAgentPanel,
}

pub fn handle_event(state: &mut AppState, event: AppEvent) {
    match event {
        AppEvent::SelectNode(id) => {
            state.current_node = Some(id);
            state.screen = Screen::Cbu(CbuScreen::Overview);
        }
        AppEvent::OpenCaseEditor(case_id) => {
            state.screen = Screen::Case(CaseScreen::Editing(case_id));
            state.case_form = CaseFormState::from_case_id(case_id);
        }
        AppEvent::SubmitCaseForm => {
            // validate + write to domain model
        }
        AppEvent::ToggleAgentPanel => {
            state.agent_panel_open = !state.agent_panel_open;
        }
    }
}

UI functions then emit events, rather than modifying many flags directly:

fn cbu_node_ui(ui: &mut Ui, state: &mut AppState, node: &Node) {
    if ui.button("Edit Case").clicked() {
        handle_event(state, AppEvent::OpenCaseEditor(node.case_id));
    }
}

RULE (for agents and humans):
For non-trivial UI flows, prefer adding events to AppEvent + handle_event instead of mutating many AppState fields in many scattered places.

⸻

6. Forms in egui – Pattern for Data Capture

We do occasionally need simple forms (case creation, KYC attribute capture, etc.). These follow a consistent pattern:
	•	Form state is a struct on AppState (e.g. CaseFormState).
	•	The UI function receives &mut CaseFormState.
	•	Validation errors are stored in the form state and displayed alongside the fields.
	•	The function optionally returns an action (e.g. “submitted”).

Example:

#[derive(Default, Clone)]
pub struct CaseFormState {
    pub name: String,
    pub jurisdiction: String,
    pub is_ubo_complex: bool,
    pub error: Option<String>,
}

pub fn case_form_ui(ui: &mut egui::Ui, form: &mut CaseFormState) -> bool {
    let mut submitted = false;

    ui.heading("New CBU / KYC Case");

    ui.horizontal(|ui| {
        ui.label("CBU Name:");
        ui.text_edit_singleline(&mut form.name);
    });

    ui.horizontal(|ui| {
        ui.label("Jurisdiction:");
        egui::ComboBox::from_id_source("jurisdiction")
            .selected_text(if form.jurisdiction.is_empty() {
                "Select…"
            } else {
                &form.jurisdiction
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut form.jurisdiction, "LU".to_string(), "Luxembourg");
                ui.selectable_value(&mut form.jurisdiction, "IE".to_string(), "Ireland");
                ui.selectable_value(&mut form.jurisdiction, "UK".to_string(), "United Kingdom");
            });
    });

    ui.checkbox(&mut form.is_ubo_complex, "Expect complex UBO structure");

    if let Some(err) = &form.error {
        ui.colored_label(egui::Color32::RED, err);
    }

    if ui.button("Create Case").clicked() {
        if form.name.trim().is_empty() {
            form.error = Some("CBU name is required".into());
        } else if form.jurisdiction.is_empty() {
            form.error = Some("Jurisdiction is required".into());
        } else {
            form.error = None;
            submitted = true;
        }
    }

    submitted
}

RULE:
	•	form state lives in AppState,
	•	form UI is a pure function of (&mut Ui, &mut FormState),
	•	submission/validation is explicit and never hidden inside random flags.

⸻

7. Using egui’s Own State (When Appropriate)

egui provides some widget-local state mechanisms:
	•	CollapsingHeader, Window, SelectableLabel, etc. maintain their open/selected state keyed by Id.
	•	You can use ui.make_persistent_id("something") and ctx.memory_mut() for more complex widget-local state.

Guidelines:
	•	Use widget-local state only for truly widget-local concerns (e.g. a small inspector’s scroll offset).
	•	For anything that matters to the overall application behaviour (current screen, selected node, forms, panels), store state in AppState.

RULE:
If a piece of state affects navigation, DSL behaviour, or CBU/UBO semantics, it belongs in AppState, not ctx.memory().

⸻

8. Domain Context: CBU / KYC / UBO Explorer

This UI is not a generic CRUD app; it is:
	•	A graphical explorer over a CBU graph (funds, SPVs, managers, investors, accounts).
	•	A window onto KYC / UBO / Document DSL states.
	•	A shell around agent interactions.

Therefore:
	•	AppState should mirror domain concepts:
	•	selected_node: Option<NodeId>,
	•	selected_case: Option<CaseId>,
	•	screen: Screen that distinguishes “CBU overview” vs “Case details” vs “Agent chat”.
	•	UI code should not re-encode domain rules via random flags; use enums + domain types.

Agents MUST respect this domain layering when editing UI code.

⸻

9. Rules for Agents (Claude / GPT / etc.)

When modifying egui UI code in this repo:
	1.	Do not introduce new boolean flags for modes that are mutually exclusive. Add/extend enums instead.
	2.	Do not store persistent state in local variables inside update() or UI functions. Use AppState (or structs owned by it).
	3.	Prefer:
	•	AppState for persistent UI/domain state,
	•	AppEvent + handle_event() for complex interactions,
	•	simple, pure UI functions of the form:
	•	fn panel(ui: &mut Ui, state: &mut AppState)
	•	fn form(ui: &mut Ui, form: &mut FormState) -> bool.
	4.	Before introducing new stateful UI flows, describe the intended states and transitions (screens/modes) and encode them as enums on AppState.
	5.	If existing code violates these patterns, propose a refactor plan first (AppState + enums + events), then apply changes in small, reviewable steps.

⸻

By following this pattern, we avoid the typical egui failure modes (flags overriding each other, panels resetting, double-toggles) and keep the UI architecture aligned with our DSL + CBU graph + agent-driven design.

You can now:

- create `.agents/EGUI_ARCHITECTURE_PATTERN.md` in your repo,
- paste this in,
- and tell Claude “read and follow this file when editing egui code.”
