//! Control domain verbs (11 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/control.yaml`.
//!
//! Split into two submodules:
//!
//! - [`analysis`] — graph-level control analysis: `analyze`,
//!   `build-graph`, `identify-ubos`, `trace-chain`,
//!   `reconcile-ownership`.
//! - [`board`] — board-controller lifecycle: `show-board-controller`,
//!   `recompute-board-controller`, `set-board-controller`,
//!   `clear-board-controller-override`, plus the two
//!   `import-*-register` stubs. Recompute / clear share a
//!   `compute_show_board_controller` helper with Show.

pub mod analysis;
pub mod board;

pub use analysis::{
    ControlAnalyze, ControlBuildGraph, ControlIdentifyUbos, ControlReconcileOwnership,
    ControlTraceChain,
};
pub use board::{
    ClearBoardControllerOverride, ImportGleifControl, ImportPscRegister,
    RecomputeBoardController, SetBoardController, ShowBoardController,
};
