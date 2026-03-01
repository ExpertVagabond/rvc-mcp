import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCallStreaming, isConnectionError } from "../../gradio-client.js";

const schema = z.object({
  experiment_name: z
    .string()
    .describe("Experiment name (must match the name used in rvc_preprocess)"),
  f0_method: z
    .enum(["pm", "dio", "harvest", "crepe", "rmvpe", "fcpe"])
    .default("rmvpe")
    .describe("Pitch extraction method (rmvpe=best quality, pm=fastest)"),
  pitch_guidance: z
    .boolean()
    .default(true)
    .describe("Enable pitch guidance (required for singing voices)"),
  version: z.enum(["v1", "v2"]).default("v2").describe("RVC model version"),
  cpu_processes: z
    .number()
    .int()
    .min(1)
    .max(16)
    .default(4)
    .describe("Number of CPU processes for extraction"),
});

export const rvcExtractFeatures: McpAction = {
  tool: {
    name: "rvc_extract_features",
    description:
      "Extract F0 pitch curves and HuBERT features from preprocessed audio. Second step of the RVC training pipeline (run after rvc_preprocess).",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const args = schema.parse(request.params.arguments);
    const config = loadConfig();

    try {
      process.stderr.write(
        `[rvc-mcp] Extracting features for: ${args.experiment_name} (${args.f0_method}, ${args.version})\n`
      );

      const result = await gradioCallStreaming(
        "train_extract_f0_feature",
        [
          args.cpu_processes,
          args.f0_method,
          args.pitch_guidance ? "Yes" : "No",
          args.experiment_name,
          args.version,
        ],
        config,
        config.extractTimeout
      );

      return textResult({
        status: "success",
        experiment_name: args.experiment_name,
        f0_method: args.f0_method,
        version: args.version,
        log: result[0] ?? "Feature extraction complete",
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
