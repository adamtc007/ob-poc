//! The two folds: control + determination, obligation.
//! Both are pure functions over the per-subject event stream.
//! `state = fold(events)`.

pub mod control;
pub mod obligation;
pub mod registry;
