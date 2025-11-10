//! Build script for ob-poc
//!
//! Simplified build script that avoids protobuf compilation issues
//! during the initial Phase 1 implementation of centralized DSL editing.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // For Phase 1, we're focusing on the core DSL editing functionality
    // Proto compilation will be re-enabled in later phases when needed

    println!("cargo:rerun-if-changed=build.rs");
    // Proto compilation disabled for Phase 1 - no warning needed

    Ok(())
}
