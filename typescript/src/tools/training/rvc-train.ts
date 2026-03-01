import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCallStreaming, isConnectionError } from "../../gradio-client.js";
import { resolve } from "node:path";

const schema = z.object({
  experiment_name: z
    .string()
    .describe("Experiment name (must match previous preprocess/extract steps)"),
  training_folder: z
    .string()
    .describe("Path to folder with training audio (same as used in rvc_preprocess)"),
  sample_rate: z.enum(["32k", "40k", "48k"]).default("48k"),
  pitch_guidance: z.boolean().default(true),
  speaker_id: z
    .number()
    .int()
    .min(0)
    .max(4)
    .default(0)
    .describe("Speaker ID (0-4, use 0 for single speaker)"),
  cpu_processes: z.number().int().min(1).max(16).default(4),
  f0_method: z
    .enum(["pm", "dio", "harvest", "crepe", "rmvpe", "fcpe"])
    .default("rmvpe"),
  save_every_epoch: z
    .number()
    .int()
    .min(1)
    .max(50)
    .default(5)
    .describe("Save checkpoint every N epochs"),
  total_epochs: z
    .number()
    .int()
    .min(2)
    .max(1000)
    .default(20)
    .describe("Total training epochs"),
  batch_size: z
    .number()
    .int()
    .min(1)
    .max(40)
    .default(4)
    .describe("Training batch size (lower=less RAM)"),
  save_only_latest: z
    .boolean()
    .default(false)
    .describe("Only keep the latest checkpoint (saves disk space)"),
  cache_to_gpu: z
    .boolean()
    .default(false)
    .describe("Cache training data to GPU (faster but uses more VRAM)"),
  save_every_weights: z
    .boolean()
    .default(true)
    .describe("Save small model weights at each save interval"),
  version: z.enum(["v1", "v2"]).default("v2"),
  author: z.string().default("").describe("Model author name"),
  gpus: z
    .string()
    .default("0")
    .describe("GPU index(es) separated by '-' (e.g., '0' or '0-1')"),
});

function pretrainedPath(rvcDir: string, version: string, sr: string, type: string, f0: boolean): string {
  const prefix = f0 ? "f0" : "";
  return resolve(rvcDir, `assets/pretrained${version === "v2" ? "_v2" : ""}/${prefix}${type}${sr}.pth`);
}

export const rvcTrain: McpAction = {
  tool: {
    name: "rvc_train",
    description:
      "Train an RVC voice model and build FAISS index. Final step of the pipeline (run after rvc_preprocess and rvc_extract_features). WARNING: This can take 30-60+ minutes depending on epochs and data size.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const args = schema.parse(request.params.arguments);
    const config = loadConfig();

    const pretrainedG = pretrainedPath(
      config.rvcDir,
      args.version,
      args.sample_rate,
      "G",
      args.pitch_guidance
    );
    const pretrainedD = pretrainedPath(
      config.rvcDir,
      args.version,
      args.sample_rate,
      "D",
      args.pitch_guidance
    );

    try {
      process.stderr.write(
        `[rvc-mcp] Starting training: ${args.experiment_name} (${args.total_epochs} epochs, batch ${args.batch_size})\n`
      );

      const result = await gradioCallStreaming(
        "train_start_all",
        [
          args.experiment_name,
          args.sample_rate,
          args.pitch_guidance ? "Yes" : "No",
          args.training_folder,
          args.speaker_id,
          args.cpu_processes,
          args.f0_method,
          args.save_every_epoch,
          args.total_epochs,
          args.batch_size,
          args.save_only_latest ? "Yes" : "No",
          pretrainedG,
          pretrainedD,
          args.gpus,
          args.cache_to_gpu ? "Yes" : "No",
          args.save_every_weights ? "Yes" : "No",
          args.version,
          args.author,
        ],
        config,
        config.trainTimeout
      );

      return textResult({
        status: "success",
        experiment_name: args.experiment_name,
        total_epochs: args.total_epochs,
        batch_size: args.batch_size,
        version: args.version,
        log: result[0] ?? "Training complete",
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
