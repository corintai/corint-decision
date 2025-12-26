fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Output generated code to OUT_DIR (standard Rust build location)
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .file_descriptor_set_path("proto/decision_descriptor.bin")
        .compile_protos(&["proto/decision.proto"], &["proto"])?;
    Ok(())
}
