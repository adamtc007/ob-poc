fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use standard OUT_DIR for generated proto code
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/ob/gateway/v1/entity_gateway.proto"], &["proto"])?;
    Ok(())
}
