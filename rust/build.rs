//! Build script for ob-poc

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Expose CARGO_MANIFEST_DIR at runtime for config loading in tests
    println!(
        "cargo:rustc-env=OB_POC_MANIFEST_DIR={}",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );
}
