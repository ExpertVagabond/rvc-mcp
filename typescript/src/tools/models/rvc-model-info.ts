import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCall, isConnectionError } from "../../gradio-client.js";
import { resolve } from "node:path";

const schema = z.object({
  model_path: z
    .string()
    .describe(
      "Path to .pth model file (absolute, or relative to RVC weights dir)"
    ),
});

export const rvcModelInfo: McpAction = {
  tool: {
    name: "rvc_model_info",
    description:
      "View information about an RVC voice model (.pth file): architecture, sample rate, training info.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const { model_path } = schema.parse(request.params.arguments);
    const config = loadConfig();

    const absPath = model_path.startsWith("/")
      ? model_path
      : resolve(config.weightsDir, model_path);

    try {
      const result = await gradioCall(
        "ckpt_show",
        [{ path: absPath }],
        config,
        config.modelOpTimeout
      );

      return textResult({
        model_path: absPath,
        info: result[0] ?? "No info returned",
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
