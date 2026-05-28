import * as readline from "readline";

export interface JsonRpcRequest {
  jsonrpc: "2.0";
  id?: number | string;
  method: string;
  params?: Record<string, unknown>;
}

/**
 * Start listening on stdin for newline-delimited JSON-RPC requests.
 * Calls the handler for each parsed message until stdin closes.
 * Exits the process with code 0 when stdin ends.
 */
export function startServer(
  handler: (req: JsonRpcRequest) => Promise<void> | void,
): void {
  const rl = readline.createInterface({ input: process.stdin });

  rl.on("line", async (line) => {
    if (!line.trim()) return;
    let req: JsonRpcRequest;
    try {
      req = JSON.parse(line);
    } catch {
      sendError(0, -32700, "Parse error");
      return;
    }
    if (req.jsonrpc !== "2.0") {
      sendError(req.id ?? 0, -32600, "Invalid Request: jsonrpc must be 2.0");
      return;
    }
    // Notifications have no id — handle but don't respond
    if (req.id === undefined || req.id === null) {
      // Silently process notifications
      try { await handler(req); } catch { /* ignore */ }
      return;
    }
    try {
      await handler(req);
    } catch (e: any) {
      sendError(req.id, -32603, `Internal error: ${e.message}`);
    }
  });

  rl.on("close", () => {
    process.exit(0);
  });
}

export function sendResponse(id: number | string, result: unknown): void {
  const msg = { jsonrpc: "2.0", id, result };
  process.stdout.write(JSON.stringify(msg) + "\n");
}

export function sendError(
  id: number | string,
  code: number,
  message: string,
  data?: unknown,
): void {
  const msg = { jsonrpc: "2.0", id, error: { code, message, data } };
  process.stdout.write(JSON.stringify(msg) + "\n");
}

// JSON-RPC 2.0 standard error codes
export const ERROR_CODES = {
  PARSE_ERROR: -32700,
  INVALID_REQUEST: -32600,
  METHOD_NOT_FOUND: -32601,
  INVALID_PARAMS: -32602,
  INTERNAL_ERROR: -32603,
} as const;
