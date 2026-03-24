//! Canonical enforcement helpers re-exported from `sem_os_core`.

pub use sem_os_core::enforce::{
    enforce_attribute_read, enforce_read, enforce_read_label, filter_by_abac, redacted_stub,
    EnforceResult,
};
