import { getChangeDir } from '../lib/change';
import { type FileInventory, readFileInventory, writeFileInventory } from '../lib/file-inventory';
import { getProjectDir } from '../lib/project-root';
import { type ChangeFilesInput, type ChangeFilesOutput } from '../schemas';

/**
 * Command options for `change_files`.
 * `project_root` is optional here (falls back to `getProjectDir()`); the MCP
 * handler always injects the resolved root.
 */
export type ChangeFilesOptions = Omit<ChangeFilesInput, 'project_root'> & {
  project_root?: string;
};

/** Dedupe a path list preserving first-seen order. */
function dedupe(paths: string[]): string[] {
  return Array.from(new Set(paths));
}

/**
 * append semantics: bucket-wise merge following the recorder fold rules —
 * each appended written path folds as `written += P ; deleted -= P`, each
 * appended deleted path as `deleted += P ; written -= P`. The audit source
 * map is NOT touched: existing paths keep their source on dedupe, newly
 * merged paths carry none (manual entries have no subagent source).
 */
function applyAppend(
  inventory: FileInventory,
  written: string[],
  deleted: string[],
): FileInventory {
  const writtenSet = new Set(inventory.written);
  const deletedSet = new Set(inventory.deleted);

  for (const p of written) {
    deletedSet.delete(p);
    writtenSet.add(p);
  }
  for (const p of deleted) {
    writtenSet.delete(p);
    deletedSet.add(p);
  }

  const result: FileInventory = {
    written: Array.from(writtenSet),
    deleted: Array.from(deletedSet),
  };
  if (inventory.source && Object.keys(inventory.source).length > 0) {
    result.source = { ...inventory.source };
  }
  return result;
}

/**
 * set semantics: wholesale overwrite of the provided buckets (untouched
 * buckets stay as-is), no append folding. Overwritten entries become
 * human-curated — their source entries are cleared — and source entries of
 * paths no longer present in the net state are dropped.
 */
function applySet(
  inventory: FileInventory,
  written: string[] | undefined,
  deleted: string[] | undefined,
): FileInventory {
  const finalWritten = written !== undefined ? dedupe(written) : inventory.written;
  const finalDeleted = deleted !== undefined ? dedupe(deleted) : inventory.deleted;

  const provided = new Set([...(written ?? []), ...(deleted ?? [])]);
  const members = new Set([...finalWritten, ...finalDeleted]);

  const source: Record<string, string> = {};
  for (const [p, agentType] of Object.entries(inventory.source ?? {})) {
    if (members.has(p) && !provided.has(p)) {
      source[p] = agentType;
    }
  }

  const result: FileInventory = { written: finalWritten, deleted: finalDeleted };
  if (Object.keys(source).length > 0) {
    result.source = source;
  }
  return result;
}

/**
 * Core logic for `change_files`: manually append to or explicitly correct the
 * change file inventory (`workflow.json.files`).
 *
 * Preconditions — the change dir and `workflow.json` must exist, the file must
 * pass `workflowFileSchema` and contain `files`; any miss throws with the
 * rebuild guidance (a change created before the inventory mechanism must be
 * recreated). Persistence goes exclusively through `writeFileInventory`, so
 * `workflow_type` / `created` / `eval` / unknown keys are preserved.
 */
export function runChangeFiles(options: ChangeFilesOptions): ChangeFilesOutput {
  const projectRoot = options.project_root || getProjectDir();
  const changeDir = getChangeDir(options.change, projectRoot);

  const inventory = readFileInventory(changeDir);

  const next =
    options.op === 'set'
      ? applySet(inventory, options.written, options.deleted)
      : applyAppend(inventory, dedupe(options.written ?? []), dedupe(options.deleted ?? []));

  writeFileInventory(changeDir, next);

  return { written: next.written, deleted: next.deleted };
}
