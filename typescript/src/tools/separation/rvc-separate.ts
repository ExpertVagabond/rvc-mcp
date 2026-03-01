import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCallStreaming, isConnectionError } from "../../gradio-client.js";
import { existsSync, mkdirSync, statSync } from "node:fs";
import { resolve } from "node:path";

const schema = z.object({
  input_audio: z
    .string()
    .describe("Path to audio file or folder containing audio files"),
  model: z
    .enum([
      "HP2",
      "HP3",
      "HP5",
      "MDX-Net",
      "DeEcho-Normal",
      "DeEcho-Aggressive",
      "DeEcho-DeReverb",
    ])
    .default("HP5")
    .describe(
      "UVR5 model: HP2/HP3=vocal preservation, HP5=main vocal only, MDX-Net/DeEcho=dereverb"
    ),
  output_vocals_dir: z
    .string()
    .optional()
    .describe("Output folder for vocals (default: ~/Desktop/AI-Music/uvr5/vocals)"),
  output_accompaniment_dir: z
    .string()
    .optional()
    .describe(
      "Output folder for accompaniment (default: ~/Desktop/AI-Music/uvr5/accompaniment)"
    ),
  format: z
    .enum(["wav", "flac", "mp3", "m4a"])
    .default("flac")
    .describe("Output audio format"),
  aggressiveness: z
    .number()
    .int()
    .min(0)
    .max(20)
    .default(10)
    .describe("Separation aggressiveness (0-20, higher=more aggressive)"),
});

export const rvcSeparateVocals: McpAction = {
  tool: {
    name: "rvc_separate_vocals",
    description:
      "Separate vocals from accompaniment using UVR5 (through RVC WebUI). Supports vocal preservation, main vocal isolation, and dereverb/deecho models.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async (request) => {
    const args = schema.parse(request.params.arguments);
    const config = loadConfig();

    if (!existsSync(args.input_audio)) {
      return errorResult(`Input not found: ${args.input_audio}`);
    }

    const vocalsDir =
      args.output_vocals_dir ??
      resolve(config.outputDir, "uvr5/vocals");
    const accompDir =
      args.output_accompaniment_dir ??
      resolve(config.outputDir, "uvr5/accompaniment");

    mkdirSync(vocalsDir, { recursive: true });
    mkdirSync(accompDir, { recursive: true });

    // Determine if input is a folder or single file
    const isFolder = statSync(args.input_audio).isDirectory();

    try {
      const result = await gradioCallStreaming(
        "uvr_convert",
        [
          args.model,
          isFolder ? args.input_audio : "",
          vocalsDir,
          isFolder ? null : [{ path: args.input_audio }],
          accompDir,
          args.aggressiveness,
          args.format,
        ],
        config,
        config.separateTimeout
      );

      return textResult({
        status: "success",
        model: args.model,
        input: args.input_audio,
        vocals_dir: vocalsDir,
        accompaniment_dir: accompDir,
        format: args.format,
        message: result[0] ?? "Separation complete",
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
