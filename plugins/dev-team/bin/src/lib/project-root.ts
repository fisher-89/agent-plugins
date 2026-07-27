import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp';

/** Narrow interface covering only the Server methods used by project-root. */
export type McpServerLike = Pick<McpServer['server'], 'getClientCapabilities' | 'listRoots'>;

export type ProjectRootLockErrorCode = 'not_locked' | 'multi_root' | 'literal_env' | 'invalid_path';

/** Thrown when MCP tools require a locked project root that is unavailable. */
export class ProjectRootLockError extends Error {
  readonly code: ProjectRootLockErrorCode;

  constructor(code: ProjectRootLockErrorCode, message: string) {
    super(message);
    this.name = 'ProjectRootLockError';
    this.code = code;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/** Duck-typed guard so handlers still catch lock errors across module copies. */
export function isProjectRootLockError(err: unknown): err is ProjectRootLockError {
  if (err instanceof ProjectRootLockError) {
    return true;
  }
  if (!(err instanceof Error) || err.name !== 'ProjectRootLockError') {
    return false;
  }
  return typeof Reflect.get(err, 'code') === 'string';
}

/** Max wait for MCP roots/list before treating the channel as failed. */
const LIST_ROOTS_TIMEOUT_MS = 2000;

type RootsListResult = { roots: { uri: string }[] };

/** Call listRoots with a timeout so a hung client cannot block connect forever. */
async function listRootsWithTimeout(server: McpServerLike): Promise<RootsListResult> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      server.listRoots(),
      new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          reject(new Error(`roots/list timed out after ${LIST_ROOTS_TIMEOUT_MS}ms`));
        }, LIST_ROOTS_TIMEOUT_MS);
      }),
    ]);
  } finally {
    if (timeoutId !== undefined) {
      clearTimeout(timeoutId);
    }
  }
}

/** Module-level MCP project root cache (process lifetime; immutable once set). */
let mcpProjectRootCache: string | null = null;

/** Last failure reason recorded during init (for requireLockedProjectRoot errors). */
let lockFailureReason: string | null = null;

const LITERAL_ENV_PATTERN = /\$\{[^}]+\}/;

/** Read the cached MCP project root, or null if unset. */
export function getMcpCachedProjectRoot(): string | null {
  return mcpProjectRootCache;
}

/** Convert a `file://` URI to a platform-local absolute path. */
function fileUriToPath(uri: string): string {
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

/** True when path is absolute, exists on disk, and has no unexpanded `${...}`. */
function isUsableAbsolutePath(candidate: string | undefined | null): candidate is string {
  if (!candidate) {
    return false;
  }
  if (LITERAL_ENV_PATTERN.test(candidate)) {
    return false;
  }
  if (!path.isAbsolute(candidate)) {
    return false;
  }
  return fs.existsSync(candidate);
}

/**
 * Parse WORKSPACE_FOLDER_PATHS (`,` or `;`). Returns the path only when exactly
 * one usable absolute path is present; otherwise null (never guesses `[0]`).
 */
function resolveUniqueWorkspaceFolderPath(): string | null {
  const raw = process.env.WORKSPACE_FOLDER_PATHS;
  if (!raw) {
    return null;
  }
  const segments = raw
    .split(/[,;]/)
    .map((s) => s.trim())
    .filter(Boolean);
  const usable = segments.filter(isUsableAbsolutePath);
  if (usable.length === 1) {
    return usable[0];
  }
  return null;
}

/** Convert a roots URI (file:// or bare absolute path) to a local path, or null. */
function rootEntryToPath(uri: string): string | null {
  if (!uri) {
    return null;
  }
  try {
    if (uri.startsWith('file:')) {
      return fileUriToPath(uri);
    }
  } catch {
    return null;
  }
  if (path.isAbsolute(uri)) {
    return uri;
  }
  return null;
}

/** Generic "channel unavailable" reasons that must not erase a more specific prior failure. */
function isGenericLockFailure(reason: string): boolean {
  return /client does not declare roots capability|WORKSPACE_FOLDER_PATHS is unset|roots\/list returned no usable file roots|roots\/list failed:/i.test(
    reason,
  );
}

function setLockFailure(reason: string): void {
  // Keep CLAUDE/WORKSPACE/roots specific failures (literal / invalid / multi) for requireLocked mapping.
  if (
    lockFailureReason &&
    isGenericLockFailure(reason) &&
    !isGenericLockFailure(lockFailureReason)
  ) {
    return;
  }
  lockFailureReason = reason;
}

/**
 * Return the locked MCP project root, or throw ProjectRootLockError.
 * MCP tool handlers MUST call this before any project-tree I/O.
 */
export function requireLockedProjectRoot(): string {
  if (mcpProjectRootCache) {
    return mcpProjectRootCache;
  }
  const reason = lockFailureReason ?? 'no unique usable project root found';
  let code: ProjectRootLockErrorCode = 'not_locked';
  if (/multi|more than one|>\s*1/i.test(reason)) {
    code = 'multi_root';
  } else if (/\$\{|literal/i.test(reason)) {
    code = 'literal_env';
  } else if (/invalid|not\s+(an\s+)?absolute|does not exist/i.test(reason)) {
    code = 'invalid_path';
  }
  throw new ProjectRootLockError(
    code,
    `Project root is not locked: ${reason}. ` +
      'Set CLAUDE_PROJECT_DIR to an existing absolute path, ' +
      'or provide exactly one MCP root / WORKSPACE_FOLDER_PATHS entry.',
  );
}

/**
 * CLI/command helper: prefer locked cache, then usable CLAUDE_PROJECT_DIR,
 * then exactly one usable WORKSPACE_FOLDER_PATHS, else process.cwd().
 * MCP handlers MUST NOT rely on the cwd fallback — use requireLockedProjectRoot().
 */
export function getProjectDir(): string {
  if (mcpProjectRootCache) {
    return mcpProjectRootCache;
  }
  if (isUsableAbsolutePath(process.env.CLAUDE_PROJECT_DIR)) {
    return process.env.CLAUDE_PROJECT_DIR;
  }
  const workspace = resolveUniqueWorkspaceFolderPath();
  if (workspace) {
    return workspace;
  }
  return process.cwd();
}

/** Try CLAUDE_PROJECT_DIR channel; returns path or null. */
function tryClaudeProjectDir(): string | null {
  const value = process.env.CLAUDE_PROJECT_DIR;
  if (!value) {
    return null;
  }
  if (LITERAL_ENV_PATTERN.test(value)) {
    setLockFailure(`CLAUDE_PROJECT_DIR contains unexpanded literal: ${value}`);
    return null;
  }
  if (!path.isAbsolute(value)) {
    setLockFailure(`CLAUDE_PROJECT_DIR is not an absolute path: ${value}`);
    return null;
  }
  if (!fs.existsSync(value)) {
    setLockFailure(`CLAUDE_PROJECT_DIR does not exist: ${value}`);
    return null;
  }
  return value;
}

/**
 * Try MCP roots/list channel. Returns path when exactly one usable root;
 * otherwise null (0 / >1 / error / -32601).
 */
async function tryMcpRoots(server: McpServerLike): Promise<string | null> {
  const rootsCapability = server.getClientCapabilities()?.roots;
  if (!rootsCapability) {
    setLockFailure('client does not declare roots capability');
    return null;
  }

  let result: RootsListResult;
  try {
    result = await listRootsWithTimeout(server);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`MCP roots/list failed: ${message}\n`);
    setLockFailure(`roots/list failed: ${message}`);
    return null;
  }

  const usable: string[] = [];
  for (const root of result.roots) {
    const local = rootEntryToPath(root.uri);
    if (local && isUsableAbsolutePath(local)) {
      usable.push(local);
    }
  }

  if (usable.length === 1) {
    return usable[0];
  }
  if (usable.length === 0) {
    process.stderr.write('MCP roots/list returned no usable file roots\n');
    setLockFailure('roots/list returned no usable file roots');
    return null;
  }
  process.stderr.write(
    `MCP roots/list returned ${usable.length} usable roots; refusing to guess\n`,
  );
  setLockFailure(`roots/list returned ${usable.length} usable roots (multi-root ambiguity)`);
  return null;
}

/** Try WORKSPACE_FOLDER_PATHS channel (exactly one usable path). */
function tryWorkspaceFolderPaths(): string | null {
  const raw = process.env.WORKSPACE_FOLDER_PATHS;
  if (!raw) {
    setLockFailure('WORKSPACE_FOLDER_PATHS is unset');
    return null;
  }
  const segments = raw
    .split(/[,;]/)
    .map((s) => s.trim())
    .filter(Boolean);
  const usable = segments.filter(isUsableAbsolutePath);
  if (usable.length === 1) {
    return usable[0];
  }
  if (usable.length === 0) {
    setLockFailure(
      segments.some((s) => LITERAL_ENV_PATTERN.test(s))
        ? `WORKSPACE_FOLDER_PATHS contains unexpanded literal: ${raw}`
        : `WORKSPACE_FOLDER_PATHS has no usable absolute paths: ${raw}`,
    );
    return null;
  }
  setLockFailure(`WORKSPACE_FOLDER_PATHS has ${usable.length} usable paths (multi-root ambiguity)`);
  return null;
}

/**
 * Initialize project root lock after connect.
 * Priority: CLAUDE_PROJECT_DIR → roots/list (exactly 1) → WORKSPACE_FOLDER_PATHS (exactly 1).
 * Already-locked cache is immutable. Failure leaves cache null (no cwd fallback, no exit).
 * Does NOT register list_changed handlers.
 */
export async function initProjectRootFromMcp(server: McpServerLike): Promise<void> {
  if (mcpProjectRootCache) {
    return;
  }

  const fromEnv = tryClaudeProjectDir();
  if (fromEnv) {
    mcpProjectRootCache = fromEnv;
    lockFailureReason = null;
    return;
  }

  const fromRoots = await tryMcpRoots(server);
  if (fromRoots) {
    mcpProjectRootCache = fromRoots;
    lockFailureReason = null;
    return;
  }

  const fromWorkspace = tryWorkspaceFolderPaths();
  if (fromWorkspace) {
    mcpProjectRootCache = fromWorkspace;
    lockFailureReason = null;
    return;
  }

  // Keep cache null; lockFailureReason already set by the last failing channel
  // (or earlier channels). Prefer a summary if somehow still unset.
  if (!lockFailureReason) {
    setLockFailure('no unique usable project root from env, roots, or WORKSPACE_FOLDER_PATHS');
  }
}
