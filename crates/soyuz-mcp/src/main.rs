//! Soyuz MCP Server Binary
//!
//! Runs the Soyuz MCP server on stdio transport, allowing AI agents to
//! programmatically generate 3D assets using Soyuz's SDF engine.
//!
//! ## Usage
//!
//! Run directly:
//! ```bash
//! soyuz-mcp
//! ```
//!
//! Or add to Claude Desktop's MCP configuration:
//! ```json
//! {
//!   "mcpServers": {
//!     "soyuz": {
//!       "command": "soyuz-mcp"
//!     }
//!   }
//! }
//! ```

use anyhow::Result;
use rmcp::ServiceExt;
use rmcp::transport::io::stdio;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use soyuz_mcp::state::SoyuzState;
use soyuz_mcp::SoyuzMcpService;

#[tokio::main]
async fn main() -> Result<()> {
    // CRITICAL: Log to stderr only - stdout is reserved for MCP JSON-RPC
    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(false);

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(tracing_subscriber::EnvFilter::new("info"))
        .init();

    eprintln!("Soyuz MCP server v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("Initializing GPU...");

    // Initialize headless GPU and state
    let state = SoyuzState::new().await?;

    eprintln!("GPU initialized successfully.");
    eprintln!("Ready. Listening on stdio...");

    // Create service and serve on stdio transport
    let service = SoyuzMcpService::new(state);
    let server = service.serve(stdio()).await?;

    // Wait for client to disconnect or error
    server.waiting().await?;

    eprintln!("Client disconnected. Shutting down.");
    Ok(())
}
