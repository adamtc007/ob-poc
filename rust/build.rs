//! Build script for ob-poc

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Expose CARGO_MANIFEST_DIR at runtime for config loading in tests
    println!(
        "cargo:rustc-env=OB_POC_MANIFEST_DIR={}",
        std::env::var("CARGO_MANIFEST_DIR").unwrap()
    );

    // Compile BPMN-Lite gRPC proto (client-only, no server stubs)
    println!("cargo:rerun-if-changed=proto/bpmn_lite/v1/bpmn_lite.proto");
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(&["proto/bpmn_lite/v1/bpmn_lite.proto"], &["proto"])
        .expect("Failed to compile bpmn-lite proto");
}
