//! OrbitQL Language Server binary
//!
//! This binary starts the OrbitQL Language Server Protocol (LSP) server
//! for integration with IDEs and editors.

use orbit_shared::orbitql::lsp::start_lsp_server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Start the LSP server
    if let Err(e) = start_lsp_server().await {
        eprintln!("Failed to start OrbitQL Language Server: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
