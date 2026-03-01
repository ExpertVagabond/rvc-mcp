import { z } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import type { McpAction, ToolInputSchema } from "../../types.js";
import { textResult } from "../../types.js";
import { loadConfig, notRunningMessage } from "../../config.js";
import { gradioHealthCheck } from "../../gradio-client.js";

const schema = z.object({});

export const rvcStatus: McpAction = {
  tool: {
    name: "rvc_status",
    description:
      "Check if the RVC WebUI is running. Returns status and start command if not running.",
    inputSchema: zodToJsonSchema(schema) as ToolInputSchema,
  },
  handler: async () => {
    const config = loadConfig();
    const running = await gradioHealthCheck(config);

    if (running) {
      return textResult({
        status: "running",
        url: config.baseUrl,
        rvc_dir: config.rvcDir,
      });
    }

    return textResult({
      status: "not_running",
      url: config.baseUrl,
      message: notRunningMessage(config.baseUrl),
    });
  },
};
