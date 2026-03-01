import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCall, isConnectionError } from "../../gradio-client.js";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const schema = z.object({
  model_name: z
    .string()
    .describe("Voice model filename (e.g., 'matthew.pth')"),
  input_audio: z.string().describe("Absolute path to input audio file"),
  pitch: z
    .number()
    .int()
    .min(-24)
    .max(24)
    .default(0)
    .describe("Pitch shift in semitones (-24 to +24)"),
  f0_method: z
    .enum(["pm", "dio", "harvest", "crepe", "rmvpe", "fcpe"])
    .default("rmvpe")
    .describe("Pitch extraction method (rmvpe=best, pm=fast)"),
  index_path: z
    .string()
    .optional()
    .describe("Path to .index file for better quality"),
  index_rate: z
    .number()
    .min(0)
    .max(1)
    .default(0.75)
    .describe("Feature search ratio (0=no index, 1=full index)"),
  filter_radius: z
    .number()
    .int()
    .min(0)
    .max(7)
    .default(3)
    .describe("Median filter radius for pitch (reduces breathiness)"),
  resample_sr: z
    .number()
    .int()
    .default(0)
    .describe("Resample output to this sample rate (0=no resample)"),
  rms_mix_rate: z
    .number()
    .min(0)
    .max(1)
    .default(0.25)
    .describe("Volume envelope scaling (0=original, 1=fully remapped)"),
  protect: z
    .number()
    .min(0)
    .max(0.5)
    .default(0.33)
    .describe("Voiceless consonant protection (lower=more protection)"),
});

export const rvcInfer: McpAction = {
  tool: {
    name: "rvc_infer",
    description:
      "Convert vocals to a target voice using RVC via the WebUI. Supports all F0 methods, index files, and advanced parameters. Auto-loads the model.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const args = schema.parse(request.params.arguments);
    const config = loadConfig();

    if (!existsSync(args.input_audio)) {
      return errorResult(`Input file not found: ${args.input_audio}`);
    }

    const modelPath = args.model_name.startsWith("/")
      ? args.model_name
      : resolve(config.weightsDir, args.model_name);

    if (!existsSync(modelPath)) {
      return errorResult(
        `Voice model not found: ${modelPath}. Use rvc_list_models to see available models.`
      );
    }

    try {
      // Step 1: Load the model via infer_change_voice
      process.stderr.write(
        `[rvc-mcp] Loading model: ${args.model_name}\n`
      );
      await gradioCall(
        "infer_change_voice",
        [args.model_name, args.protect, args.protect],
        config,
        60_000
      );

      // Step 2: Run inference
      process.stderr.write(
        `[rvc-mcp] Running inference on: ${args.input_audio}\n`
      );
      const result = await gradioCall(
        "infer_convert",
        [
          0, // speaker ID
          { path: args.input_audio },
          args.pitch,
          null, // f0 curve file
          args.f0_method,
          null, // uploaded index file
          args.index_path ?? "",
          args.index_rate,
          args.filter_radius,
          args.resample_sr,
          args.rms_mix_rate,
          args.protect,
        ],
        config,
        config.inferTimeout
      );

      const info = result[0] as string;
      const audio = result[1] as { path?: string; url?: string } | null;

      return textResult({
        status: "success",
        model: args.model_name,
        input: args.input_audio,
        output: audio?.path ?? audio?.url ?? "unknown",
        info,
        f0_method: args.f0_method,
        pitch: args.pitch,
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
