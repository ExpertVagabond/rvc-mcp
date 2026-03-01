import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCall, isConnectionError } from "../../gradio-client.js";

const schema = z.object({
  model_path: z.string().describe("Path to RVC .pth model file"),
  output_path: z.string().describe("Output path for the ONNX file"),
});

export const rvcExportOnnx: McpAction = {
  tool: {
    name: "rvc_export_onnx",
    description:
      "Export an RVC voice model to ONNX format for cross-platform inference.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const args = schema.parse(request.params.arguments);
    const config = loadConfig();

    try {
      const result = await gradioCall(
        "export_onnx",
        [args.model_path, args.output_path],
        config,
        300_000
      );

      return textResult({
        status: "success",
        model_path: args.model_path,
        output_path: args.output_path,
        message: result[0] ?? "Exported to ONNX",
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
