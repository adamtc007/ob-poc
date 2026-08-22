//! Store error type.
//!
//! TS.6 §1: the type itself now lives in `ob-poc-kyc-read` (the append-free
//! crate), because the read functions that return it moved there. Re-exported
//! here so every existing `ob_poc_kyc_store::StoreError` consumer — the seam,
//! `ob-poc`, and this crate's own tests — is unchanged.
pub use ob_poc_kyc_read::StoreError;
