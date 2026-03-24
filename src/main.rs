//! RVC MCP Server — voice conversion tools via Model Context Protocol.
//!
//! Migrated to psm-mcp-core: uses ToolHandler trait, McpServer transport,
//! PsmMcpError, and shared input validation.

mod config;
mod tools;

use config::{RvcConfig, SharedState};
use psm_mcp_transport::server::McpServer;
use tools::{
    RvcCleanTool, RvcExportOnnxTool, RvcExtractFeaturesTool, RvcInferTool, RvcListModelsTool,
    RvcModelExtractTool, RvcModelInfoTool, RvcModelMergeTool, RvcPreprocessTool,
    RvcSeparateVocalsTool, RvcStatusTool, RvcTrainTool,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(std::io::stderr)
        .init();

    let rvc_config = match RvcConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[rvc-mcp] Configuration error: {e}");
            std::process::exit(1);
        }
    };

    let state = SharedState::new(rvc_config);

    eprintln!(
        "[rvc-mcp] Starting with 12 tools, WebUI: {}",
        state.config.base_url
    );

    let mut server = McpServer::new("rvc-mcp", "0.1.0");

    server.register_tool(RvcStatusTool {
        state: state.clone(),
    });
    server.register_tool(RvcCleanTool {
        state: state.clone(),
    });
    server.register_tool(RvcListModelsTool {
        state: state.clone(),
    });
    server.register_tool(RvcModelInfoTool {
        state: state.clone(),
    });
    server.register_tool(RvcModelExtractTool {
        state: state.clone(),
    });
    server.register_tool(RvcModelMergeTool {
        state: state.clone(),
    });
    server.register_tool(RvcExportOnnxTool {
        state: state.clone(),
    });
    server.register_tool(RvcInferTool {
        state: state.clone(),
    });
    server.register_tool(RvcSeparateVocalsTool {
        state: state.clone(),
    });
    server.register_tool(RvcPreprocessTool {
        state: state.clone(),
    });
    server.register_tool(RvcExtractFeaturesTool {
        state: state.clone(),
    });
    server.register_tool(RvcTrainTool { state });

    if let Err(e) = server.run_stdio().await {
        eprintln!("[rvc-mcp] Server error: {e}");
        std::process::exit(1);
    }
}
