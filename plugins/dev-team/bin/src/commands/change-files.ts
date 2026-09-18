import { resolveChangeDir } from '../lib/change';
import { getProjectDir } from '../lib/project-root';
import { appendWorkflowFiles, setWorkflowFiles } from '../modules/workflow';
import { type ChangeFilesInput, type ChangeFilesOutput } from '../schemas';

/**
 * Command options for `change_files`.
 * `project_root` is optional here (falls back to `getProjectDir()`); the MCP
 * handler always injects the resolved root.
 */
export type ChangeFilesOptions = Omit<ChangeFilesInput, 'project_root'> & {
  project_root?: string;
};

/**
 * Core logic for `change_files`: manually append to or explicitly correct the
 * change file inventory (`workflow.json.file_log`).
 *
 * Preconditions — the change dir and `workflow.json` must exist, the file must
 * pass `workflowFileSchema` and contain `file_log`; any miss throws with the
 * rebuild guidance (a change created before the log mechanism must be
 * recreated). The append/set semantics — workflow-scope per-path upsert
 * (append) vs remove-every-touching-record-then-append (set) and the write
 * discipline preserving `workflow_type` / `created` / `eval` — live in the
 * workflow module. This is the manual backfill / correction channel:
 * deliberately NO gitignore filtering applies here (that is the recorder's
 * concern).
 */
export function runChangeFiles(options: ChangeFilesOptions): ChangeFilesOutput {
  const projectRoot = options.project_root || getProjectDir();
  const changeDir = resolveChangeDir(options.change, projectRoot);

  const paths = { written: options.written, deleted: options.deleted };
  const next =
    options.op === 'set'
      ? setWorkflowFiles(changeDir, paths)
      : appendWorkflowFiles(changeDir, paths);

  return { written: next.written, deleted: next.deleted };
}
