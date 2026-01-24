//! Compile-fail tests for ob-poc-macros
//!
//! These tests verify that the macros produce helpful errors when misused.

#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/trybuild/*.rs");
}
