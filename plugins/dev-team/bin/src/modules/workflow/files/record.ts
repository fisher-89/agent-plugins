/**
 * record.ts — the PostToolUse recording pipeline: turn one hook event's raw
 * file ops into inventory net state. Imports the persistence primitives
 * directly from `./file-inventory` (not via the workflow barrel) so the
 * module graph stays free of a `record.ts ↔ index.ts` cycle.
 *
 * Pipeline order: normalize (project-root-relative POSIX, out-of-root
 * dropped) → self-pollution exclusion → gitignore filtering → read → fold →
 * write. The gitignore filter — see `./gitignore` — is built lazily at most
 * once per call, only when a candidate path survives normalization and
 * exclusion, and filters write / delete / revert ops alike; it never
 * back-cleans paths already sitting in the net state.
 */

import * as path from 'node:path';

import { type FileOp, foldFileOps, readFileInventory, writeFileInventory } from './file-inventory';
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
 * Attribute raw file ops (absolute / backslash paths welcome) to a change's
 * file inventory: surviving ops are stamped with the subagent `agent_type` —
 * the event-level `context.agentType` applies to ops carrying none — then
 * folded into the net state and written back.
 *
 * A legacy change (no `files`) throws from the read step and propagates to
 * the caller (the command layer's catch-all turns it into a stderr
 * diagnostic and exit 0); the change must be recreated.
 */
export function recordFileOps(
  changeDir: string,
  ops: FileOp[],
  context: { projectRoot: string; agentType?: string },
): void {
  if (ops.length === 0) return;

  const onWarn = (message: string): void => {
    process.stderr.write(`record-files: ${message}\n`);
  };

  const recorded: FileOp[] = [];
  let filter: GitignoreFilter | null = null;
  for (const op of ops) {
    const rel = normalizeRecordedPath(op.path, context.projectRoot);
    if (!rel || isExcludedFromInventory(rel)) continue;
    if (filter === null) filter = loadGitignoreFilter(context.projectRoot, onWarn);
    if (isGitIgnored(filter, rel)) continue;
    const entry: FileOp = { op: op.op, path: rel };
    const agentType = op.agentType ?? context.agentType;
    if (agentType !== undefined) {
      entry.agentType = agentType;
    }
    recorded.push(entry);
  }
  if (recorded.length === 0) return;

  writeFileInventory(changeDir, foldFileOps(readFileInventory(changeDir), recorded));
}
