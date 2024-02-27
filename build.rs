fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(false)
        .compile(&["./schemas/tfplugin6.0.proto"], &["./schemas"])?;

    Ok(())
}
