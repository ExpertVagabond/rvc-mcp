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
  const action = actions.find((a) => a.tool.name === request.params.name);
  if (!action) {
    return errorResult(`Unknown tool: ${request.params.name}`);
  }
  return action.handler(request);
});

const config = loadConfig();
console.error(`[rvc-mcp] Starting with ${actions.length} tools`);
console.error(`[rvc-mcp] WebUI: ${config.baseUrl}`);
console.error(`[rvc-mcp] RVC dir: ${config.rvcDir}`);

const transport = new StdioServerTransport();
await server.connect(transport);
