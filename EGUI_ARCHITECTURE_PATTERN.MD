 “Read and follow this file when editing egui code.”


# egui Architecture Pattern & State Management  
_for the CBU / KYC / UBO DSL Explorer_

This document defines how we structure **egui** UI code in this project.

It exists to prevent:

- flag soup,
- random local state,
- UI glitches (panels resetting, “double toggles”, etc.),
- hidden side effects (DB/agent/DSL calls triggered from random UI code),

especially when agents are editing the code.

The goal is:

> A small set of predictable patterns so that:
> - humans can navigate the codebase,
> - agents can make safe changes,
> - and the UI remains stable as the app grows.

---

## 1. High-Level Goals

1. **Single source of truth** for UI + domain state (no random `static mut`, no global flags).
2. **Explicit modes** via enums (not boolean soup).
3. **Separation of concerns**:
   - Domain state & business rules
   - UI state & layout
   - Navigation / screen routing
   - IO / background tasks (agents, DB, DSL engine)
4. **Event → Command pattern** for non-trivial interactions.
5. **Background tasks** encoded in state (no hidden async work).
6. **Agent-safe**: clear rules on what agents may change, and how.

---

## 2. Top-Level Structure: `MyApp`, `DomainState`, `UiState`

At the top level, the application state is explicitly split between **domain** and **UI**:

```rust
pub struct MyApp {
    pub domain: DomainState,
    pub ui: UiState,
}

Domain state holds business data and DSL results:

pub struct DomainState {
    pub current_cbu: Option<CbuId>,
    pub current_case: Option<CaseId>,
    pub cbu_graph: CbuGraph,
    pub kyc_cases: Vec<KycCase>,
    pub dsl_states: DslStateStore,
    pub tasks: BackgroundTasks,
    // etc.
}

UI state holds interaction & visual state:

pub struct UiState {
    pub screen: Screen,
    pub agent_panel_open: bool,
    pub case_form: CaseFormState,
    pub graph_view: GraphViewState,
    pub modal: Modal,
    // etc.
}

RULES – State Ownership
	•	Persistent business state → DomainState.
	•	Persistent UI/interaction state → UiState.
	•	Transient visuals (purely derived from state) → local variables inside update/UI functions, recreated each frame.
	•	Do not store persistent state in local variables inside update or UI functions.

⸻

3. Screen Navigation: Screen Enum & Router

We model navigation through a Screen enum. This prevents scattered "if we’re on screen X" logic.

pub enum Screen {
    Dashboard,
    Cbu(CbuScreen),
    Case(CaseScreen),
    AgentChat,
    // add new screens here
}

The main UI uses a router function:

fn render_main_ui(ui: &mut egui::Ui, app: &mut MyApp) {
    match app.ui.screen {
        Screen::Dashboard => dashboard_screen(ui, app),
        Screen::Cbu(ref mut s) => cbu_screen(ui, app, s),
        Screen::Case(ref mut s) => case_screen(ui, app, s),
        Screen::AgentChat => agent_chat_screen(ui, app),
    }

    render_modal(ui, &mut app.ui.modal);
}

RULE – Central Router
	•	All top-level navigation flows through one router function (render_main_ui).
	•	Do not sprinkle if matches!(ui_state.screen, Screen::...) checks all over the codebase.
	•	When adding a new screen:
	1.	Add a variant to Screen.
	2.	Add a branch in the router.
	3.	Create a dedicated *_screen function.

⸻

4. No Flag Soup – Use Enums for Modes

We do not accumulate dozens of booleans like:

// ❌ anti-pattern
struct AppState {
    show_cbu: bool,
    show_case: bool,
    editing_cbu: bool,
    editing_case: bool,
    creating_new: bool,
    // ...
}

This leads to impossible / conflicting states
(e.g. “editing CBU + editing case + creating new” at the same time).

Instead, we use enums for mutually exclusive modes:

// ✅ pattern
pub enum Screen {
    Dashboard,
    Cbu(CbuScreen),
    Case(CaseScreen),
    AgentChat,
}

pub struct CbuScreen {
    pub mode: CbuMode,
    // anything else specific to this screen
}

pub enum CbuMode {
    Viewing,
    Editing(CbuId),
    CreatingNew,
}

Another example for selection state:

pub enum Selection {
    None,
    Node(NodeId),
    Edge(EdgeId),
    // other mutually exclusive selections
}

RULE – Flags vs Enums
	•	If you find yourself adding more than one boolean to represent modes, stop and:
	•	identify the mutually exclusive states,
	•	encode them as an enum on the relevant struct.

⸻

5. Forms in egui – Pattern for Data Capture

We do occasionally need forms (case creation, KYC attribute capture, etc.). These follow a consistent pattern:
	•	Form state is a dedicated struct on UiState (or nested screen state).
	•	The UI function receives a &mut FormState.
	•	Validation errors are stored in the form state and displayed alongside the fields.
	•	The function optionally returns an action (e.g. “submitted”).

Example:

#[derive(Default, Clone)]
pub struct CaseFormState {
    pub name: String,
    pub description: String,
    pub error: Option<String>,
}

pub enum FormAction {
    None,
    Submitted,
}

pub fn case_form_ui(ui: &mut egui::Ui, form: &mut CaseFormState) -> FormAction {
    ui.label("Case name");
    ui.text_edit_singleline(&mut form.name);

    ui.label("Description");
    ui.text_edit_multiline(&mut form.description);

    if let Some(err) = &form.error {
        ui.colored_label(egui::Color32::RED, err);
    }

    if ui.button("Create Case").clicked() {
        if form.name.trim().is_empty() {
            form.error = Some("Name is required".into());
            FormAction::None
        } else {
            FormAction::Submitted
        }
    } else {
        FormAction::None
    }
}

Usage:

match case_form_ui(ui, &mut app.ui.case_form) {
    FormAction::Submitted => {
        handle_event(
            &mut app.domain,
            &mut app.ui,
            AppEvent::CreateCaseFromForm,
        );
    }
    FormAction::None => {}
}

RULE – Form Pattern
	•	Do not store form fields as ad hoc locals.
	•	Always represent forms via dedicated structs on UiState / screen state.

⸻

6. Events & Commands – Two-Layer Interaction

We separate what the user did from what the app does:

6.1 Events (User Intent)

AppEvent represents user actions or simple UI-driven events:

pub enum AppEvent {
    SelectNode(NodeId),
    OpenCaseEditor(CaseId),
    SubmitCaseForm,
    ToggleAgentPanel,
    // ...
}

UI code emits events instead of mutating state all over the place:

fn cbu_node_ui(ui: &mut egui::Ui, app: &mut MyApp, node: &Node) {
    if ui.button("Edit Case").clicked() {
        app.ui_events.push(AppEvent::OpenCaseEditor(node.case_id));
    }
}

6.2 Commands (Domain Operations / IO)

AppCommand represents domain operations, often involving IO or background work:

pub enum AppCommand {
    SaveCase(CaseId),
    RunDslForCase(CaseId),
    AskAgentForHelp(AgentPrompt),
    LoadCbuGraph,
    PersistDomainState,
}

We process events centrally:

pub fn handle_event(
    domain: &mut DomainState,
    ui: &mut UiState,
    event: AppEvent,
    commands: &mut Vec<AppCommand>,
) {
    match event {
        AppEvent::OpenCaseEditor(case_id) => {
            ui.screen = Screen::Case(CaseScreen { case_id });
        }
        AppEvent::SubmitCaseForm => {
            if let Some(case_id) = apply_case_form(domain, &ui.case_form) {
                commands.push(AppCommand::SaveCase(case_id));
            }
        }
        AppEvent::ToggleAgentPanel => {
            ui.agent_panel_open = !ui.agent_panel_open;
        }
        // etc.
    }
}

Then we execute commands in a small, central place:

pub fn execute_commands(app: &mut MyApp) {
    let commands: Vec<AppCommand> = app.domain.pending_commands.drain(..).collect();

    for cmd in commands {
        match cmd {
            AppCommand::SaveCase(case_id) => {
                // kick off async DB save, or update in-memory storage
            }
            AppCommand::RunDslForCase(case_id) => {
                // trigger DSL engine
            }
            AppCommand::AskAgentForHelp(prompt) => {
                // enqueue agent request
            }
            AppCommand::LoadCbuGraph => { /* ... */ }
            AppCommand::PersistDomainState => { /* ... */ }
        }
    }
}

RULE – Events & Commands
	•	UI code:
	•	may read MyApp,
	•	may mutate UiState,
	•	should emit AppEvents, not call domain/IO logic directly.
	•	handle_event:
	•	mutates DomainState and UiState,
	•	may emit AppCommands.
	•	Only the command executor interacts with IO, DB, agents, or DSL engine.

⸻

7. Background Tasks (TaskStatus)

Async/long-running operations (DB, agents, DSL compilation) are represented explicitly in state.

pub enum TaskStatus<T, E> {
    Idle,
    InProgress,
    Finished(Result<T, E>),
}

pub struct BackgroundTasks {
    pub agent_reply: TaskStatus<AgentReply, AgentError>,
    pub dsl_compile: TaskStatus<CompileResult, CompileError>,
    // add more as needed
}

When kicking off a task:

fn start_agent_request(domain: &mut DomainState, prompt: AgentPrompt) {
    domain.tasks.agent_reply = TaskStatus::InProgress;
    // elsewhere, spawn async job and send result via a channel
}

In the main update loop, we poll channels and update TaskStatus:

fn poll_background_tasks(app: &mut MyApp) {
    if let Some(result) = app.domain.agent_rx.try_recv().ok() {
        app.domain.tasks.agent_reply = TaskStatus::Finished(result);
    }

    // etc. for other tasks
}

In the UI:

fn agent_panel_ui(ui: &mut egui::Ui, app: &mut MyApp) {
    match &app.domain.tasks.agent_reply {
        TaskStatus::Idle => ui.label("No agent request."),
        TaskStatus::InProgress => ui.label("Waiting for agent response..."),
        TaskStatus::Finished(Ok(reply)) => {
            ui.label(&reply.text);
        }
        TaskStatus::Finished(Err(e)) => {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
        }
    }
}

RULE – Background Work
	•	All async/long-running operations MUST be surfaced as TaskStatus fields on DomainState.tasks.
	•	UI only:
	•	reflects current TaskStatus,
	•	may trigger new tasks via AppCommands.
	•	No hidden tokio::spawn or async work buried inside random UI functions.

⸻

8. Transient UI Overlays (Modals, Popups, etc.)

For confirmations and error dialogs, we use a Modal enum:

pub enum Modal {
    None,
    ConfirmDeleteCbu(CbuId),
    Error(String),
}

pub struct UiState {
    pub screen: Screen,
    pub modal: Modal,
    // ...
}

Rendering:

fn render_modal(ui: &mut egui::Ui, modal: &mut Modal) {
    match modal {
        Modal::None => {}
        Modal::ConfirmDeleteCbu(cbu_id) => {
            egui::Window::new("Confirm delete")
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Delete CBU {cbu_id:?}?"));

                    if ui.button("Delete").clicked() {
                        // emit event/command:
                        // commands.push(AppCommand::DeleteCbu(*cbu_id));
                        *modal = Modal::None;
                    }
                    if ui.button("Cancel").clicked() {
                        *modal = Modal::None;
                    }
                });
        }
        Modal::Error(msg) => {
            egui::Window::new("Error")
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.colored_label(egui::Color32::RED, msg);
                    if ui.button("OK").clicked() {
                        *modal = Modal::None;
                    }
                });
        }
    }
}

RULE – Modals
	•	There is a single UiState.modal that controls all overlays.
	•	Do not introduce multiple “is_modal_open” style booleans.
	•	New overlays = new variants on Modal.

⸻

9. Graph / Large View Performance

The CBU/UBO explorer will render graphs and potentially large lists. We adopt a few simple rules:

pub struct GraphViewState {
    pub camera_pos: egui::Vec2,
    pub zoom: f32,
    // caches, layout results, etc.
}

RULES – Graph & Lists
	•	Use a dedicated GraphViewState for camera/zoom and layout-related UI state.
	•	Use stable Ids for interactable elements:

ui.push_id(node.id, |ui| {
    // per-node controls
});


	•	Avoid heavy allocations in hot loops:
	•	Precompute layout or derived data when underlying domain data changes, not every frame.
	•	For very large graphs:
	•	Only render nodes/edges within the current camera viewport (simple “frustum” culling is enough).

⸻

10. Theming & Layout Helpers

To keep UI changes localized and agent-safe, we encourage simple layout/theming helpers:

pub struct Theme {
    pub spacing: f32,
    pub section_title_size: f32,
    // etc.
}

impl Theme {
    pub fn default() -> Self {
        Self {
            spacing: 8.0,
            section_title_size: 18.0,
        }
    }
}

pub fn section_header(ui: &mut egui::Ui, title: &str) {
    ui.heading(title);
    ui.add_space(4.0);
}

RULE – Theming
	•	Visual improvements should preferentially go through shared helpers (Theme, section_header, etc.).
	•	Avoid duplicating layout magic numbers across the codebase.

⸻

11. Domain Context: CBU / KYC / UBO Explorer

This UI is not a generic CRUD app; it is:
	•	A graphical explorer over a CBU graph (funds, SPVs, managers, investors, accounts).
	•	A window onto KYC / UBO / Document DSL states.
	•	A shell around agent interactions that produce/read DSL.

Therefore:
	•	DomainState should mirror domain concepts:

pub struct DomainState {
    pub cbu_graph: CbuGraph,
    pub selected_node: Option<NodeId>,
    pub selected_case: Option<CaseId>,
    pub kyc_cases: Vec<KycCase>,
    pub dsl_states: DslStateStore,
    pub tasks: BackgroundTasks,
    // ...
}


	•	UiState captures how we view them:

pub struct UiState {
    pub screen: Screen,
    pub graph_view: GraphViewState,
    pub case_form: CaseFormState,
    pub agent_panel_open: bool,
    pub modal: Modal,
    // ...
}


	•	UI code MUST NOT re-encode domain rules via a web of boolean flags and ad-hoc conditions. Use enums and domain types.

⸻

12. Rules for Agents (Claude / GPT / etc.)

When modifying egui UI code in this repo:
	1.	Do not introduce new boolean flags for mutually exclusive modes.
	•	Instead, extend existing enums (Screen, CbuMode, Selection, Modal) or create a new enum.
	2.	Do not put persistent state in locals inside update() or UI functions.
	•	All persistent state goes into DomainState or UiState (or their nested structs).
	3.	Follow the event/command pattern:
	•	UI:
	•	may update UiState,
	•	may push AppEvents,
	•	must NOT directly call heavy IO, DB, or agent logic.
	•	handle_event:
	•	mutates DomainState & UiState,
	•	pushes AppCommands.
	•	Command executor:
	•	is the only place performing IO / background work.
	4.	Background tasks:
	•	Represent them via TaskStatus fields on DomainState.tasks.
	•	Do not spawn ad-hoc async tasks from arbitrary UI functions.
	5.	Modals and overlays:
	•	Use UiState.modal and add variants to the Modal enum.
	•	Do not create free-floating show_*_dialog flags.

12.1 Agent Checklist (Before Committing Changes)

Agents should run through this checklist before finalizing any PR:
	1.	Did I add any new booleans for modes or screens?
	•	If they are mutually exclusive, convert them into an enum.
	2.	Did I add any persistent state as a local variable?
	•	If it needs to survive across frames, move it into DomainState or UiState.
	3.	Did I trigger any domain operation (DB, DSL, agent, filesystem) directly from UI code?
	•	If yes, refactor it into AppEvent + AppCommand.
	4.	Did I introduce any async/background work?
	•	Represent its lifecycle via TaskStatus on DomainState.tasks.
	5.	Did I add a new screen, modal, or graph mode?
	•	Ensure:
	•	a new Screen/Modal/mode variant has been added,
	•	the router and/or render_modal handles it explicitly.

⸻

By following this pattern, we avoid the typical egui failure modes and keep a clean, inspectable architecture aligned with our DSL + CBU graph + agent-driven design.
	•	Create .agents/EGUI_ARCHITECTURE_PATTERN.md in your repo,
	•	paste this in,
