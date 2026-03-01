import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCall, isConnectionError } from "../../gradio-client.js";

const schema = z.object({});

export const rvcClean: McpAction = {
  tool: {
    name: "rvc_clean",
    description:
      "Unload voice models from memory to free up RAM/VRAM. Call this after inference if memory is tight.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async () => {
    const config = loadConfig();

    try {
      const result = await gradioCall("infer_clean", [], config, 30_000);
      return textResult({
        status: "success",
        message: "Models unloaded from memory",
        raw: result,
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
