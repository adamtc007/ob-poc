//! Build script for ob-poc
//!
//! Proto compilation (disabled for Phase 1)
//! WASM files are served directly from crates/ob-poc-ui/pkg

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
