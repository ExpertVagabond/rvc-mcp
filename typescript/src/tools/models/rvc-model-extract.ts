import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCall, isConnectionError } from "../../gradio-client.js";

const schema = z.object({
  checkpoint_path: z
    .string()
    .describe("Path to large checkpoint .pth in logs/ folder (e.g., logs/my-model/G_23333.pth)"),
  save_name: z.string().describe("Output model name (without extension)"),
  sample_rate: z.enum(["32k", "40k", "48k"]).default("48k"),
  pitch_guidance: z.boolean().default(true),
  version: z.enum(["v1", "v2"]).default("v2"),
  info: z.string().default("").describe("Model information text"),
  author: z.string().default("").describe("Model author"),
});

export const rvcModelExtract: McpAction = {
  tool: {
    name: "rvc_model_extract",
    description:
      "Extract a small portable voice model from a large training checkpoint. Produces a .pth in assets/weights/.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const args = schema.parse(request.params.arguments);
    const config = loadConfig();

    try {
      const result = await gradioCall(
        "ckpt_extract",
        [
          { path: args.checkpoint_path },
          args.save_name,
          args.author,
          args.sample_rate,
          args.pitch_guidance ? "1" : "0",
          args.info,
          args.version,
        ],
        config,
        config.modelOpTimeout
      );

      return textResult({
        status: "success",
        save_name: args.save_name,
        message: result[0] ?? "Model extracted",
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
