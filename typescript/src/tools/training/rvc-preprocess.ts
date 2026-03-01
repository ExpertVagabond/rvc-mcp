import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCallStreaming, isConnectionError } from "../../gradio-client.js";
import { existsSync } from "node:fs";

const schema = z.object({
  training_folder: z
    .string()
    .describe("Path to folder containing training audio files (WAV/MP3/FLAC)"),
  experiment_name: z
    .string()
    .describe("Name for this training experiment (used as folder name in logs/)"),
  sample_rate: z
    .enum(["32k", "40k", "48k"])
    .default("48k")
    .describe("Target sample rate for training"),
  cpu_processes: z
    .number()
    .int()
    .min(1)
    .max(16)
    .default(4)
    .describe("Number of CPU processes for parallel slicing"),
});

export const rvcPreprocess: McpAction = {
  tool: {
    name: "rvc_preprocess",
    description:
      "Preprocess training audio: slice into segments, normalize volume, resample. First step of the RVC training pipeline.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const args = schema.parse(request.params.arguments);
    const config = loadConfig();

    if (!existsSync(args.training_folder)) {
      return errorResult(
        `Training folder not found: ${args.training_folder}`
      );
    }

    try {
      process.stderr.write(
        `[rvc-mcp] Preprocessing: ${args.training_folder} → ${args.experiment_name}\n`
      );

      const result = await gradioCallStreaming(
        "train_preprocess",
        [
          args.training_folder,
          args.experiment_name,
          args.sample_rate,
          args.cpu_processes,
        ],
        config,
        config.preprocessTimeout
      );

      return textResult({
        status: "success",
        experiment_name: args.experiment_name,
        training_folder: args.training_folder,
        sample_rate: args.sample_rate,
        log: result[0] ?? "Preprocessing complete",
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
