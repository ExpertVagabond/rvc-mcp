import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult, errorResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioCall, isConnectionError } from "../../gradio-client.js";

const schema = z.object({});

interface GradioUpdate {
  choices?: string[];
  __type__?: string;
}

export const rvcListModels: McpAction = {
  tool: {
    name: "rvc_list_models",
    description:
      "List available RVC voice models (.pth) and index files (.index) from the WebUI.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async () => {
    const config = loadConfig();

    try {
      const result = await gradioCall("infer_refresh", [], config, 30_000);

      const models: string[] = [];
      const indices: string[] = [];

      for (const item of result) {
        const update = item as GradioUpdate;
        if (update?.choices) {
          if (models.length === 0) {
            models.push(...update.choices);
          } else {
            indices.push(...update.choices);
          }
        }
      }

      return textResult({
        models,
        indices,
        model_count: models.length,
        index_count: indices.length,
      });
    } catch (e) {
      if (isConnectionError(e)) {
        return errorResult(notRunningMessage(config.baseUrl));
      }
      return errorResult(e instanceof Error ? e.message : String(e));
    }
  },
};
