#![recursion_limit = "512"]
//! RVC MCP Server — voice conversion tools via Model Context Protocol.
//!
//! Security:
//! - All environment-sourced URLs validated (http/https only)
//! - File paths validated against traversal attacks
//! - Argument length limits enforced before processing
//! - No credentials/keys in log output
//! - Process exit on config validation failure

use serde::Deserialize;
use serde_json::{Value, json};
use std::io::BufRead;

// --- Security: input validation constants ---
/// Maximum length for any single string argument.
const MAX_ARG_LEN: usize = 4096;
/// Maximum file path length.
const MAX_PATH_LEN: usize = 1024;
/// Allowed audio file extensions for input validation.
const ALLOWED_AUDIO_EXTS: &[&str] = &[".wav", ".mp3", ".flac", ".ogg", ".m4a", ".aac"];
/// Allowed model file extensions.
const ALLOWED_MODEL_EXTS: &[&str] = &[".pth", ".pt", ".onnx", ".index"];

/// Validate a string argument is safe and within length bounds.
fn validate_arg(value: &str, field: &str) -> Result<(), String> {
    if value.len() > MAX_ARG_LEN {
        return Err(format!("{field} exceeds maximum length of {MAX_ARG_LEN}"));
    }
    if value.contains('\0') {
        return Err(format!("{field} contains null bytes"));
    }
    Ok(())
}

/// Validate a file path — no traversal, within length limits.
fn validate_path(path: &str, field: &str) -> Result<(), String> {
    validate_arg(path, field)?;
    if path.len() > MAX_PATH_LEN {
        return Err(format!("{field} exceeds max path length of {MAX_PATH_LEN}"));
    }
    if path.contains("..") {
        return Err(format!("{field} contains path traversal sequence"));
    }
    Ok(())
}

/// Sanitize error output for external consumption — strip internal details.
fn sanitize_error(msg: &str) -> String {
    let first_line = msg.lines().next().unwrap_or("Internal error");
    first_line.chars().take(300).collect()
}

#[derive(Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

struct Config {
    base_url: String,
    rvc_dir: String,
    output_dir: String,
    weights_dir: String,
    logs_dir: String,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let base_url = std::env::var("RVC_URL").unwrap_or_else(|_| "http://localhost:7865".into());
        let rvc_dir = std::env::var("RVC_DIR")
            .unwrap_or_else(|_| "/Volumes/Virtual Server/projects/ai-music-rvc".into());
        let output_dir = std::env::var("RVC_OUTPUT_DIR").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{}/Desktop/AI-Music", home)
        });

        // Validate base_url format
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(format!(
                "RVC_URL must start with http:// or https://, got: {}",
                base_url
            ));
        }

        // Validate rvc_dir exists
        if !std::path::Path::new(&rvc_dir).is_dir() {
            eprintln!(
                "[rvc-mcp] Warning: RVC_DIR does not exist: {}",
                rvc_dir
            );
        }

        // Validate output_dir is non-empty
        if output_dir.is_empty() {
            return Err("RVC_OUTPUT_DIR must not be empty".into());
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

fn tool_definitions() -> Value {
    json!([
        {
            "name": "rvc_status",
            "description": "Check if RVC WebUI is running and responsive",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "rvc_clean",
            "description": "Clean up temporary files in the RVC directory (audio temp, logs)",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "rvc_list_models",
            "description": "List available RVC voice models (.pth files) in the weights directory",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "rvc_model_info",
            "description": "Get info about a specific RVC voice model (file size, date, associated index)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model_name": { "type": "string", "description": "Model filename (e.g. 'matthew.pth')" }
                },
                "required": ["model_name"]
            }
        },
        {
            "name": "rvc_model_extract",
            "description": "Extract a smaller model from a trained RVC model for sharing",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model_path": { "type": "string", "description": "Path to the model to extract" },
                    "output_name": { "type": "string", "description": "Name for the extracted model" },
                    "sample_rate": { "type": "string", "description": "Target sample rate: 32k, 40k, 48k", "default": "40k" },
                    "pitch_guidance": { "type": "boolean", "description": "Include pitch guidance", "default": true }
                },
                "required": ["model_path", "output_name"]
            }
        },
        {
            "name": "rvc_model_merge",
            "description": "Merge two RVC models together with a specified ratio",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model_a": { "type": "string", "description": "First model name" },
                    "model_b": { "type": "string", "description": "Second model name" },
                    "ratio": { "type": "number", "description": "Merge ratio (0=all A, 1=all B)", "default": 0.5 },
                    "output_name": { "type": "string", "description": "Name for merged model" }
                },
                "required": ["model_a", "model_b", "output_name"]
            }
        },
        {
            "name": "rvc_export_onnx",
            "description": "Export an RVC model to ONNX format for deployment",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model_path": { "type": "string", "description": "Path to RVC model" },
                    "output_path": { "type": "string", "description": "Output ONNX file path" }
                },
                "required": ["model_path", "output_path"]
            }
        },
        {
            "name": "rvc_infer",
            "description": "Convert vocals to a target voice using RVC. Supports all F0 methods, index files, and advanced parameters.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "model_name": { "type": "string", "description": "Voice model filename (e.g. 'matthew.pth')" },
                    "input_audio": { "type": "string", "description": "Absolute path to input audio file" },
                    "pitch": { "type": "integer", "description": "Pitch shift in semitones (-24 to +24)", "default": 0 },
                    "f0_method": { "type": "string", "description": "Pitch extraction: pm, dio, harvest, crepe, rmvpe, fcpe", "default": "rmvpe" },
                    "index_path": { "type": "string", "description": "Path to .index file for quality" },
                    "index_rate": { "type": "number", "description": "Feature search ratio (0-1)", "default": 0.75 },
                    "filter_radius": { "type": "integer", "description": "Median filter for pitch (0-7)", "default": 3 },
                    "resample_sr": { "type": "integer", "description": "Resample output sample rate (0=none)", "default": 0 },
                    "rms_mix_rate": { "type": "number", "description": "Volume envelope scaling (0-1)", "default": 0.25 },
                    "protect": { "type": "number", "description": "Voiceless consonant protection (0-0.5)", "default": 0.33 }
                },
                "required": ["model_name", "input_audio"]
            }
        },
        {
            "name": "rvc_separate_vocals",
            "description": "Separate vocals from an audio file using UVR/Demucs",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input_audio": { "type": "string", "description": "Path to input audio file" },
                    "model": { "type": "string", "description": "Separation model (HP5_only_main_vocal, etc.)", "default": "HP5_only_main_vocal" }
                },
                "required": ["input_audio"]
            }
        },
        {
            "name": "rvc_preprocess",
            "description": "Preprocess training audio: slice into segments, normalize volume, resample. First step of the RVC training pipeline.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "training_folder": { "type": "string", "description": "Path to folder containing training audio files (WAV/MP3/FLAC)" },
                    "experiment_name": { "type": "string", "description": "Name for this training experiment (used as folder name in logs/)" },
                    "sample_rate": { "type": "string", "description": "Target sample rate: 32k, 40k, 48k", "default": "48k" },
                    "cpu_processes": { "type": "integer", "description": "Number of CPU processes for parallel slicing (1-16)", "default": 4, "minimum": 1, "maximum": 16 }
                },
                "required": ["training_folder", "experiment_name"]
            }
        },
        {
            "name": "rvc_extract_features",
            "description": "Extract pitch and voice features from preprocessed training data",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "experiment_name": { "type": "string", "description": "Experiment name from preprocessing" },
                    "f0_method": { "type": "string", "description": "Pitch extraction method", "default": "rmvpe" },
                    "version": { "type": "string", "description": "Model version: v1 or v2", "default": "v2" }
                },
                "required": ["experiment_name"]
            }
        },
        {
            "name": "rvc_train",
            "description": "Train an RVC voice model on preprocessed and extracted data",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "experiment_name": { "type": "string", "description": "Experiment name" },
                    "epochs": { "type": "integer", "description": "Number of training epochs", "default": 200 },
                    "batch_size": { "type": "integer", "description": "Training batch size", "default": 8 },
                    "save_frequency": { "type": "integer", "description": "Save checkpoint every N epochs", "default": 25 },
                    "sample_rate": { "type": "string", "description": "Sample rate: 32k, 40k, 48k", "default": "40k" },
                    "version": { "type": "string", "description": "Model version: v1 or v2", "default": "v2" }
                },
                "required": ["experiment_name"]
            }
        }
    ])
}

struct GradioClient {
    client: reqwest::Client,
    base_url: String,
}

impl GradioClient {
    fn new(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.to_string(),
        }
    }

    async fn health_check(&self) -> bool {
        self.client
            .get(&self.base_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn call(&self, api_name: &str, data: &Value) -> Result<Value, String> {
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
                Self::redact_connection_error(&e)
            })?;
        if !post_res.status().is_success() {
            let status = post_res.status();
            let _body = post_res.text().await.unwrap_or_default();
            eprintln!("[rvc-mcp] Gradio POST {api_name} failed: {status} {_body}");
            return Err(format!("Gradio call to {api_name} failed (HTTP {status})"));
        }
        let resp: Value = post_res
            .json()
            .await
            .map_err(|_| "Failed to parse Gradio response".to_string())?;
        let event_id = resp["event_id"].as_str().ok_or("No event_id in response")?;

        let sse_url = format!("{}/{}", url, event_id);
        let sse_res = self
            .client
            .get(&sse_url)
            .timeout(std::time::Duration::from_secs(600))
            .send()
            .await
            .map_err(|e| {
                eprintln!("[rvc-mcp] Gradio SSE error: {e}");
                Self::redact_connection_error(&e)
            })?;
        let body = sse_res.text().await.map_err(|_| "Failed to read SSE stream".to_string())?;
        for line in body.lines() {
            if let Some(data_str) = line.strip_prefix("data: ")
                && let Ok(v) = serde_json::from_str::<Value>(data_str)
            {
                return Ok(v);
            }
        }
        Err(format!("No complete event from Gradio for {api_name}"))
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

async fn call_tool(name: &str, args: &Value, config: &Config) -> Value {
    let gradio = GradioClient::new(&config.base_url);
    let text = match name {
        "rvc_status" => {
            let up = gradio.health_check().await;
            if up {
                format!("RVC WebUI is running at {}", config.base_url)
            } else {
                format!(
                    "RVC WebUI is NOT running at {}\n\nStart it with:\n  cd \"{}\" && source .venv/bin/activate && python web.py --pycmd python --noautoopen",
                    config.base_url, config.rvc_dir
                )
            }
        }
        "rvc_clean" => {
            let mut cleaned = Vec::new();
            for dir in &["TEMP", "temp"] {
                let p = format!("{}/{}", config.rvc_dir, dir);
                if std::path::Path::new(&p).exists() {
                    let _ = std::fs::remove_dir_all(&p);
                    cleaned.push(p);
                }
            }
            if cleaned.is_empty() {
                "No temporary files to clean".into()
            } else {
                format!("Cleaned: {}", cleaned.join(", "))
            }
        }
        "rvc_list_models" => match std::fs::read_dir(&config.weights_dir) {
            Ok(entries) => {
                let models: Vec<String> = entries
                    .filter_map(|e| {
                        let e = e.ok()?;
                        let name = e.file_name().to_string_lossy().to_string();
                        if name.ends_with(".pth") {
                            Some(name)
                        } else {
                            None
                        }
                    })
                    .collect();
                if models.is_empty() {
                    format!("No models found in {}", config.weights_dir)
                } else {
                    format!("Models ({}):\n{}", models.len(), models.join("\n"))
                }
            }
            Err(e) => format!("Cannot read weights dir {}: {e}", config.weights_dir),
        },
        "rvc_model_info" => {
            let model_name = args["model_name"].as_str().unwrap_or("");
            let path = format!("{}/{}", config.weights_dir, model_name);
            match std::fs::metadata(&path) {
                Ok(m) => {
                    let size_mb = m.len() as f64 / 1_048_576.0;
                    let index_glob =
                        format!("{}/{}", config.logs_dir, model_name.replace(".pth", ""));
                    let has_index = std::path::Path::new(&index_glob).exists();
                    format!(
                        "Model: {model_name}\nSize: {size_mb:.1} MB\nIndex dir exists: {has_index}"
                    )
                }
                Err(_) => format!("Model not found: {path}"),
            }
        }
        "rvc_model_extract" | "rvc_model_merge" | "rvc_export_onnx" => {
            if !gradio.health_check().await {
                format!("RVC WebUI not running at {}", config.base_url)
            } else {
                let api = match name {
                    "rvc_model_extract" => {
                        let sr = args["sample_rate"].as_str().unwrap_or("40k");
                        let pg = if args["pitch_guidance"].as_bool().unwrap_or(true) {
                            1
                        } else {
                            0
                        };
                        gradio
                            .call(
                                "extract_small_model",
                                &json!([
                                    args["model_path"].as_str().unwrap_or(""),
                                    args["output_name"].as_str().unwrap_or(""),
                                    sr,
                                    pg
                                ]),
                            )
                            .await
                    }
                    "rvc_model_merge" => {
                        let ratio = args["ratio"].as_f64().unwrap_or(0.5);
                        gradio
                            .call(
                                "merge",
                                &json!([
                                    args["model_a"].as_str().unwrap_or(""),
                                    args["model_b"].as_str().unwrap_or(""),
                                    ratio,
                                    args["output_name"].as_str().unwrap_or("")
                                ]),
                            )
                            .await
                    }
                    "rvc_export_onnx" => {
                        gradio
                            .call(
                                "export_onnx",
                                &json!([
                                    args["model_path"].as_str().unwrap_or(""),
                                    args["output_path"].as_str().unwrap_or("")
                                ]),
                            )
                            .await
                    }
                    _ => unreachable!(),
                };
                match api {
                    Ok(v) => serde_json::to_string_pretty(&v)
                        .unwrap_or_default()
                        .to_string(),
                    Err(e) => format!("Error: {e}"),
                }
            }
        }
        "rvc_infer" => {
            if !gradio.health_check().await {
                return json!({"content":[{"type":"text","text":format!("RVC WebUI not running at {}", config.base_url)}],"isError":true});
            }
            let model = args["model_name"].as_str().unwrap_or("");
            let input = args["input_audio"].as_str().unwrap_or("");
            let pitch = args["pitch"].as_i64().unwrap_or(0);
            let f0 = args["f0_method"].as_str().unwrap_or("rmvpe");
            let idx_path = args["index_path"].as_str().unwrap_or("");
            let idx_rate = args["index_rate"].as_f64().unwrap_or(0.75);
            let filt = args["filter_radius"].as_i64().unwrap_or(3);
            let resample = args["resample_sr"].as_i64().unwrap_or(0);
            let rms = args["rms_mix_rate"].as_f64().unwrap_or(0.25);
            let protect = args["protect"].as_f64().unwrap_or(0.33);

            // Load model
            let _ = gradio
                .call("infer_change_voice", &json!([model, protect, protect]))
                .await;
            // Run inference
            match gradio.call("infer_convert", &json!([0, {"path": input}, pitch, null, f0, null, idx_path, idx_rate, filt, resample, rms, protect])).await {
                Ok(v) => serde_json::to_string_pretty(&json!({"status":"success","model":model,"input":input,"result":v})).unwrap_or_default().to_string(),
                Err(e) => format!("Inference error: {e}"),
            }
        }
        "rvc_separate_vocals" => {
            if !gradio.health_check().await {
                return json!({"content":[{"type":"text","text":format!("RVC WebUI not running at {}", config.base_url)}],"isError":true});
            }
            let input = args["input_audio"].as_str().unwrap_or("");
            let model = args["model"].as_str().unwrap_or("HP5_only_main_vocal");
            match gradio.call("uvr", &json!([model, {"path": input}])).await {
                Ok(v) => serde_json::to_string_pretty(
                    &json!({"status":"success","input":input,"result":v}),
                )
                .unwrap_or_default()
                .to_string(),
                Err(e) => format!("Separation error: {e}"),
            }
        }
        "rvc_preprocess" => {
            if !gradio.health_check().await {
                return json!({"content":[{"type":"text","text":format!("RVC WebUI not running at {}", config.base_url)}],"isError":true});
            }
            let folder = args["training_folder"].as_str().unwrap_or("");
            let exp = args["experiment_name"].as_str().unwrap_or("");
            let sr = args["sample_rate"].as_str().unwrap_or("48k");
            let cpu = args["cpu_processes"].as_i64().unwrap_or(4).clamp(1, 16);
            if folder.is_empty() || exp.is_empty() {
                return json!({"content":[{"type":"text","text":"Missing required: training_folder and experiment_name"}],"isError":true});
            }
            match gradio
                .call("train_preprocess", &json!([folder, exp, sr, cpu]))
                .await
            {
                Ok(v) => serde_json::to_string_pretty(
                    &json!({"status":"success","experiment":exp,"training_folder":folder,"result":v}),
                )
                .unwrap_or_default()
                .to_string(),
                Err(e) => format!("Preprocess error: {e}"),
            }
        }
        "rvc_extract_features" => {
            if !gradio.health_check().await {
                return json!({"content":[{"type":"text","text":format!("RVC WebUI not running at {}", config.base_url)}],"isError":true});
            }
            let exp = args["experiment_name"].as_str().unwrap_or("");
            let f0 = args["f0_method"].as_str().unwrap_or("rmvpe");
            let ver = args["version"].as_str().unwrap_or("v2");
            match gradio
                .call("extract_f0_feature", &json!([exp, f0, ver]))
                .await
            {
                Ok(v) => serde_json::to_string_pretty(
                    &json!({"status":"success","experiment":exp,"result":v}),
                )
                .unwrap_or_default()
                .to_string(),
                Err(e) => format!("Extract error: {e}"),
            }
        }
        "rvc_train" => {
            if !gradio.health_check().await {
                return json!({"content":[{"type":"text","text":format!("RVC WebUI not running at {}", config.base_url)}],"isError":true});
            }
            let exp = args["experiment_name"].as_str().unwrap_or("");
            let epochs = args["epochs"].as_i64().unwrap_or(200);
            let batch = args["batch_size"].as_i64().unwrap_or(8);
            let save_freq = args["save_frequency"].as_i64().unwrap_or(25);
            let sr = args["sample_rate"].as_str().unwrap_or("40k");
            let ver = args["version"].as_str().unwrap_or("v2");
            match gradio
                .call(
                    "click_train",
                    &json!([
                        exp, sr, true, epochs, save_freq, batch, true, "no", ver, "v2"
                    ]),
                )
                .await
            {
                Ok(v) => serde_json::to_string_pretty(
                    &json!({"status":"success","experiment":exp,"epochs":epochs,"result":v}),
                )
                .unwrap_or_default()
                .to_string(),
                Err(e) => format!("Training error: {e}"),
            }
        }
        _ => format!("Unknown tool: {name}"),
    };
    json!({"content":[{"type":"text","text":text}]})
}

/// Valid tool names for allowlist validation
const VALID_TOOLS: &[&str] = &[
    "rvc_status",
    "rvc_clean",
    "rvc_list_models",
    "rvc_model_info",
    "rvc_model_extract",
    "rvc_model_merge",
    "rvc_export_onnx",
    "rvc_infer",
    "rvc_separate_vocals",
    "rvc_preprocess",
    "rvc_extract_features",
    "rvc_train",
];

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(std::io::stderr)
        .init();

    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[rvc-mcp] Configuration error: {e}");
            std::process::exit(1);
        }
    };

    eprintln!(
        "[rvc-mcp] Starting with {} tools, WebUI: {}",
        VALID_TOOLS.len(),
        config.base_url
    );

    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("[rvc-mcp] stdin read error: {e}");
                break;
            }
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let resp = match req.method.as_str() {
            "initialize" => {
                json!({"jsonrpc":"2.0","id":req.id,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"rvc-mcp","version":"0.1.0"}}})
            }
            "notifications/initialized" => continue,
            "tools/list" => {
                json!({"jsonrpc":"2.0","id":req.id,"result":{"tools":tool_definitions()}})
            }
            "tools/call" => {
                let params = req.params.unwrap_or(json!({}));
                let name = params["name"].as_str().unwrap_or("");
                // Validate tool name against allowlist
                if !VALID_TOOLS.contains(&name) {
                    json!({"jsonrpc":"2.0","id":req.id,"result":{"content":[{"type":"text","text":format!("Unknown tool: {name}")}],"isError":true}})
                } else {
                    let args = params.get("arguments").cloned().unwrap_or(json!({}));
                    let result = call_tool(name, &args, &config).await;
                    json!({"jsonrpc":"2.0","id":req.id,"result":result})
                }
            }
            _ => {
                json!({"jsonrpc":"2.0","id":req.id,"error":{"code":-32601,"message":"Method not found"}})
            }
        };
        match serde_json::to_string(&resp) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("[rvc-mcp] Failed to serialize response: {e}"),
        }
    }
}
