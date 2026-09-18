/**
 * file-inventory.ts — change file inventory (`workflow.json.files`) shared
 * library under `modules/workflow` (the module boundary for `workflow.json`
 * operation logic; everything exports via `workflow/index.ts`).
 *
 * The inventory is the net state of file operations recorded for a change:
 * `written` / `deleted` path lists (relative to project root, POSIX style)
 * plus an optional side-map `source` (path → subagent `agent_type`) used for
 * audit only — no consumer may depend on it.
 *
 * Consumed by the PostToolUse recorder, `change_files`, `workflow_files`,
 * `test-execution`, `test-resolve-paths` and `c4-cross-ref`.
 */

import * as fs from 'fs';
import * as path from 'path';

import { type z } from 'zod/v4';

import { workflowFileSchema, type workflowFilesSchema } from '../../schemas';
import { isPlainObject } from '../../utils';

const WORKFLOW_JSON_FILE = 'workflow.json';

/** Net state of the change file inventory stored in `workflow.json.files`. */
export type FileInventory = z.infer<typeof workflowFilesSchema>;

/** One observed file operation, as extracted by the hooks extractor. */
export interface FileOp {
  op: 'write' | 'delete' | 'revert';
  path: string;
  /** Subagent `agent_type` carried by the hook event; absent for main-session ops. */
  agentType?: string;
}

/**
 * Render Zod issues as `<field.path: message; …>`, mirroring the format used
 * by `lib/eval-json.ts`.
 */
function formatIssues(error: z.ZodError): string {
  return error.issues
    .map((issue) => `${issue.path.map((segment) => String(segment)).join('.')}: ${issue.message}`)
    .join('; ');
}

/**
 * Read and validate the change file inventory from `<changeDir>/workflow.json`.
 *
 * Hard-errors — no git fallback — when:
 * 1. the file does not exist;
 * 2. the JSON is invalid or the root is not an object;
 * 3. the file fails `workflowFileSchema`;
 * 4. the `files` field is missing (a change created before the inventory
 *    mechanism — the message carries the rebuild guidance).
 */
export function readFileInventory(changeDir: string): FileInventory {
  const filePath = path.join(changeDir, WORKFLOW_JSON_FILE);

  if (!fs.existsSync(filePath)) {
    throw new Error(
      `workflow.json 不存在: ${filePath}。该文件由 change_create 建立；请通过 change_create 创建 change。`,
    );
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
  } catch (e: unknown) {
    if (e instanceof SyntaxError) {
      throw new Error(`workflow.json 解析失败: ${e.message}`);
    }
    throw e;
  }

  if (!isPlainObject(parsed)) {
    throw new Error(`workflow.json 根元素必须是对象，但实际类型为 ${typeof parsed}`);
  }

  const validated = workflowFileSchema.safeParse(parsed);
  if (!validated.success) {
    throw new Error(`workflow.json 格式非法 (${filePath}): ${formatIssues(validated.error)}`);
  }

  if (validated.data.files === undefined) {
    throw new Error(
      `workflow.json 缺少 files 字段 (${filePath})：该 change 创建于文件清单机制之前，请重建该 change（change_create）。`,
    );
  }

  return validated.data.files;
}

/**
 * Maintain the `source` audit map for a write/delete op: an op carrying
 * `agentType` overwrites the entry (last-writer-wins), an op without one
 * clears it (main-session ops and manual append carry no subagent source).
 */
function setSource(source: Record<string, string>, target: string, agentType?: string): void {
  if (agentType !== undefined) {
    source[target] = agentType;
  } else {
    delete source[target];
  }
}

/**
 * Fold a batch of observed file operations into the inventory net state
 * (pure function; returns a new inventory).
 *
 * Symmetric rules — folding only overstates, never underrecords dangerous
 * operations:
 * ```
 * write(P)   →  deleted -= P ;  written += P
 * delete(P)  →  written -= P ;  deleted  += P
 * revert(P)  →  written -= P ;  deleted -= P
 * ```
 *
 * `source` maintenance: an op carrying `agentType` overwrites the entry
 * (last-writer-wins); an op without `agentType` clears it; removing a path
 * from both buckets (revert) removes its source too. An empty `source` map is
 * dropped from the result.
 */
export function foldFileOps(inventory: FileInventory, ops: FileOp[]): FileInventory {
  const written = new Set(inventory.written);
  const deleted = new Set(inventory.deleted);
  const source: Record<string, string> = { ...inventory.source };

  for (const op of ops) {
    const target = op.path;
    switch (op.op) {
      case 'write':
        deleted.delete(target);
        written.add(target);
        setSource(source, target, op.agentType);
        break;
      case 'delete':
        written.delete(target);
        deleted.add(target);
        setSource(source, target, op.agentType);
        break;
      case 'revert':
        written.delete(target);
        deleted.delete(target);
        delete source[target];
        break;
    }
  }

  const result: FileInventory = {
    written: Array.from(written),
    deleted: Array.from(deleted),
  };
  if (Object.keys(source).length > 0) {
    result.source = source;
  }
  return result;
}

/**
 * Write the inventory net state back to the `files` field of
 * `<changeDir>/workflow.json`.
 *
 * Write discipline mirrors `lib/eval-json.ts`:
 * - the file MUST already exist (`change_create` is its only creator) — a
 *   missing file throws and this function never creates the file or directory;
 * - the on-disk content MUST be valid JSON with a plain-object root passing
 *   `workflowFileSchema` — otherwise it throws and leaves the file untouched;
 * - `workflow_type`, `created`, `eval` and unknown keys are preserved as-is;
 * - output uses 2-space indentation with a trailing newline.
 */
export function writeFileInventory(changeDir: string, files: FileInventory): void {
  const filePath = path.join(changeDir, WORKFLOW_JSON_FILE);

  if (!fs.existsSync(filePath)) {
    throw new Error(
      `workflow.json 不存在: ${filePath}，无法写入文件清单。请先通过 change_create 创建 change。`,
    );
  }

  let doc: unknown;
  try {
    doc = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
  } catch (e: unknown) {
    if (e instanceof SyntaxError) {
      throw new Error(`workflow.json 解析失败: ${e.message}`);
    }
    throw e;
  }

  if (!isPlainObject(doc)) {
    throw new Error(`workflow.json 根元素必须是对象，但实际类型为 ${typeof doc}`);
  }

  const validated = workflowFileSchema.safeParse(doc);
  if (!validated.success) {
    throw new Error(`workflow.json 格式非法 (${filePath}): ${formatIssues(validated.error)}`);
  }

  doc.files = files;

  fs.writeFileSync(filePath, `${JSON.stringify(doc, null, 2)}\n`, 'utf-8');
}
