//! RVC server configuration and Gradio HTTP client.

use psm_mcp_core::error::PsmMcpError;
use serde_json::{json, Value};
use std::sync::Arc;

/// RVC-specific configuration loaded from environment variables.
#[allow(dead_code)]
pub struct RvcConfig {
    pub base_url: String,
    pub rvc_dir: String,
    pub output_dir: String,
    pub weights_dir: String,
    pub logs_dir: String,
}

impl RvcConfig {
    pub fn from_env() -> Result<Self, PsmMcpError> {
        let base_url =
            std::env::var("RVC_URL").unwrap_or_else(|_| "http://localhost:7865".into());
        let rvc_dir = std::env::var("RVC_DIR")
            .unwrap_or_else(|_| "/Volumes/Virtual Server/projects/ai-music-rvc".into());
        let output_dir = std::env::var("RVC_OUTPUT_DIR").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{}/Desktop/AI-Music", home)
        });

        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(PsmMcpError::Config(
                "RVC_URL must start with http:// or https://".into(),
            ));
        }

        if !std::path::Path::new(&rvc_dir).is_dir() {
            eprintln!("[rvc-mcp] Warning: RVC_DIR does not exist: {}", rvc_dir);
        }

        if output_dir.is_empty() {
            return Err(PsmMcpError::Config(
                "RVC_OUTPUT_DIR must not be empty".into(),
            ));
        }

        let weights_dir = format!("{}/assets/weights", rvc_dir);
        let logs_dir = format!("{}/logs", rvc_dir);
        Ok(Self {
            base_url,
            rvc_dir,
            output_dir,
            weights_dir,
            logs_dir,
        })
    }
}

/// HTTP client for RVC's Gradio API.
pub struct GradioClient {
    client: reqwest::Client,
    base_url: String,
}

impl GradioClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
        }
    }

    pub async fn health_check(&self) -> bool {
        self.client
            .get(&self.base_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Ensure the WebUI is reachable, or return a ShellExec error.
    pub async fn require_healthy(&self) -> Result<(), PsmMcpError> {
        if !self.health_check().await {
            return Err(PsmMcpError::ShellExec(format!(
                "RVC WebUI not running at {}",
                self.base_url
            )));
        }
        Ok(())
    }

    pub async fn call(&self, api_name: &str, data: &Value) -> Result<Value, PsmMcpError> {
        let url = format!("{}/gradio_api/call/{}", self.base_url, api_name);
        let post_res = self
            .client
            .post(&url)
            .json(&json!({"data": data}))
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| {
                eprintln!("[rvc-mcp] Gradio POST error: {e}");
                PsmMcpError::ShellExec(Self::redact_connection_error(&e))
            })?;
        if !post_res.status().is_success() {
            let status = post_res.status();
            let _body = post_res.text().await.unwrap_or_default();
            eprintln!("[rvc-mcp] Gradio POST {api_name} failed: {status} {_body}");
            return Err(PsmMcpError::ShellExec(format!(
                "Gradio call to {api_name} failed (HTTP {status})"
            )));
        }
        let resp: Value = post_res
            .json()
            .await
            .map_err(|_| PsmMcpError::ShellExec("Failed to parse Gradio response".into()))?;
        let event_id = resp["event_id"]
            .as_str()
            .ok_or_else(|| PsmMcpError::ShellExec("No event_id in response".into()))?;

        let sse_url = format!("{}/{}", url, event_id);
        let sse_res = self
            .client
            .get(&sse_url)
            .timeout(std::time::Duration::from_secs(600))
            .send()
            .await
            .map_err(|e| {
                eprintln!("[rvc-mcp] Gradio SSE error: {e}");
                PsmMcpError::ShellExec(Self::redact_connection_error(&e))
            })?;
        let body = sse_res
            .text()
            .await
            .map_err(|_| PsmMcpError::ShellExec("Failed to read SSE stream".into()))?;
        for line in body.lines() {
            if let Some(data_str) = line.strip_prefix("data: ") {
                if let Ok(v) = serde_json::from_str::<Value>(data_str) {
                    return Ok(v);
                }
            }
        }
        Err(PsmMcpError::ShellExec(format!(
            "No complete event from Gradio for {api_name}"
        )))
    }

    fn redact_connection_error(e: &reqwest::Error) -> String {
        if e.is_connect() {
            "Service unavailable: cannot connect to RVC WebUI".to_string()
        } else if e.is_timeout() {
            "Service unavailable: request timed out".to_string()
        } else {
            "Service unavailable".to_string()
        }
    }
}

/// Shared state passed to all tool handlers.
pub struct SharedState {
    pub config: RvcConfig,
    pub gradio: GradioClient,
}

impl SharedState {
    pub fn new(config: RvcConfig) -> Arc<Self> {
        let gradio = GradioClient::new(&config.base_url);
        Arc::new(Self { config, gradio })
    }
}
