//! Compile-fail tests proving the proof-carrying construction invariants
//! (V&S §9.4, T1.3 exit criteria (a) and (c)):
//!
//! (a) `ExecutionEnvelope::seal` — the crate's only envelope constructor —
//!     is unreachable from outside `ob-poc-control-plane`. Since `tests/`
//!     integration tests compile as a separate crate linking only against
//!     the library's public API (exactly the same visibility boundary an
//!     external consumer sees), a fixture here that tries to call `seal`
//!     directly proves there is no code path from outside the crate to a
//!     sealed envelope — a fortiori no code path from any failure/rejection
//!     value, since there is no reachable code path *at all*.
//!
//! (c) `ExecutionEnvelope` does not implement `serde::Deserialize` — the
//!     runtime must obtain an envelope only via `seal`, never by
//!     deserializing one from storage or the wire.

#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/*.rs");
}
