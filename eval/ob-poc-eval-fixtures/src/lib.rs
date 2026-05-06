#![forbid(unsafe_code)]
//! Fixture helpers and bundled test cases for the Sage Eval Harness.

use std::path::PathBuf;

/// Absolute path to the bundled test-case directory.
pub const TEST_CASE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test_cases");

/// Return the absolute path to a bundled test case.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_fixtures::test_case_path;
///
/// let path = test_case_path("cbu_promote_active.yaml");
/// assert!(path.ends_with("cbu_promote_active.yaml"));
/// ```
pub fn test_case_path(file_name: &str) -> PathBuf {
    PathBuf::from(TEST_CASE_DIR).join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_test_case_exists() {
        assert!(test_case_path("cbu_promote_active.yaml").exists());
    }
}
