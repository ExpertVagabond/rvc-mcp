//! Tool handler implementations for all 12 RVC MCP tools.

use crate::config::SharedState;
use async_trait::async_trait;
use psm_mcp_core::error::PsmMcpError;
use psm_mcp_core::input::{optional_string, require_string, validate_input_size, MAX_INPUT_BYTES};
use psm_mcp_core::tool::{ToolDefinition, ToolHandler, ToolResult};
use serde_json::{json, Value};
use std::sync::Arc;

// ---------- helpers ----------

fn validate_path(path: &str, field: &str) -> Result<(), PsmMcpError> {
    validate_input_size(path, MAX_INPUT_BYTES)?;
    if path.len() > 1024 {
        return Err(PsmMcpError::InputValidation(format!(
            "{field} exceeds max path length of 1024"
        )));
    }
    if path.contains("..") {
        return Err(PsmMcpError::InputValidation(format!(
            "{field} contains path traversal sequence"
        )));
    }
    if path.contains('\0') {
        return Err(PsmMcpError::InputValidation(format!(
            "{field} contains null bytes"
        )));
    }
    Ok(())
}

// ---------- rvc_status ----------

pub struct RvcStatusTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcStatusTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_status".into(),
            description: "Check if RVC WebUI is running and responsive".into(),
            input_schema: json!({"type": "object", "properties": {}}),
        }
    }

    async fn handle(&self, _args: Value) -> Result<ToolResult, PsmMcpError> {
        let up = self.state.gradio.health_check().await;
        if up {
            Ok(ToolResult::text(format!(
                "RVC WebUI is running at {}",
                self.state.config.base_url
            )))
        } else {
            Ok(ToolResult::text(format!(
                "RVC WebUI is NOT running at {}\n\nStart it with:\n  cd \"{}\" && source .venv/bin/activate && python web.py --pycmd python --noautoopen",
                self.state.config.base_url, self.state.config.rvc_dir
            )))
        }
    }
}

// ---------- rvc_clean ----------

pub struct RvcCleanTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcCleanTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_clean".into(),
            description: "Clean up temporary files in the RVC directory (audio temp, logs)".into(),
            input_schema: json!({"type": "object", "properties": {}}),
        }
    }

    async fn handle(&self, _args: Value) -> Result<ToolResult, PsmMcpError> {
        let mut cleaned = Vec::new();
        for dir in &["TEMP", "temp"] {
            let p = format!("{}/{}", self.state.config.rvc_dir, dir);
            if std::path::Path::new(&p).exists() {
                let _ = std::fs::remove_dir_all(&p);
                cleaned.push(p);
            }
        }
        if cleaned.is_empty() {
            Ok(ToolResult::text("No temporary files to clean"))
        } else {
            Ok(ToolResult::text(format!(
                "Cleaned: {}",
                cleaned.join(", ")
            )))
        }
    }
}

// ---------- rvc_list_models ----------

pub struct RvcListModelsTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcListModelsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_list_models".into(),
            description: "List available RVC voice models (.pth files) in the weights directory"
                .into(),
            input_schema: json!({"type": "object", "properties": {}}),
        }
    }

    async fn handle(&self, _args: Value) -> Result<ToolResult, PsmMcpError> {
        let weights_dir = &self.state.config.weights_dir;
        match std::fs::read_dir(weights_dir) {
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
                    Ok(ToolResult::text(format!(
                        "No models found in {}",
                        weights_dir
                    )))
                } else {
                    Ok(ToolResult::text(format!(
                        "Models ({}):\n{}",
                        models.len(),
                        models.join("\n")
                    )))
                }
            }
            Err(e) => Ok(ToolResult::text(format!(
                "Cannot read weights dir {}: {e}",
                weights_dir
            ))),
        }
    }
}

// ---------- rvc_model_info ----------

pub struct RvcModelInfoTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcModelInfoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_model_info".into(),
            description:
                "Get info about a specific RVC voice model (file size, date, associated index)"
                    .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "model_name": { "type": "string", "description": "Model filename (e.g. 'matthew.pth')" }
                },
                "required": ["model_name"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        let model_name = require_string(&args, "model_name")?;
        validate_input_size(&model_name, MAX_INPUT_BYTES)?;
        let path = format!("{}/{}", self.state.config.weights_dir, model_name);
        match std::fs::metadata(&path) {
            Ok(m) => {
                let size_mb = m.len() as f64 / 1_048_576.0;
                let index_glob = format!(
                    "{}/{}",
                    self.state.config.logs_dir,
                    model_name.replace(".pth", "")
                );
                let has_index = std::path::Path::new(&index_glob).exists();
                Ok(ToolResult::text(format!(
                    "Model: {model_name}\nSize: {size_mb:.1} MB\nIndex dir exists: {has_index}"
                )))
            }
            Err(_) => Err(PsmMcpError::NotFound(format!("Model not found: {path}"))),
        }
    }
}

// ---------- rvc_model_extract ----------

pub struct RvcModelExtractTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcModelExtractTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_model_extract".into(),
            description: "Extract a smaller model from a trained RVC model for sharing".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "model_path": { "type": "string", "description": "Path to the model to extract" },
                    "output_name": { "type": "string", "description": "Name for the extracted model" },
                    "sample_rate": { "type": "string", "description": "Target sample rate: 32k, 40k, 48k", "default": "40k" },
                    "pitch_guidance": { "type": "boolean", "description": "Include pitch guidance", "default": true }
                },
                "required": ["model_path", "output_name"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        self.state.gradio.require_healthy().await?;
        let model_path = require_string(&args, "model_path")?;
        let output_name = require_string(&args, "output_name")?;
        validate_path(&model_path, "model_path")?;
        let sr = optional_string(&args, "sample_rate").unwrap_or_else(|| "40k".into());
        let pg = if args
            .get("pitch_guidance")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
        {
            1
        } else {
            0
        };
        let result = self
            .state
            .gradio
            .call(
                "extract_small_model",
                &json!([model_path, output_name, sr, pg]),
            )
            .await?;
        ToolResult::json(&result)
    }
}

// ---------- rvc_model_merge ----------

pub struct RvcModelMergeTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcModelMergeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_model_merge".into(),
            description: "Merge two RVC models together with a specified ratio".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "model_a": { "type": "string", "description": "First model name" },
                    "model_b": { "type": "string", "description": "Second model name" },
                    "ratio": { "type": "number", "description": "Merge ratio (0=all A, 1=all B)", "default": 0.5 },
                    "output_name": { "type": "string", "description": "Name for merged model" }
                },
                "required": ["model_a", "model_b", "output_name"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        self.state.gradio.require_healthy().await?;
        let model_a = require_string(&args, "model_a")?;
        let model_b = require_string(&args, "model_b")?;
        let output_name = require_string(&args, "output_name")?;
        let ratio = args
            .get("ratio")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
        let result = self
            .state
            .gradio
            .call("merge", &json!([model_a, model_b, ratio, output_name]))
            .await?;
        ToolResult::json(&result)
    }
}

// ---------- rvc_export_onnx ----------

pub struct RvcExportOnnxTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcExportOnnxTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_export_onnx".into(),
            description: "Export an RVC model to ONNX format for deployment".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "model_path": { "type": "string", "description": "Path to RVC model" },
                    "output_path": { "type": "string", "description": "Output ONNX file path" }
                },
                "required": ["model_path", "output_path"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        self.state.gradio.require_healthy().await?;
        let model_path = require_string(&args, "model_path")?;
        let output_path = require_string(&args, "output_path")?;
        validate_path(&model_path, "model_path")?;
        validate_path(&output_path, "output_path")?;
        let result = self
            .state
            .gradio
            .call("export_onnx", &json!([model_path, output_path]))
            .await?;
        ToolResult::json(&result)
    }
}

// ---------- rvc_infer ----------

pub struct RvcInferTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcInferTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_infer".into(),
            description: "Convert vocals to a target voice using RVC. Supports all F0 methods, index files, and advanced parameters.".into(),
            input_schema: json!({
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
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        self.state.gradio.require_healthy().await?;
        let model = require_string(&args, "model_name")?;
        let input = require_string(&args, "input_audio")?;
        validate_input_size(&model, MAX_INPUT_BYTES)?;
        validate_path(&input, "input_audio")?;

        let pitch = args.get("pitch").and_then(|v| v.as_i64()).unwrap_or(0);
        let f0 = optional_string(&args, "f0_method").unwrap_or_else(|| "rmvpe".into());
        let idx_path = optional_string(&args, "index_path").unwrap_or_default();
        let idx_rate = args
            .get("index_rate")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.75);
        let filt = args
            .get("filter_radius")
            .and_then(|v| v.as_i64())
            .unwrap_or(3);
        let resample = args
            .get("resample_sr")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let rms = args
            .get("rms_mix_rate")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.25);
        let protect = args
            .get("protect")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.33);

        // Load model
        let _ = self
            .state
            .gradio
            .call("infer_change_voice", &json!([model, protect, protect]))
            .await;
        // Run inference
        let result = self
            .state
            .gradio
            .call(
                "infer_convert",
                &json!([0, {"path": input}, pitch, null, f0, null, idx_path, idx_rate, filt, resample, rms, protect]),
            )
            .await?;
        ToolResult::json(&json!({"status": "success", "model": model, "input": input, "result": result}))
    }
}

// ---------- rvc_separate_vocals ----------

pub struct RvcSeparateVocalsTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcSeparateVocalsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_separate_vocals".into(),
            description: "Separate vocals from an audio file using UVR/Demucs".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "input_audio": { "type": "string", "description": "Path to input audio file" },
                    "model": { "type": "string", "description": "Separation model (HP5_only_main_vocal, etc.)", "default": "HP5_only_main_vocal" }
                },
                "required": ["input_audio"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        self.state.gradio.require_healthy().await?;
        let input = require_string(&args, "input_audio")?;
        validate_path(&input, "input_audio")?;
        let model =
            optional_string(&args, "model").unwrap_or_else(|| "HP5_only_main_vocal".into());
        let result = self
            .state
            .gradio
            .call("uvr", &json!([model, {"path": input}]))
            .await?;
        ToolResult::json(&json!({"status": "success", "input": input, "result": result}))
    }
}

// ---------- rvc_preprocess ----------

pub struct RvcPreprocessTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcPreprocessTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_preprocess".into(),
            description: "Preprocess training audio: slice into segments, normalize volume, resample. First step of the RVC training pipeline.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "training_folder": { "type": "string", "description": "Path to folder containing training audio files (WAV/MP3/FLAC)" },
                    "experiment_name": { "type": "string", "description": "Name for this training experiment (used as folder name in logs/)" },
                    "sample_rate": { "type": "string", "description": "Target sample rate: 32k, 40k, 48k", "default": "48k" },
                    "cpu_processes": { "type": "integer", "description": "Number of CPU processes for parallel slicing (1-16)", "default": 4, "minimum": 1, "maximum": 16 }
                },
                "required": ["training_folder", "experiment_name"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        self.state.gradio.require_healthy().await?;
        let folder = require_string(&args, "training_folder")?;
        let exp = require_string(&args, "experiment_name")?;
        validate_path(&folder, "training_folder")?;
        validate_input_size(&exp, MAX_INPUT_BYTES)?;
        let sr = optional_string(&args, "sample_rate").unwrap_or_else(|| "48k".into());
        let cpu = args
            .get("cpu_processes")
            .and_then(|v| v.as_i64())
            .unwrap_or(4)
            .clamp(1, 16);
        let result = self
            .state
            .gradio
            .call("train_preprocess", &json!([folder, exp, sr, cpu]))
            .await?;
        ToolResult::json(&json!({"status": "success", "experiment": exp, "training_folder": folder, "result": result}))
    }
}

// ---------- rvc_extract_features ----------

pub struct RvcExtractFeaturesTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcExtractFeaturesTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_extract_features".into(),
            description: "Extract pitch and voice features from preprocessed training data".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "experiment_name": { "type": "string", "description": "Experiment name from preprocessing" },
                    "f0_method": { "type": "string", "description": "Pitch extraction method", "default": "rmvpe" },
                    "version": { "type": "string", "description": "Model version: v1 or v2", "default": "v2" }
                },
                "required": ["experiment_name"]
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        self.state.gradio.require_healthy().await?;
        let exp = require_string(&args, "experiment_name")?;
        validate_input_size(&exp, MAX_INPUT_BYTES)?;
        let f0 = optional_string(&args, "f0_method").unwrap_or_else(|| "rmvpe".into());
        let ver = optional_string(&args, "version").unwrap_or_else(|| "v2".into());
        let result = self
            .state
            .gradio
            .call("extract_f0_feature", &json!([exp, f0, ver]))
            .await?;
        ToolResult::json(&json!({"status": "success", "experiment": exp, "result": result}))
    }
}

// ---------- rvc_train ----------

pub struct RvcTrainTool {
    pub state: Arc<SharedState>,
}

#[async_trait]
impl ToolHandler for RvcTrainTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "rvc_train".into(),
            description: "Train an RVC voice model on preprocessed and extracted data".into(),
            input_schema: json!({
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
            }),
        }
    }

    async fn handle(&self, args: Value) -> Result<ToolResult, PsmMcpError> {
        self.state.gradio.require_healthy().await?;
        let exp = require_string(&args, "experiment_name")?;
        validate_input_size(&exp, MAX_INPUT_BYTES)?;
        let epochs = args
            .get("epochs")
            .and_then(|v| v.as_i64())
            .unwrap_or(200);
        let batch = args
            .get("batch_size")
            .and_then(|v| v.as_i64())
            .unwrap_or(8);
        let save_freq = args
            .get("save_frequency")
            .and_then(|v| v.as_i64())
            .unwrap_or(25);
        let sr = optional_string(&args, "sample_rate").unwrap_or_else(|| "40k".into());
        let ver = optional_string(&args, "version").unwrap_or_else(|| "v2".into());
        let result = self
            .state
            .gradio
            .call(
                "click_train",
                &json!([exp, sr, true, epochs, save_freq, batch, true, "no", ver, "v2"]),
            )
            .await?;
        ToolResult::json(&json!({"status": "success", "experiment": exp, "epochs": epochs, "result": result}))
    }
}
