import { resolve } from "node:path";
import { homedir } from "node:os";

export interface RvcConfig {
  baseUrl: string;
  rvcDir: string;
  outputDir: string;
  weightsDir: string;
  logsDir: string;
  inferTimeout: number;
  separateTimeout: number;
  preprocessTimeout: number;
  extractTimeout: number;
  trainTimeout: number;
  modelOpTimeout: number;
}

function env(key: string, fallback: string): string {
  return process.env[key] || fallback;
}

export function loadConfig(): RvcConfig {
  const baseUrl = env("RVC_URL", "http://localhost:7865");
  const rvcDir = env("RVC_DIR", "/Volumes/Virtual Server/projects/ai-music-rvc");
  const outputDir = env("RVC_OUTPUT_DIR", resolve(homedir(), "Desktop/AI-Music"));

  return {
    baseUrl,
    rvcDir,
    outputDir,
    weightsDir: resolve(rvcDir, "assets/weights"),
    logsDir: resolve(rvcDir, "logs"),
    inferTimeout: 300_000,
    separateTimeout: 600_000,
    preprocessTimeout: 600_000,
    extractTimeout: 1_800_000,
    trainTimeout: 3_600_000,
    modelOpTimeout: 120_000,
  };
}

export const START_COMMAND = `cd "/Volumes/Virtual Server/projects/ai-music-rvc" && source .venv/bin/activate && python web.py --pycmd python --noautoopen`;

export function notRunningMessage(baseUrl: string): string {
  return `RVC WebUI is not running at ${baseUrl}.\n\nStart it with:\n  ${START_COMMAND}`;
}
