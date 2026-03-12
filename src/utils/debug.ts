const DEBUG_ENABLED =
  process.env.DEBUG_DOCMOST_MCP === "1" || process.env.DEBUG_DOCMOST_MCP === "true";

export function debugLog(scope: string, message: string, details?: unknown): void {
  if (!DEBUG_ENABLED) {
    return;
  }

  const timestamp = new Date().toISOString();
  const prefix = `[docmost-local-mcp][${timestamp}][${scope}]`;

  if (details === undefined) {
    process.stderr.write(`${prefix} ${message}\n`);
    return;
  }

  process.stderr.write(`${prefix} ${message} ${safeStringify(details)}\n`);
}

function safeStringify(value: unknown): string {
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
