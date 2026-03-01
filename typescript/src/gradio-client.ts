import type { RvcConfig } from "./config.js";

export class GradioError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "GradioError";
  }
}

export async function gradioHealthCheck(config: RvcConfig): Promise<boolean> {
  try {
    const res = await fetch(config.baseUrl, {
      signal: AbortSignal.timeout(5000),
    });
    return res.ok;
  } catch {
    return false;
  }
}

export async function gradioCall(
  apiName: string,
  data: unknown[],
  config: RvcConfig,
  timeoutMs: number = 60_000
): Promise<unknown[]> {
  const url = `${config.baseUrl}/call/${apiName}`;

  // Phase 1: POST to initiate
  const postRes = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ data }),
    signal: AbortSignal.timeout(30_000),
  });

  if (!postRes.ok) {
    const body = await postRes.text().catch(() => "");
    throw new GradioError(
      `POST /call/${apiName} failed: ${postRes.status} ${postRes.statusText} ${body}`
    );
  }

  const { event_id } = (await postRes.json()) as { event_id: string };

  // Phase 2: GET SSE stream
  return readSSE(url, event_id, apiName, timeoutMs);
}

export async function gradioCallStreaming(
  apiName: string,
  data: unknown[],
  config: RvcConfig,
  timeoutMs: number = 600_000
): Promise<unknown[]> {
  const url = `${config.baseUrl}/call/${apiName}`;

  const postRes = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ data }),
    signal: AbortSignal.timeout(30_000),
  });

  if (!postRes.ok) {
    const body = await postRes.text().catch(() => "");
    throw new GradioError(
      `POST /call/${apiName} failed: ${postRes.status} ${postRes.statusText} ${body}`
    );
  }

  const { event_id } = (await postRes.json()) as { event_id: string };

  return readSSE(url, event_id, apiName, timeoutMs);
}

async function readSSE(
  baseUrl: string,
  eventId: string,
  apiName: string,
  timeoutMs: number
): Promise<unknown[]> {
  const sseUrl = `${baseUrl}/${eventId}`;
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  try {
    const sseRes = await fetch(sseUrl, { signal: controller.signal });

    if (!sseRes.ok || !sseRes.body) {
      throw new GradioError(
        `SSE GET /call/${apiName}/${eventId} failed: ${sseRes.status}`
      );
    }

    const reader = sseRes.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    let lastData: unknown[] | null = null;

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";

      let currentEvent = "";
      for (const line of lines) {
        if (line.startsWith("event: ")) {
          currentEvent = line.slice(7).trim();
        } else if (line.startsWith("data: ")) {
          const dataStr = line.slice(6);
          if (currentEvent === "complete") {
            return JSON.parse(dataStr) as unknown[];
          } else if (currentEvent === "error") {
            throw new GradioError(
              `Gradio error on ${apiName}: ${dataStr}`
            );
          } else if (currentEvent === "generating") {
            lastData = JSON.parse(dataStr) as unknown[];
            process.stderr.write(
              `[rvc-mcp] ${apiName}: ${dataStr.slice(0, 300)}\n`
            );
          }
        }
      }
    }

    if (lastData) return lastData;
    throw new GradioError(
      `SSE stream ended without complete event for ${apiName}`
    );
  } finally {
    clearTimeout(timeout);
  }
}

export function isConnectionError(e: unknown): boolean {
  if (e instanceof TypeError && String(e).includes("fetch failed")) return true;
  if (e instanceof Error && e.message.includes("ECONNREFUSED")) return true;
  if (
    e instanceof Error &&
    (e.name === "AbortError" || e.message.includes("aborted"))
  )
    return false;
  return false;
}
