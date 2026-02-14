fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile all modular proto files
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/common.proto",
                "proto/session.proto",
                "proto/game.proto",
                "proto/engine.proto",
                "proto/events.proto",
                "proto/persistence.proto",
                "proto/positions.proto",
                "proto/chess_service.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
