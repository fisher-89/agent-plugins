/**
 * session-registry.ts — persistent `session_id → change` binding for the
 * PostToolUse file recorder.
 *
 * The recorder observes the MCP `phase_next` call event (which carries both
 * `session_id` and `tool_input.change`) to build the binding, then attributes
 * subsequent tool events of the same session (including subagent events) to
 * that change.
 *
 * Storage lives under `os.tmpdir()` — never inside the user repository — and
 * is isolated per project root via a SHA-256 hash prefix. All failures are
 * swallowed (registry read → `null`, bind → silent) so the hook stays
 * fail-open: a lost registry only costs one turn of missed records because
 * every `phase_next` call refreshes the binding.
 */

import { createHash } from 'node:crypto';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { isPlainObject } from '../utils';

const REGISTRY_DIR_NAME = 'dev-team-hooks';

/** Entries older than this are dropped lazily on the next bind. */
const TTL_MS = 24 * 60 * 60 * 1000;

interface RegistryEntry {
  change: string;
  updatedAt: string;
}

type SessionRegistry = Record<string, RegistryEntry>;

/**
 * Registry file path for a project root:
 * `<tmpdir>/dev-team-hooks/<sha256(resolvedProjectRoot) first 16 hex>.json`.
 */
function registryPath(projectRoot: string): string {
  const hash = createHash('sha256').update(path.resolve(projectRoot)).digest('hex').slice(0, 16);
  return path.join(os.tmpdir(), REGISTRY_DIR_NAME, `${hash}.json`);
}

/**
 * Read the registry file. A missing, malformed or non-object file is treated
 * as an empty registry (fail-open); malformed entries are ignored.
 */
function readRegistry(filePath: string): SessionRegistry {
  try {
    const parsed: unknown = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
    if (!isPlainObject(parsed)) {
      return {};
    }
    const registry: SessionRegistry = {};
    for (const [sessionId, entry] of Object.entries(parsed)) {
      if (isPlainObject(entry) && typeof entry.change === 'string') {
        registry[sessionId] = { change: entry.change, updatedAt: String(entry.updatedAt) };
      }
    }
    return registry;
  } catch {
    return {};
  }
}

function writeRegistry(filePath: string, registry: SessionRegistry): void {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(registry)}\n`, 'utf-8');
}

/** True when an entry's `updatedAt` parses and is within the TTL. */
function isFresh(entry: RegistryEntry, now: number): boolean {
  const updated = Date.parse(entry.updatedAt);
  return !Number.isNaN(updated) && now - updated < TTL_MS;
}

/**
 * Establish or refresh the `session_id → change` binding, then lazily drop
 * entries whose `updatedAt` exceeds the 24h TTL. Never throws.
 */
export function bindSession(projectRoot: string, sessionId: string, change: string): void {
  try {
    const filePath = registryPath(projectRoot);
    const registry = readRegistry(filePath);
    const now = Date.now();

    const next: SessionRegistry = {};
    for (const [id, entry] of Object.entries(registry)) {
      if (isFresh(entry, now)) {
        next[id] = entry;
      }
    }
    next[sessionId] = { change, updatedAt: new Date(now).toISOString() };

    writeRegistry(filePath, next);
  } catch {
    // Fail-open: a failed bind only costs recorder attribution for this session.
  }
}

/**
 * Look up the change bound to a session. Returns `null` on miss (caller
 * silently drops the event) or on any read failure. Never throws.
 */
export function lookupChange(projectRoot: string, sessionId: string): string | null {
  try {
    const entry = readRegistry(registryPath(projectRoot))[sessionId];
    return entry ? entry.change : null;
  } catch {
    return null;
  }
}
