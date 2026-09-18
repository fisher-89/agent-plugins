import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

import type { McpServer } from '@modelcontextprotocol/sdk/server/mcp';

import { isPlainObject } from '../utils';

/** Narrow interface covering only the Server methods used by project-root. */
export type McpServerLike = Pick<McpServer['server'], 'getClientCapabilities' | 'listRoots'>;

type ProjectRootResolveErrorCode = 'invalid_path' | 'not_in_candidates';

const LITERAL_ENV_PATTERN = /\$\{[^}]+\}/;

/** Timeout for MCP `roots/list` collection (ms). */
const ROOTS_LIST_TIMEOUT_MS = 2000;

const FORCE_HINT =
  'Resubmit the same tool call with identical complete arguments to force-add this `project_root` into candidates and proceed.';

/** Thrown when MCP tool `project_root` fails validity / candidate / force checks. */
class ProjectRootResolveError extends Error {
  readonly code: ProjectRootResolveErrorCode;
  readonly candidates?: readonly string[];
  readonly force_hint?: string;
  readonly project_root?: string;

  constructor(
    code: ProjectRootResolveErrorCode,
    message: string,
    extras?: {
      candidates?: readonly string[];
      force_hint?: string;
      project_root?: string;
    },
  ) {
    super(message);
    this.name = 'ProjectRootResolveError';
    this.code = code;
    this.candidates = extras?.candidates;
    this.force_hint = extras?.force_hint;
    this.project_root = extras?.project_root;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/** Duck-typed guard so handlers still catch resolve errors across module copies. */
function isProjectRootResolveError(err: unknown): err is ProjectRootResolveError {
  if (err instanceof ProjectRootResolveError) {
    return true;
  }
  if (!(err instanceof Error) || err.name !== 'ProjectRootResolveError') {
    return false;
  }
  return typeof Reflect.get(err, 'code') === 'string';
}

export type ProjectRootResolveErrorResult = {
  isError: true;
  content: { type: 'text'; text: string }[];
};

function resolveErrorResult(err: {
  code: string;
  message: string;
  project_root?: string;
  candidates?: readonly string[];
  force_hint?: string;
}): ProjectRootResolveErrorResult {
  return {
    isError: true,
    content: [
      {
        type: 'text',
        text: JSON.stringify({
          code: err.code,
          message: err.message,
          project_root: err.project_root,
          candidates: err.candidates,
          force_hint: err.force_hint,
        }),
      },
    ],
  };
}

/**
 * Resolve required `project_root`, inject call-scoped root for `run`, map resolve
 * errors to MCP `isError` JSON. Non-resolve errors rethrow.
 */
export async function withResolvedProjectRoot<T>(
  toolName: string,
  args: Record<string, unknown>,
  run: (projectRoot: string) => T | Promise<T>,
): Promise<T | ProjectRootResolveErrorResult> {
  try {
    const projectRoot = resolveProjectRootForTool(toolName, args);
    return await runWithCallScopedRoot(projectRoot, () => run(projectRoot));
  } catch (err) {
    // Prefer early rethrow so non-resolve errors never enter resolveErrorResult.
    if (!isProjectRootResolveError(err)) {
      throw err;
    }
    return resolveErrorResult(err);
  }
}

type RootsListResult = { roots: { uri: string }[] };

/**
 * Candidate set: comparison-key → first-seen display path (normalizeRootPath form).
 * Process memory only; force may append; restart clears.
 */
const candidatesByKey = new Map<string, string>();

/** Most recent pending force key (`toolName\\n` + stableStringify(args)), or null. */
let pendingKey: string | null = null;

/** Call-scoped root injected for the duration of a tool handler (phase_* → resolveChangeDir). */
let callScopedProjectRoot: string | null = null;

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
 * Normalize a root path for membership / storage:
 * resolve (fold `.`/`..`), strip trailing separators (keep drive/Unix root), no realpath.
 */
function normalizeRootPath(p: string): string {
  let resolved = path.resolve(p);
  if (process.platform === 'win32') {
    // Keep "C:\" (and UNC roots); strip other trailing separators.
    if (/^[A-Za-z]:[\\/]$/.test(resolved)) {
      return resolved.endsWith('\\') ? resolved : `${resolved}\\`;
    }
    resolved = resolved.replace(/[\\/]+$/, '');
    return resolved;
  }
  if (resolved === '/') {
    return resolved;
  }
  return resolved.replace(/\/+$/, '') || '/';
}

/** Platform comparison key for candidate membership (Windows: case-insensitive). */
function comparisonKey(normalized: string): string {
  if (process.platform === 'win32') {
    return normalized.toLowerCase();
  }
  return normalized;
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

/** Add a usable path to candidates (dedupe by comparison key; keep first display form). */
function addCandidate(raw: string): void {
  if (!isUsableAbsolutePath(raw)) {
    return;
  }
  const normalized = normalizeRootPath(raw);
  const key = comparisonKey(normalized);
  if (!candidatesByKey.has(key)) {
    candidatesByKey.set(key, normalized);
  }
}

/**
 * Parse WORKSPACE_FOLDER_PATHS (`,` or `;`). Returns the path only when exactly
 * one usable absolute path is present; otherwise null (CLI getProjectDir only).
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

/** Recursively sort object keys for stable JSON serialization. */
function sortKeysDeep(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map(sortKeysDeep);
  }
  if (isPlainObject(value)) {
    const sorted: Record<string, unknown> = {};
    for (const k of Object.keys(value).sort()) {
      sorted[k] = sortKeysDeep(value[k]);
    }
    return sorted;
  }
  return value;
}

function stableStringify(value: unknown): string {
  return JSON.stringify(sortKeysDeep(value));
}

function makePendingKey(toolName: string, args: Record<string, unknown>): string {
  return `${toolName}\n${stableStringify(args)}`;
}

function invalidPathMessage(raw: string): string {
  if (LITERAL_ENV_PATTERN.test(raw)) {
    return `project_root contains unexpanded literal: ${raw}`;
  }
  if (!path.isAbsolute(raw)) {
    return `project_root is not an absolute path: ${raw}`;
  }
  if (!fs.existsSync(raw)) {
    return `project_root does not exist: ${raw}`;
  }
  return `project_root is not a usable absolute path: ${raw}`;
}

/**
 * Resolve `args.project_root` for an MCP tool call:
 * validity → ∈ candidates → pending force → else write pending and throw.
 * MUST NOT fall back to process.cwd().
 */
function resolveProjectRootForTool(toolName: string, args: Record<string, unknown>): string {
  const raw = args.project_root;
  if (typeof raw !== 'string') {
    const display =
      typeof raw === 'number' || typeof raw === 'boolean' || typeof raw === 'bigint'
        ? String(raw)
        : '';
    throw new ProjectRootResolveError('invalid_path', 'project_root must be a non-empty string', {
      project_root: display,
      candidates: getProjectRootCandidates(),
    });
  }

  if (!isUsableAbsolutePath(raw)) {
    throw new ProjectRootResolveError('invalid_path', invalidPathMessage(raw), {
      project_root: raw,
      candidates: getProjectRootCandidates(),
    });
  }

  const normalized = normalizeRootPath(raw);
  const key = comparisonKey(normalized);

  const existing = candidatesByKey.get(key);
  if (existing !== undefined) {
    pendingKey = null;
    return existing;
  }

  const callKey = makePendingKey(toolName, args);
  if (pendingKey !== null && pendingKey === callKey) {
    candidatesByKey.set(key, normalized);
    pendingKey = null;
    return normalized;
  }

  pendingKey = callKey;
  throw new ProjectRootResolveError(
    'not_in_candidates',
    `project_root is not in collected candidates: ${normalized}`,
    {
      project_root: raw,
      candidates: getProjectRootCandidates(),
      force_hint: FORCE_HINT,
    },
  );
}

/**
 * Run `fn` with `getProjectDir()` preferring `projectRoot`.
 * Clears the call-scoped root in `finally` (including after async settlement).
 */
function runWithCallScopedRoot<T>(projectRoot: string, fn: () => T | Promise<T>): T | Promise<T> {
  const previous = callScopedProjectRoot;
  callScopedProjectRoot = projectRoot;
  try {
    const result = fn();
    if (result instanceof Promise) {
      return result.finally(() => {
        callScopedProjectRoot = previous;
      });
    }
    callScopedProjectRoot = previous;
    return result;
  } catch (err) {
    callScopedProjectRoot = previous;
    throw err;
  }
}

/** Read-only snapshot of current candidates (display paths). */
export function getProjectRootCandidates(): readonly string[] {
  return Array.from(candidatesByKey.values());
}

/**
 * CLI/command helper: call-scoped → usable CLAUDE_PROJECT_DIR →
 * exactly one usable WORKSPACE_FOLDER_PATHS → process.cwd().
 * MCP handlers MUST resolve via withResolvedProjectRoot first (no cwd fallback there).
 */
export function getProjectDir(): string {
  if (callScopedProjectRoot) {
    return callScopedProjectRoot;
  }
  const claude = process.env.CLAUDE_PROJECT_DIR;
  if (isUsableAbsolutePath(claude)) {
    return normalizeRootPath(claude);
  }
  const workspace = resolveUniqueWorkspaceFolderPath();
  if (workspace) {
    return normalizeRootPath(workspace);
  }
  return process.cwd();
}

async function listRootsWithTimeout(server: McpServerLike): Promise<RootsListResult> {
  let timeoutId: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      server.listRoots().finally(() => {
        if (timeoutId !== undefined) {
          clearTimeout(timeoutId);
          timeoutId = undefined;
        }
      }),
      new Promise<never>((_, reject) => {
        timeoutId = setTimeout(() => {
          reject(new Error(`roots/list timed out after ${ROOTS_LIST_TIMEOUT_MS}ms`));
        }, ROOTS_LIST_TIMEOUT_MS);
      }),
    ]);
  } finally {
    if (timeoutId !== undefined) {
      clearTimeout(timeoutId);
    }
  }
}

/** Collect every usable path from WORKSPACE_FOLDER_PATHS into candidates. */
function collectWorkspaceFolderPaths(): void {
  const raw = process.env.WORKSPACE_FOLDER_PATHS;
  if (!raw) {
    return;
  }
  const segments = raw
    .split(/[,;]/)
    .map((s) => s.trim())
    .filter(Boolean);
  for (const segment of segments) {
    addCandidate(segment);
  }
}

/** Collect usable roots from MCP roots/list (capability + timeout). Failures do not throw. */
async function collectMcpRoots(server: McpServerLike): Promise<void> {
  const caps = server.getClientCapabilities?.();
  if (!caps || !('roots' in caps) || caps.roots == null) {
    return;
  }

  let result: RootsListResult;
  try {
    result = await listRootsWithTimeout(server);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    process.stderr.write(`MCP roots/list failed: ${message}\n`);
    return;
  }

  let usableCount = 0;
  for (const root of result.roots) {
    const local = rootEntryToPath(root.uri);
    if (local && isUsableAbsolutePath(local)) {
      addCandidate(local);
      usableCount += 1;
    }
  }

  if (usableCount === 0) {
    process.stderr.write('MCP roots/list returned no usable file roots\n');
  }
}

/**
 * After connect: merge CLAUDE ∪ WORKSPACE ∪ roots into candidates.
 * Does not lock a default root, does not exit, does not register list_changed, no cwd.
 */
export async function collectProjectRootCandidates(server: McpServerLike): Promise<void> {
  const claude = process.env.CLAUDE_PROJECT_DIR;
  if (claude) {
    addCandidate(claude);
  }

  collectWorkspaceFolderPaths();
  await collectMcpRoots(server);
}
