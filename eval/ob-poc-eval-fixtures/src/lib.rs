#![forbid(unsafe_code)]
//! Fixture helpers and bundled test cases for the Sage Eval Harness.

mod bundle;
mod fixture;

use std::path::PathBuf;

pub use bundle::*;
pub use fixture::*;

/// Absolute path to the bundled test-case directory.
pub const TEST_CASE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/test_cases");

/// Absolute path to the seed-bundle directory.
pub const SEED_BUNDLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/seed_bundles");

/// Absolute path to the probe-stub directory.
pub const PROBE_STUB_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/probe_stubs");

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

/// Return the absolute path to a seed-bundle directory.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_fixtures::seed_bundle_path;
///
/// let path = seed_bundle_path("seed-kyc-baseline-cbu-portfolio");
/// assert!(path.ends_with("seed-kyc-baseline-cbu-portfolio"));
/// ```
pub fn seed_bundle_path(bundle_id: &str) -> PathBuf {
    PathBuf::from(SEED_BUNDLE_DIR).join(bundle_id)
}

/// Return the absolute path to a probe-stub file.
///
/// # Examples
///
/// ```rust
/// use ob_poc_eval_fixtures::probe_stub_path;
///
/// let path = probe_stub_path("lei_lookup.json");
/// assert!(path.ends_with("lei_lookup.json"));
/// ```
pub fn probe_stub_path(file_name: &str) -> PathBuf {
    PathBuf::from(PROBE_STUB_DIR).join(file_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_test_case_exists() {
        assert!(test_case_path("cbu_promote_active.yaml").exists());
    }

    #[test]
    fn seed_bundle_path_points_at_seed_bundle_dir() {
        assert!(seed_bundle_path("example").starts_with(SEED_BUNDLE_DIR));
    }

    #[test]
    fn probe_stub_path_points_at_probe_stub_dir() {
        assert!(probe_stub_path("lei_lookup.json").starts_with(PROBE_STUB_DIR));
    }
}
