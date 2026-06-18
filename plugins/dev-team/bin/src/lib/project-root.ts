import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

import type { Server } from '@modelcontextprotocol/sdk/server/index.js';

/** Module-level MCP project root cache (process lifetime). */
let mcpProjectRootCache: string | null = null;

/** Read the cached MCP project root, or null if unset. */
export function getMcpCachedProjectRoot(): string | null {
  return mcpProjectRootCache;
}

/** Clear the MCP project root cache (test isolation only). */
export function resetMcpProjectRootCacheForTests(): void {
  mcpProjectRootCache = null;
}

/** Convert a `file://` URI to a platform-local absolute path. */
export function fileUriToPath(uri: string): string {
  if (!uri) {
    throw new TypeError('Invalid URL');
  }
  const parsed = new URL(uri);
  if (parsed.protocol !== 'file:') {
    throw new TypeError('Invalid URL');
  }

  try {
    return path.resolve(fileURLToPath(parsed));
  } catch {
    // Unix-style absolute paths (e.g. file:///home/user/...) on Windows
    return path.resolve(decodeURIComponent(parsed.pathname));
  }
}

function applyRootsList(roots: { uri: string }[]): void {
  if (roots.length === 0) {
    return;
  }
  mcpProjectRootCache = fileUriToPath(roots[0].uri);
}

/** Refresh project root cache after `roots/list_changed` notification. */
export async function refreshProjectRootFromMcp(server: Server): Promise<void> {
  try {
    const result = await server.listRoots();
    applyRootsList(result.roots);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`Failed to refresh MCP project root: ${message}\n`);
  }
}

/** Initialize project root from MCP `roots/list` after connect. */
export async function initProjectRootFromMcp(server: Server): Promise<void> {
  const rootsCapability = server.getClientCapabilities()?.roots;
  if (!rootsCapability) {
    return;
  }

  await refreshProjectRootFromMcp(server);
}
