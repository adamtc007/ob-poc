//! Build script for ob-poc
//!
//! Handles:
//! - Copying WASM UI files from crates/ob-poc-ui/pkg to src/ui/static/wasm
//! - Proto compilation (disabled for Phase 1)

use std::fs;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=crates/ob-poc-ui/pkg/ob_poc_ui.js");
    println!("cargo:rerun-if-changed=crates/ob-poc-ui/pkg/ob_poc_ui_bg.wasm");

    // Copy WASM files from crates/ob-poc-ui/pkg to static/pkg (where server serves from)
    let wasm_src = Path::new("crates/ob-poc-ui/pkg");
    let wasm_dst = Path::new("static/pkg");

    if wasm_src.exists() {
        // Create destination directory if it doesn't exist
        fs::create_dir_all(wasm_dst)?;

        let files = [
            "ob_poc_ui.js",
            "ob_poc_ui_bg.wasm",
            "ob_poc_ui.d.ts",
            "ob_poc_ui_bg.wasm.d.ts",
        ];

        for file in &files {
            let src = wasm_src.join(file);
            let dst = wasm_dst.join(file);
            if src.exists() {
                // Only copy if source is newer than destination
                let should_copy = if dst.exists() {
                    let src_meta = fs::metadata(&src)?;
                    let dst_meta = fs::metadata(&dst)?;
                    src_meta.modified()? > dst_meta.modified()?
                } else {
                    true
                };

                if should_copy {
                    fs::copy(&src, &dst)?;
                    println!("cargo:warning=Copied {} to static/wasm/", file);
                }
            }
        }
    }

    Ok(())
}
