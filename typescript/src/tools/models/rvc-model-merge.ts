import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCall, isConnectionError } from "../../gradio-client.js";

const schema = z.object({
  model_a_path: z.string().describe("Path to first .pth voice model"),
  model_b_path: z.string().describe("Path to second .pth voice model"),
  weight_a: z
    .number()
    .min(0)
    .max(1)
    .default(0.5)
    .describe("Weight for model A (0-1, 0.5 = equal blend)"),
  output_name: z.string().describe("Output model name (without extension)"),
  sample_rate: z.enum(["32k", "40k", "48k"]).default("48k"),
  pitch_guidance: z.boolean().default(true),
  version: z.enum(["v1", "v2"]).default("v2"),
  info: z.string().default("").describe("Model info text"),
});

export const rvcModelMerge: McpAction = {
  tool: {
    name: "rvc_model_merge",
    description:
      "Fuse two RVC voice models by weighted average. Creates a blended voice model combining characteristics of both.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const args = schema.parse(request.params.arguments);
    const config = loadConfig();

    try {
      const result = await gradioCall(
        "ckpt_merge",
        [
          { path: args.model_a_path },
          { path: args.model_b_path },
          args.weight_a,
          args.sample_rate,
          args.pitch_guidance ? "Yes" : "No",
          args.info,
          args.output_name,
          args.version,
        ],
        config,
        config.modelOpTimeout
      );

      return textResult({
        status: "success",
        output_name: args.output_name,
        weight_a: args.weight_a,
        weight_b: 1 - args.weight_a,
        message: result[0] ?? "Models merged",
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
