# CBU View Refactor - Session TODO

## Completed
- [x] Fixed CBU selection not switching view from Universe to graph widget
- [x] Added `lookup_and_select_cbu()` for name-based CBU lookup
- [x] Changed ViewMode from enum (5 variants) to single struct (CBU/Trading only)
- [x] Updated default view mode to TRADING in UI
- [x] Remove debug logging from app.rs and state.rs
- [x] Clean up dead ViewMode code in command.rs (consolidated voice triggers, changed keyboard "g" for graph)
- [x] Update server-side graph API default from KYC_UBO to TRADING
- [x] Clean up session/view DSL verbs in YAML files (graph.yaml default=TRADING, fixed session.yaml return type)
- [x] Test CBU graph shows trading entities (verified: Trading Profile, Instrument Matrix, instrument classes, markets, counterparties)

## All Tasks Complete

### Summary of Changes
1. **app.rs/state.rs**: Removed all debug logging for CBU selection and disambiguation
2. **command.rs**: Consolidated 4 view mode voice triggers into 1, changed keyboard shortcut to "g" for graph view
3. **graph_routes.rs**: Changed all `unwrap_or("KYC_UBO")` to `unwrap_or("TRADING")` (5 locations)
4. **graph.yaml**: Changed default view-mode from KYC_UBO to TRADING, removed SERVICE_DELIVERY and PRODUCTS_ONLY
5. **session.yaml**: Fixed invalid `type: list` to `type: record_set`
6. **Verified**: CBU graph API now defaults to TRADING view, showing trading profile, instrument matrix, instrument classes, markets, and counterparty entities

## Key Files Changed
- `rust/crates/ob-poc-graph/src/graph/mod.rs` - ViewMode now unit struct
- `rust/crates/ob-poc-ui/src/state.rs` - pending_cbu_lookup, process_async_results
- `rust/crates/ob-poc-ui/src/app.rs` - lookup_and_select_cbu(), view mode handling
- `rust/crates/ob-poc-ui/src/panels/toolbar.rs` - view_mode_name()
- `rust/crates/ob-poc-ui/src/command.rs` - SetViewMode DSL generation
