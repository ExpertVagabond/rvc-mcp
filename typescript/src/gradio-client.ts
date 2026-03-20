import type { RvcConfig } from "./config.js";

export class GradioError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "GradioError";
  }
}

/** Redact server internals from error messages returned to callers */
function redactError(detail: string): string {
  // Strip absolute paths, stack traces, and server config from messages
  return detail
    .replace(/\/[\w/.:-]+/g, "[path]")
    .replace(/\b\d{1,3}(\.\d{1,3}){3}(:\d+)?\b/g, "[host]")
    .replace(/at\s+.+\(.+\)/g, "")
    .replace(/\n\s*at\s+.+/g, "")
    .slice(0, 200);
}

/** Validate that POST data is an array of expected types */
function validatePostData(data: unknown[]): void {
  if (!Array.isArray(data)) {
    throw new GradioError("POST data must be an array");
  }
  for (let i = 0; i < data.length; i++) {
    const item = data[i];
    const t = typeof item;
    if (
      item !== null &&
      t !== "string" &&
      t !== "number" &&
      t !== "boolean" &&
      t !== "object"
    ) {
      throw new GradioError(
        `Invalid data type at index ${i}: expected string|number|boolean|object|null, got ${t}`
      );
    }
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
  validatePostData(data);
  const url = `${config.baseUrl}/gradio_api/call/${apiName}`;

  let postRes: Response;
  try {
    // Phase 1: POST to initiate
    postRes = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ data }),
      signal: AbortSignal.timeout(30_000),
    });
  } catch (e) {
    throw classifyFetchError(e, apiName);
  }

  if (!postRes.ok) {
    const body = await postRes.text().catch(() => "");
    process.stderr.write(
      `[rvc-mcp] Gradio POST /call/${apiName} failed: ${postRes.status} ${body}\n`
    );
    throw new GradioError(
      `Gradio call to ${apiName} failed (HTTP ${postRes.status})`
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
  validatePostData(data);
  const url = `${config.baseUrl}/gradio_api/call/${apiName}`;

  let postRes: Response;
  try {
    postRes = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ data }),
      signal: AbortSignal.timeout(30_000),
    });
  } catch (e) {
    throw classifyFetchError(e, apiName);
  }

  if (!postRes.ok) {
    const body = await postRes.text().catch(() => "");
    process.stderr.write(
      `[rvc-mcp] Gradio POST /call/${apiName} failed: ${postRes.status} ${body}\n`
    );
    throw new GradioError(
      `Gradio call to ${apiName} failed (HTTP ${postRes.status})`
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
    let sseRes: Response;
    try {
      sseRes = await fetch(sseUrl, { signal: controller.signal });
    } catch (e) {
      throw classifyFetchError(e, apiName);
    }

    if (!sseRes.ok || !sseRes.body) {
      throw new GradioError(
        `SSE stream for ${apiName} failed (HTTP ${sseRes.status})`
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
            // Log full error to stderr, return redacted message to caller
            process.stderr.write(
              `[rvc-mcp] Gradio error on ${apiName}: ${dataStr}\n`
            );
            throw new GradioError(
              `Gradio processing error on ${apiName}: ${redactError(dataStr)}`
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

/** Classify a fetch error into a typed GradioError with connection-specific messaging */
function classifyFetchError(e: unknown, apiName: string): GradioError {
  const msg = e instanceof Error ? e.message : String(e);
  const cause = (e as NodeJS.ErrnoException)?.cause as
    | { code?: string }
    | undefined;
  const code = cause?.code ?? "";

  // Log full details to stderr for debugging
  process.stderr.write(`[rvc-mcp] Fetch error on ${apiName}: ${msg}\n`);

  if (code === "ECONNREFUSED" || msg.includes("ECONNREFUSED")) {
    return new GradioError("Service unavailable: connection refused");
  }
  if (code === "ETIMEDOUT" || msg.includes("ETIMEDOUT")) {
    return new GradioError("Service unavailable: connection timed out");
  }
  if (code === "ENOTFOUND" || msg.includes("ENOTFOUND")) {
    return new GradioError("Service unavailable: host not found");
  }
  if (e instanceof TypeError && msg.includes("fetch failed")) {
    return new GradioError("Service unavailable: cannot reach RVC WebUI");
  }
  if (
    e instanceof Error &&
    (e.name === "AbortError" || msg.includes("aborted"))
  ) {
    return new GradioError("Service unavailable: request timed out");
  }
  return new GradioError("Service unavailable");
}

export function isConnectionError(e: unknown): boolean {
  if (e instanceof TypeError && String(e).includes("fetch failed")) return true;
  if (e instanceof Error && e.message.includes("ECONNREFUSED")) return true;
  if (e instanceof Error && e.message.includes("ETIMEDOUT")) return true;
  if (e instanceof Error && e.message.includes("ENOTFOUND")) return true;
  if (
    e instanceof Error &&
    e.message.startsWith("Service unavailable")
  )
    return true;
  if (
    e instanceof Error &&
    (e.name === "AbortError" || e.message.includes("aborted"))
  )
    return false;
  return false;
}
