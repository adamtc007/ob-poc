# egui UI Development

Before writing any egui/WASM UI code, review the 5 non-negotiable rules in CLAUDE.md → "egui State Management & Best Practices".

Key rules:
1. NO local state mirroring server data - use AppState.session (fetched), not panel.messages
2. Actions return values, no callbacks - return Some(Action::Save), don't self.save_data()
3. Short lock, then render - extract data, drop lock, then render
4. Process async first, render second - process_async_results() at top of update()
5. Server round-trip for mutations - POST → wait → refetch, not optimistic updates

Files to understand:
- rust/crates/ob-poc-ui/src/app.rs - Main app struct and update loop
- rust/crates/ob-poc-ui/src/state.rs - AppState, AsyncState, TextBuffers
- rust/crates/ob-poc-ui/src/panels/ - Panel implementations

Read CLAUDE.md section "egui State Management & Best Practices" for full details.
