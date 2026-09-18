/**
 * file-inventory.ts — log-structured change file inventory
 * (`workflow.json.file_log`) shared library under `modules/workflow` (the
 * module boundary for `workflow.json` operation logic; the public surface
 * re-exports via `workflow/index.ts`).
 *
 * The inventory is an append-only log of recorded file operations; the net
 * state consumed by readers (`workflow_files`, `test-execution`,
 * `test-resolve-paths`, `c4-cross-ref`) is DERIVED from the log (same path,
 * later entry wins) and never persisted. Readers reach it through the
 * barrel's single read channel `getChangedFiles` (`files-query`); the
 * changeDir-level `readNetState` seam, the raw-log read and the pure
 * derivation below are module-internal.
 *
 * Each entry carries its recording scope — a phase id (with `attempt`) for
 * executor writes, `'workflow'` for everything else — so the log doubles as
 * the per-phase touch audit. Dedupe key for appends is
 * (scope namespace, attempt?, path): same key overwrites in place, cross-key
 * appends at the tail.
 *
 * Consumed by the PostToolUse recorder, `change_files`, `workflow_files`,
 * `test-execution`, `test-resolve-paths` and `c4-cross-ref`. Persistence
 * primitives come from `../doc-io` (module root, shared with
 * `phase/phase-state.ts`).
 */

import { fileLogSchema } from '../../../schemas';
import { getWorkflowJsonPath, loadWorkflowDoc, saveWorkflowDoc } from '../doc-io';

/** One file_log entry as persisted in `workflow.json.file_log`. */
export interface FileLogEntry {
  op: 'write' | 'delete' | 'revert';
  /** Recording context: a phase id, or `'workflow'` for non-executor writes. */
  scope: string;
  /** Attempt number; present only for phase-scoped entries. */
  attempt?: number;
  /** Project-root-relative POSIX path. */
  path: string;
  /** Record timestamp (ISO 8601). */
  at: string;
}

/** One observed file operation, as extracted by the hooks extractor. */
export interface FileOp {
  op: 'write' | 'delete' | 'revert';
  path: string;
}

/**
 * Recording context resolved by the recorder gate: phase-scoped when the
 * event's agent matches the running phase's executor, workflow-scoped
 * otherwise. Shared by the recorder and the manual `change_files` channel.
 */
export type RecordScope = { kind: 'phase'; phase: string; attempt: number } | { kind: 'workflow' };

/** Derived net state of the log (virtual view, never persisted). */
export type NetFileState = { written: string[]; deleted: string[] };

/**
 * Build the dedupe key for a log entry:
 * - phase scope → `(phase id, attempt, path)`;
 * - workflow scope → `('workflow', path)` (no attempt, per the log keying
 *   contract). `'workflow'` is not a valid phase id, so the namespaces
 *   cannot collide.
 */
function logKey(entry: FileLogEntry): string {
  if (entry.scope === 'workflow') {
    return `workflow\0${entry.path}`;
  }
  return `${entry.scope}\0${entry.attempt ?? ''}\0${entry.path}`;
}

/**
 * True when the validated document lacks the `file_log` field — a change
 * created before the log mechanism. The message carries the rebuild
 * guidance; callers never fall back to git.
 */
function isFileLogMissing(doc: Record<string, unknown>): boolean {
  return !Object.prototype.hasOwnProperty.call(doc, 'file_log');
}

function missingFileLogError(changeDir: string): Error {
  return new Error(
    `workflow.json 缺少 file_log 字段 (${getWorkflowJsonPath(changeDir)})：该 change 创建于文件清单机制之前，请重建该 change（change_create）。`,
  );
}

/**
 * Read the change file log from `<changeDir>/workflow.json` (module-internal
 * read primitive behind `readNetState`).
 *
 * Hard-errors — no git fallback — when the file does not exist, the JSON is
 * invalid, the root is not an object, the file fails `workflowFileSchema`,
 * or the `file_log` field is missing (a change created before the log
 * mechanism — the message carries the rebuild guidance). Entry shapes are
 * guaranteed by the schema validation inside `loadWorkflowDoc`.
 */
function readFileLog(changeDir: string): FileLogEntry[] {
  const doc = loadWorkflowDoc(changeDir);
  if (isFileLogMissing(doc)) {
    throw missingFileLogError(changeDir);
  }
  // Schema-parse instead of a cast: `loadWorkflowDoc` already validated the
  // document, so this never throws — it only narrows the raw value.
  return fileLogSchema.array().parse(doc.file_log);
}

/**
 * Read-modify-write the log with a single load/save round trip: validate +
 * load once, apply `update`, persist. `update` receives the current log
 * (safe to mutate in place) and returns the next log.
 */
function updateLog(
  changeDir: string,
  update: (log: FileLogEntry[]) => FileLogEntry[],
): FileLogEntry[] {
  const doc = loadWorkflowDoc(changeDir);
  if (isFileLogMissing(doc)) {
    throw missingFileLogError(changeDir);
  }
  const next = update(fileLogSchema.array().parse(doc.file_log));
  doc.file_log = next;
  saveWorkflowDoc(changeDir, doc);
  return next;
}

/**
 * Append log entries with (scope namespace, attempt?, path) keying: an entry
 * whose key already exists overwrites it in place (keeping the original
 * array position, last writer wins — including scope flips across
 * attempts); any other entry is appended at the tail. Returns the log as
 * persisted.
 */
export function appendLogEntries(changeDir: string, entries: FileLogEntry[]): FileLogEntry[] {
  if (entries.length === 0) {
    return readFileLog(changeDir);
  }
  return updateLog(changeDir, (log) => {
    const indexByKey = new Map<string, number>();
    log.forEach((entry, index) => indexByKey.set(logKey(entry), index));
    for (const entry of entries) {
      const existing = indexByKey.get(logKey(entry));
      if (existing !== undefined) {
        log[existing] = entry;
      } else {
        indexByKey.set(logKey(entry), log.length);
        log.push(entry);
      }
    }
    return log;
  });
}

/**
 * Derive the net state from the log (pure function). Symmetric rules —
 * replaying only overstates, never underrecords dangerous operations:
 * ```
 * write(P)   →  deleted -= P ;  written += P
 * delete(P)  →  written -= P ;  deleted  += P
 * revert(P)  →  written -= P ;  deleted -= P
 * ```
 * Equivalent to replaying the log through the former `foldFileOps` rules:
 * a path's net op is decided by its latest entry.
 */
function deriveNetState(log: FileLogEntry[]): NetFileState {
  const written = new Set<string>();
  const deleted = new Set<string>();

  for (const entry of log) {
    switch (entry.op) {
      case 'write':
        deleted.delete(entry.path);
        written.add(entry.path);
        break;
      case 'delete':
        written.delete(entry.path);
        deleted.add(entry.path);
        break;
      case 'revert':
        written.delete(entry.path);
        deleted.delete(entry.path);
        break;
    }
  }

  return { written: Array.from(written), deleted: Array.from(deleted) };
}

/**
 * The changeDir-level combined read (module-internal seam behind
 * `getChangedFiles`, imported only by `files-query` — not part of the barrel
 * surface): read the change file log and derive its net state in one step.
 * Hard-error semantics are the raw read's — missing / invalid
 * `workflow.json` or a missing `file_log` field throws with the rebuild
 * guidance; no git fallback.
 */
export function readNetState(changeDir: string): NetFileState {
  return deriveNetState(readFileLog(changeDir));
}

/** Dedupe a path list preserving first-seen order. */
function dedupe(paths: string[]): string[] {
  return Array.from(new Set(paths));
}

/** Build one workflow-scoped log entry. */
function workflowEntry(op: FileLogEntry['op'], path: string, at: string): FileLogEntry {
  const parsed = fileLogSchema.parse({ op, scope: 'workflow', path, at });
  return parsed as FileLogEntry;
}

/**
 * append semantics (read-modify-write): append one workflow-scoped record
 * per path (`written` → write, `deleted` → delete). Within the workflow
 * namespace the path is the dedupe key, so a path already recorded at
 * workflow scope is upserted in place; phase-scoped audit records are never
 * touched. A path present in both buckets resolves to delete (appended
 * later, same key). This file never applies gitignore filtering:
 * `change_files` is the manual backfill / correction channel.
 */
export function appendWorkflowFiles(
  changeDir: string,
  paths: { written?: string[]; deleted?: string[] },
): NetFileState {
  const at = new Date().toISOString();
  const entries = [
    ...dedupe(paths.written ?? []).map((p) => workflowEntry('write', p, at)),
    ...dedupe(paths.deleted ?? []).map((p) => workflowEntry('delete', p, at)),
  ];
  const log = appendLogEntries(changeDir, entries);
  return deriveNetState(log);
}

/**
 * set semantics (read-modify-write): every log record touching a provided
 * path (any scope, attempt or op) is removed from the log, then
 * workflow-scoped records are appended at the tail in bucket order
 * (`written` first, then `deleted` — a path in both buckets resolves to
 * delete). Records for untouched paths (including phase audit entries) are
 * preserved verbatim. No gitignore filtering, same reason as
 * `appendWorkflowFiles`.
 */
export function setWorkflowFiles(
  changeDir: string,
  paths: { written?: string[]; deleted?: string[] },
): NetFileState {
  const provided = new Set([...(paths.written ?? []), ...(paths.deleted ?? [])]);
  const at = new Date().toISOString();

  const log = updateLog(changeDir, (current) => {
    const kept = current.filter((entry) => !provided.has(entry.path));
    for (const p of dedupe(paths.written ?? [])) {
      kept.push(workflowEntry('write', p, at));
    }
    for (const p of dedupe(paths.deleted ?? [])) {
      kept.push(workflowEntry('delete', p, at));
    }
    return kept;
  });
  return deriveNetState(log);
}
