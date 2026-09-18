import * as fs from 'fs';
import * as path from 'path';

import { type z } from 'zod/v4';

import { workflowFileSchema, type WorkflowFile } from '../schemas';
import { isPlainObject } from '../utils';
import { resolveChangeDir } from './change';
import { getProjectDir } from './project-root';

const WORKFLOW_JSON = 'workflow.json';

/**
 * Render Zod issues as `<field.path: message; …>` so the offending field is
 * visible to whoever has to fix the file.
 */
function formatIssues(error: z.ZodError): string {
  return error.issues
    .map((issue) => `${issue.path.map((segment) => String(segment)).join('.')}: ${issue.message}`)
    .join('; ');
}

/**
 * Read and validate `openspec/changes/<change>/workflow.json`.
 *
 * `workflow.json` is created by `change_create` (its only creator) and is the
 * precondition of `phase_next` / `backtrack` / `phase_log`, so every failure is
 * fatal here — there is no default type and no implicitly created file:
 *
 * 1. the file MUST exist;
 * 2. its content MUST be valid JSON;
 * 3. its root MUST be a plain object;
 * 4. it MUST pass `workflowFileSchema` (`workflow_type` is a required enum
 *    member, `created` — when present — MUST be `YYYY-MM-DD`, `eval` — when
 *    present — MUST be an entry array; unknown keys are allowed).
 */
function readWorkflowFile(change: string): WorkflowFile {
  const filePath = path.join(resolveChangeDir(change, getProjectDir()), WORKFLOW_JSON);

  if (!fs.existsSync(filePath)) {
    throw new Error(
      `${WORKFLOW_JSON} 不存在: ${filePath}。该文件由 change_create 建立，是工作流的前置条件；请通过 change_create 或 workflow-* skill 创建 change，不要手写该文件。`,
    );
  }

  const raw = fs.readFileSync(filePath, 'utf-8');
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (e: unknown) {
    if (e instanceof SyntaxError) {
      throw new Error(`${WORKFLOW_JSON} 解析失败: ${e.message}`);
    }
    throw e;
  }

  if (!isPlainObject(parsed)) {
    throw new Error(`${WORKFLOW_JSON} 根元素必须是对象，但实际类型为 ${typeof parsed}`);
  }

  const result = workflowFileSchema.safeParse(parsed);
  if (!result.success) {
    throw new Error(`${WORKFLOW_JSON} 格式非法 (${filePath}): ${formatIssues(result.error)}`);
  }
  return result.data;
}

/**
 * Return `workflow_type` from change `workflow.json`.
 *
 * Strict by contract: the file MUST exist and MUST pass `workflowFileSchema`,
 * otherwise this throws (see `readWorkflowFile`). It never falls back to a
 * default type — a missing or malformed file means the workflow cannot be
 * advanced and the caller terminates with the readable error.
 */
export function getWorkflowType(change: string): string {
  return readWorkflowFile(change).workflow_type;
}
