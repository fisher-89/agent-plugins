import { resolveChangeDir } from '../../lib/change';
import { readFileInventory } from './file-inventory';

type ChangedFiles = { written: string[]; deleted: string[] };

/**
 * Core logic for `workflow_files`: read-only query of the change file
 * inventory (`workflow.json.files`) net state.
 *
 * Strictly read-only — no fs write path, no git diff fallback, no field
 * repair. Hard-errors with the rebuild guidance when `workflow.json` is
 * missing, invalid, fails `workflowFileSchema`, or lacks `files` (same
 * semantics as every other inventory consumer, via `readFileInventory`).
 *
 * The returned projection structurally excludes the `source` audit map —
 * it is audit-only and consumers MUST NOT read it.
 */
export function getChangedFiles(changeName: string, projectRoot: string): ChangedFiles {
  const changeDir = resolveChangeDir(changeName, projectRoot);

  const inventory = readFileInventory(changeDir);

  return { written: inventory.written, deleted: inventory.deleted };
}
