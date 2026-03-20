import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";

import type { McpAction } from "./types.js";
import { errorResult } from "./types.js";
import { loadConfig } from "./config.js";

// Server management
import { rvcStatus } from "./tools/server/rvc-status.js";
import { rvcClean } from "./tools/server/rvc-clean.js";

// Model management
import { rvcListModels } from "./tools/models/rvc-list-models.js";
import { rvcModelInfo } from "./tools/models/rvc-model-info.js";
import { rvcModelExtract } from "./tools/models/rvc-model-extract.js";
import { rvcModelMerge } from "./tools/models/rvc-model-merge.js";
import { rvcExportOnnx } from "./tools/models/rvc-export-onnx.js";

// Inference
import { rvcInfer } from "./tools/inference/rvc-infer.js";

// Separation
import { rvcSeparateVocals } from "./tools/separation/rvc-separate.js";

// Training
import { rvcPreprocess } from "./tools/training/rvc-preprocess.js";
import { rvcExtractFeatures } from "./tools/training/rvc-extract.js";
import { rvcTrain } from "./tools/training/rvc-train.js";

const actions: McpAction[] = [
  rvcStatus,
  rvcClean,
  rvcListModels,
  rvcModelInfo,
  rvcModelExtract,
  rvcModelMerge,
  rvcExportOnnx,
  rvcInfer,
  rvcSeparateVocals,
  rvcPreprocess,
  rvcExtractFeatures,
  rvcTrain,
];

// Build allowlist of valid tool names from registered actions
const VALID_TOOL_NAMES = new Set(actions.map((a) => a.tool.name));

/** Redact sensitive paths and config values from log messages */
function redactForLog(msg: string): string {
  return msg
    .replace(/\/[\w/.:-]+/g, "[path]")
    .replace(/\b\d{1,3}(\.\d{1,3}){3}(:\d+)?\b/g, "[host]")
    .replace(/[A-Za-z0-9+/=]{40,}/g, "[redacted]");
}

/** Safe stderr logger that redacts sensitive info */
function safeLog(msg: string): void {
  process.stderr.write(`${redactForLog(msg)}\n`);
}

const server = new Server(
  {
    name: "rvc-webui",
    version: "0.1.0",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: actions.map((a) => a.tool),
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const toolName = request.params.name;

  // Validate tool name against allowlist
  if (typeof toolName !== "string" || !VALID_TOOL_NAMES.has(toolName)) {
    safeLog(`[rvc-mcp] Rejected unknown tool request: ${String(toolName)}`);
    return errorResult("Unknown tool");
  }

  // Validate arguments is an object (not array, not primitive)
  const args = request.params.arguments;
  if (args !== undefined && (typeof args !== "object" || args === null || Array.isArray(args))) {
    return errorResult("Invalid arguments: expected an object");
  }

  const action = actions.find((a) => a.tool.name === toolName)!;
  try {
    return await action.handler(request);
  } catch (e) {
    // Log full error to stderr (redacted), return generic message to caller
    const fullMsg = e instanceof Error ? e.message : String(e);
    safeLog(`[rvc-mcp] Tool ${toolName} error: ${fullMsg}`);
    return errorResult(`Tool execution failed: ${toolName}`);
  }
});

const config = loadConfig();
// Log startup info without exposing full paths
process.stderr.write(`[rvc-mcp] Starting with ${actions.length} tools\n`);
process.stderr.write(`[rvc-mcp] WebUI: ${config.baseUrl}\n`);

const transport = new StdioServerTransport();
await server.connect(transport);
