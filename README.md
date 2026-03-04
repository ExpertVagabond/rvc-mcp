# rvc-mcp

MCP server for [RVC WebUI](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI) — voice conversion, training, model management, and vocal separation through Claude Code or any MCP client. Communicates with RVC via its Gradio API.

## Tools (12)

### Inference & Separation

| Tool | Description |
|---|---|
| `rvc_infer` | Convert vocals to a target voice (all F0 methods, index support, pitch shift) |
| `rvc_separate_vocals` | Separate vocals from accompaniment using UVR5 (HP2/HP3/HP5/MDX-Net/DeEcho) |

### Training Pipeline

| Tool | Description |
|---|---|
| `rvc_preprocess` | Step 1: Slice audio into segments, normalize, resample |
| `rvc_extract_features` | Step 2: Extract F0 pitch curves and HuBERT features |
| `rvc_train` | Step 3: Train RVC model and build FAISS index (30-60+ min) |

### Model Management

| Tool | Description |
|---|---|
| `rvc_list_models` | List available .pth voice models and .index files |
| `rvc_model_info` | View model metadata (architecture, sample rate, training info) |
| `rvc_model_extract` | Extract portable model from large training checkpoint |
| `rvc_model_merge` | Blend two voice models by weighted average |
| `rvc_export_onnx` | Convert model to ONNX format for cross-platform inference |

### Server Management

| Tool | Description |
|---|---|
| `rvc_status` | Check if RVC WebUI is running, get start command |
| `rvc_clean` | Unload voice models from memory (free RAM/VRAM) |

## Quick Start

### Prerequisites

- Node.js 18+
- [RVC WebUI](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI) running on localhost:7865
- Python 3.11+ with PyTorch

### Install

```bash
npm install -g rvc-mcp
```

### Start RVC WebUI

```bash
cd /path/to/rvc-webui
source .venv/bin/activate
python web.py --pycmd python --noautoopen
# WebUI runs on http://localhost:7865
```

### Configure in Claude Code

Add to `~/.mcp.json`:

```json
{
  "mcpServers": {
    "rvc-webui": {
      "command": "rvc-mcp",
      "env": {
        "RVC_URL": "http://localhost:7865",
        "RVC_DIR": "/path/to/rvc-webui",
        "RVC_OUTPUT_DIR": "~/Desktop/AI-Music"
      }
    }
  }
}
```

### Or run from source

```bash
git clone https://github.com/ExpertVagabond/rvc-mcp.git
cd rvc-mcp/typescript
npm install && npm run build
node build/index.js
```

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `RVC_URL` | `http://localhost:7865` | RVC WebUI base URL |
| `RVC_DIR` | — | Path to RVC WebUI installation |
| `RVC_OUTPUT_DIR` | `~/Desktop/AI-Music` | Output directory |

## Training Workflow

Train a custom voice model in 3 steps:

```
1. rvc_preprocess  → Slice + normalize training audio
2. rvc_extract_features → Extract F0 + HuBERT features
3. rvc_train → Train model (epochs, batch size, checkpoints)
```

### Example: Train a voice from 90 seconds of audio

```
Step 1: rvc_preprocess
  training_folder: /path/to/audio-samples/
  experiment_name: my-voice
  sample_rate: 48k

Step 2: rvc_extract_features
  experiment_name: my-voice
  f0_method: rmvpe
  version: v2

Step 3: rvc_train
  experiment_name: my-voice
  training_folder: /path/to/audio-samples/
  total_epochs: 20
  batch_size: 4
  save_every_epoch: 5
```

Result: `my-voice.pth` in `assets/weights/`, ready for inference.

## Inference Parameters

`rvc_infer` supports full control:

| Parameter | Default | Description |
|---|---|---|
| `model_name` | — | Voice model filename (.pth) |
| `input_audio` | — | Path to input audio file |
| `pitch` | 0 | Semitone shift (-24 to +24) |
| `f0_method` | rmvpe | pm, dio, harvest, crepe, rmvpe, fcpe |
| `index_path` | — | .index file for quality boost |
| `index_rate` | 0.75 | Feature search ratio (0-1) |
| `filter_radius` | 3 | Median filter for pitch (0-7) |
| `rms_mix_rate` | 0.25 | Volume envelope scaling (0-1) |
| `protect` | 0.33 | Voiceless consonant protection (0-0.5) |

## Timeouts

| Operation | Timeout |
|---|---|
| Inference | 5 min |
| Vocal separation | 10 min |
| Preprocessing | 10 min |
| Feature extraction | 30 min |
| Training | 60 min |
| Model operations | 2 min |

## Related Projects

- [ai-music-mcp](https://github.com/ExpertVagabond/ai-music-mcp) — MCP server for MusicGen + Demucs + RVC pipeline
- [ai-music-studio](https://github.com/ExpertVagabond/ai-music-studio) — CLI for the same tools
- [music-distro](https://github.com/ExpertVagabond/music-distro) — MCP server for SoundCloud/YouTube distribution

## License

MIT — Purple Squirrel Media
