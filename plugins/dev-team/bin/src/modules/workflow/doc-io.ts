/**
 * doc-io.ts — shared `workflow.json` read/write primitives for the workflow
 * module's subdirectories (`files/` file inventory and `phase/` running
 * state). Deliberately module-internal: NOT re-exported from the barrel, so
 * the module's public surface stays read/write-semantic channels.
 *
 * Write discipline (mirrors `lib/eval-json.ts`): the file MUST already exist
 * (`change_create` is its only creator), the on-disk content MUST be valid
 * JSON with a plain-object root passing `workflowFileSchema` before any
 * mutation, unknown keys are preserved as-is, and output uses 2-space
 * indentation with a trailing newline.
 */

import * as fs from 'fs';
import * as path from 'path';

import { type z } from 'zod/v4';

import { workflowFileSchema } from '../../schemas';
import { isPlainObject } from '../../utils';

const WORKFLOW_JSON_FILE = 'workflow.json';

/** Absolute path of a change's `workflow.json`. */
export function getWorkflowJsonPath(changeDir: string): string {
  return path.join(changeDir, WORKFLOW_JSON_FILE);
}

/**
 * Render Zod issues as `<field.path: message; …>`, mirroring the format used
 * by `lib/eval-json.ts`.
 */
function formatWorkflowIssues(error: z.ZodError): string {
  return error.issues
    .map((issue) => `${issue.path.map((segment) => String(segment)).join('.')}: ${issue.message}`)
    .join('; ');
}

/**
 * Load `workflow.json` as the raw document: existence check → JSON parse →
 * plain-object root → `workflowFileSchema` validation (validation gates the
 * load only). Any failure throws and leaves the file untouched. The raw
 * document is returned — not the parsed projection — so unknown keys (root
 * and nested) survive a subsequent `saveWorkflowDoc` byte-for-byte.
 */
export function loadWorkflowDoc(changeDir: string): Record<string, unknown> {
  const filePath = getWorkflowJsonPath(changeDir);

  if (!fs.existsSync(filePath)) {
    throw new Error(
      `${WORKFLOW_JSON_FILE} 不存在: ${filePath}。该文件由 change_create 建立；请通过 change_create 创建 change。`,
    );
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
  } catch (e: unknown) {
    if (e instanceof SyntaxError) {
      throw new Error(`${WORKFLOW_JSON_FILE} 解析失败: ${e.message}`);
    }
    throw e;
  }

  if (!isPlainObject(parsed)) {
    throw new Error(`${WORKFLOW_JSON_FILE} 根元素必须是对象，但实际类型为 ${typeof parsed}`);
  }

  const validated = workflowFileSchema.safeParse(parsed);
  if (!validated.success) {
    throw new Error(
      `${WORKFLOW_JSON_FILE} 格式非法 (${filePath}): ${formatWorkflowIssues(validated.error)}`,
    );
  }

  return parsed;
}

/**
 * Persist a loaded document back to `workflow.json`: the file MUST already
 * exist (this function never creates it), output uses 2-space indentation
 * with a trailing newline. Callers MUST have loaded the document via
 * `loadWorkflowDoc` (which validates) before mutating — this function does
 * not re-validate, so the raw document's unknown keys pass through untouched.
 */
export function saveWorkflowDoc(changeDir: string, doc: Record<string, unknown>): void {
  const filePath = getWorkflowJsonPath(changeDir);

  if (!fs.existsSync(filePath)) {
    throw new Error(
      `${WORKFLOW_JSON_FILE} 不存在: ${filePath}，无法写入。请先通过 change_create 创建 change。`,
    );
  }

  fs.writeFileSync(filePath, `${JSON.stringify(doc, null, 2)}\n`, 'utf-8');
}
