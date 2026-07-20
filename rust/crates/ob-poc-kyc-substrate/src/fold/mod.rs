//! The two folds: control + determination, obligation.
//! Both are pure functions over the per-subject event stream.
//! `state = fold(events)`.

pub(crate) mod control;
pub(crate) mod obligation;
pub(crate) mod registry;
