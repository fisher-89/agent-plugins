/**
 * record.ts — the PostToolUse recording pipeline: turn one hook event's raw
 * file ops into file_log entries. Imports the persistence primitives
 * directly from `./file-inventory` (not via the workflow barrel) so the
 * module graph stays free of a `record.ts ↔ index.ts` cycle.
 *
 * Pipeline order: normalize (project-root-relative POSIX, out-of-root
 * dropped) → self-pollution exclusion → gitignore filtering → append (log
 * keying and persistence live in `./file-inventory`). The gitignore filter —
 * see `./gitignore` — is built lazily at most once per call, only when a
 * candidate path survives normalization and exclusion, and filters write /
 * delete / revert ops alike; it never back-cleans paths already recorded.
 *
 * The recording scope — phase (with attempt) or workflow — is resolved by
 * the hook command layer's gate and passed in via `context.scope`; the
 * pipeline itself never inspects the running phase.
 */

import * as path from 'node:path';

import {
  type FileLogEntry,
  type FileOp,
  type RecordScope,
  appendLogEntries,
} from './file-inventory';
import { type GitignoreFilter, isGitIgnored, loadGitignoreFilter } from './gitignore';

/**
 * Convert an extracted path to a project-root-relative POSIX path.
 * Returns null when the path resolves outside the project root (event dropped).
 */
function normalizeRecordedPath(inputPath: string, projectRoot: string): string | null {
  if (!inputPath) return null;
  const abs = path.isAbsolute(inputPath)
    ? path.resolve(inputPath)
    : path.resolve(projectRoot, inputPath);
  const rel = path.relative(projectRoot, abs);
  if (!rel || rel.startsWith('..') || path.isAbsolute(rel)) return null;
  return rel.replace(/\\/g, '/');
}

/**
 * Self-pollution exclusion: `openspec/**` workflow artifacts (proposal /
 * design / reports / architecture models) and `workflow.json` itself never
 * enter the inventory.
 */
function isExcludedFromInventory(relPath: string): boolean {
  return (
    relPath === 'openspec' ||
    relPath.startsWith('openspec/') ||
    relPath === 'workflow.json' ||
    relPath.endsWith('/workflow.json')
  );
}

/**
 * Project one recording scope onto a file_log entry: phase scope records the
 * phase id + attempt, workflow scope records the reserved `'workflow'`
 * namespace (no attempt).
 */
function toLogEntry(
  op: FileOp['op'],
  target: string,
  scope: RecordScope,
  at: string,
): FileLogEntry {
  if (scope.kind === 'phase') {
    return { op, scope: scope.phase, attempt: scope.attempt, path: target, at };
  }
  return { op, scope: 'workflow', path: target, at };
}

/**
 * Attribute raw file ops (absolute / backslash paths welcome) to a change's
 * file log: surviving ops are stamped with the resolved recording scope
 * (phase + attempt, or workflow) and appended via the log's
 * same-key-overwrite / cross-key-append keying.
 *
 * A legacy change (no `file_log`) throws from the read step and propagates
 * to the caller (the command layer's catch-all turns it into a stderr
 * diagnostic and exit 0); the change must be recreated.
 */
export function recordFileOps(
  changeDir: string,
  ops: FileOp[],
  context: { projectRoot: string; scope: RecordScope },
): void {
  if (ops.length === 0) return;

  const onWarn = (message: string): void => {
    process.stderr.write(`record-files: ${message}\n`);
  };

  const at = new Date().toISOString();
  const recorded: FileLogEntry[] = [];
  let filter: GitignoreFilter | null = null;
  for (const op of ops) {
    const rel = normalizeRecordedPath(op.path, context.projectRoot);
    if (!rel || isExcludedFromInventory(rel)) continue;
    if (filter === null) filter = loadGitignoreFilter(context.projectRoot, onWarn);
    if (isGitIgnored(filter, rel)) continue;
    recorded.push(toLogEntry(op.op, rel, context.scope, at));
  }
  if (recorded.length === 0) return;

  appendLogEntries(changeDir, recorded);
}
