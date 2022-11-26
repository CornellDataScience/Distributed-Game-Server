fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .type_attribute(
            ".",
            "#[derive(serde_derive::Serialize,serde_derive::Deserialize)]",
        )
        .compile(&["proto/raft.proto"], &["proto"])?;
    Ok(())
}

