import * as fs from 'fs';
import * as path from 'path';

import { type z } from 'zod/v4';

import { phaseLogSchema, workflowEvalSchema, workflowFileSchema } from '../schemas';
import { isPlainObject } from '../utils';
import { getDependents } from './workflow';

const WORKFLOW_JSON_FILE = 'workflow.json';
const LEGACY_EVAL_JSON_FILE = 'eval.json';

export type EvalEntry = z.infer<typeof phaseLogSchema>;

export type BuildEntryParams = Pick<
  EvalEntry,
  'phase' | 'attempt' | 'verdict' | 'report' | 'checklist' | 'skipped'
> & {
  /** Attempt start time (ISO 8601); written into the entry only when explicitly provided. */
  start_at?: string;
};

/**
 * Render Zod issues as `<field.path: message; …>`, mirroring the format used
 * by `lib/change-config.ts`.
 */
function formatIssues(error: z.ZodError): string {
  return error.issues
    .map((issue) => `${issue.path.map((segment) => String(segment)).join('.')}: ${issue.message}`)
    .join('; ');
}

/**
 * Parse `workflow.json` and assert that the root is a plain object.
 * Throws a readable error when JSON is invalid or the root is not an object.
 * Field shapes are validated separately against `workflowFileSchema`.
 */
function parseWorkflowJson(raw: string): Record<string, unknown> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (e: unknown) {
    if (e instanceof SyntaxError) {
      throw new Error(`workflow.json 解析失败: ${e.message}`);
    }
    throw e;
  }
  if (!isPlainObject(parsed)) {
    throw new Error(`workflow.json 根元素必须是对象，但实际类型为 ${typeof parsed}`);
  }
  return parsed;
}

/**
 * Read the authoritative `eval` array from `workflow.json`.
 * Returns `null` when the file or the `eval` key is absent, so the caller can
 * fall back to the legacy `eval.json`.
 * Throws when the JSON is invalid, the root is not an object, or `eval` is
 * present but not an array.
 *
 * Only the `eval` field is validated here (via `workflowEvalSchema`); the rest
 * of the file is validated by the full-file readers (`getWorkflowType` /
 * `writeEvalJson`) so that entry reads stay independent of `workflow_type`.
 */
function readWorkflowEval(changeDir: string): EvalEntry[] | null {
  const filePath = path.join(changeDir, WORKFLOW_JSON_FILE);
  if (!fs.existsSync(filePath)) {
    return null;
  }
  const parsed = parseWorkflowJson(fs.readFileSync(filePath, 'utf-8'));
  if (!Object.prototype.hasOwnProperty.call(parsed, 'eval')) {
    return null;
  }
  const evalValue = parsed.eval;
  if (!Array.isArray(evalValue)) {
    throw new Error(`workflow.json.eval 必须是数组，但实际类型为 ${typeof evalValue}`);
  }
  return workflowEvalSchema.parse(evalValue);
}

/**
 * Read a legacy `eval.json` array file (read-only fallback).
 * Returns an empty array if the file does not exist.
 * Throws an error if JSON parsing fails or the root is not an array.
 */
function readLegacyEvalJson(changeDir: string): EvalEntry[] {
  const filePath = path.join(changeDir, LEGACY_EVAL_JSON_FILE);
  if (!fs.existsSync(filePath)) {
    return [];
  }
  const raw = fs.readFileSync(filePath, 'utf-8');
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      throw new Error(`eval.json 根元素必须是数组，但实际类型为 ${typeof parsed}`);
    }
    return parsed.map((record) => phaseLogSchema.parse(record));
  } catch (e: unknown) {
    if (e instanceof SyntaxError) {
      throw new Error(`eval.json 解析失败: ${e.message}`);
    }
    throw e;
  }
}

/**
 * Read the evaluation entries of a change directory.
 *
 * Priority — the two sources are never merged:
 * 1. `workflow.json` exists and its `eval` key is an array (including `[]`) →
 *    return that array, ignoring any legacy `eval.json`.
 * 2. Otherwise, a legacy `eval.json` exists → parse it with the legacy rules.
 * 3. Otherwise → `[]`.
 *
 * Throws when `workflow.json` is invalid, has a non-object root, or stores a
 * non-array `eval`; throws when a legacy `eval.json` root is not an array.
 */
export function readEvalJson(changeDir: string): EvalEntry[] {
  const workflowEntries = readWorkflowEval(changeDir);
  if (workflowEntries !== null) {
    return workflowEntries;
  }
  return readLegacyEvalJson(changeDir);
}

/**
 * Validate that verdict is exactly "pass" or "fail".
 * When skipped is true, "pass" verdict is always allowed (no-op skip).
 * Throws an error if invalid.
 */
export function validateVerdict(verdict: string, skipped?: boolean): void {
  if (verdict !== 'pass' && verdict !== 'fail') {
    throw new Error(`verdict 必须为 "pass" 或 "fail"，但收到: ${JSON.stringify(verdict)}`);
  }
  if (skipped && verdict !== 'pass') {
    throw new Error(`skipped=true 时 verdict 必须为 "pass"，但收到: ${JSON.stringify(verdict)}`);
  }
}

/**
 * Validate that report does not exceed 500 characters.
 * Throws an error if too long.
 */
export function validateReportLength(report: string): void {
  if (report.length > 500) {
    throw new Error(`报告长度超过 500 字符限制（当前 ${report.length} 字符）。请精简报告内容。`);
  }
}

/**
 * Build an EvalEntry object with auto-generated fields.
 * - timestamp: current ISO 8601 string
 * - start_at: written only when explicitly provided (stamped from
 *   `active_phase` by `phase_log`)
 * - backtrack_to/reason: not set here (handled by standalone backtrack tool)
 */
export function buildEntry(params: BuildEntryParams): EvalEntry {
  const entry: EvalEntry = {
    phase: params.phase,
    attempt: params.attempt,
    verdict: params.verdict,
    report: params.report,
    checklist: params.checklist,
    timestamp: new Date().toISOString(),
  };
  // Extended fields: only include when explicitly set
  if (params.skipped !== undefined) {
    entry.skipped = params.skipped;
  }
  if (params.start_at !== undefined) {
    entry.start_at = params.start_at;
  }
  return phaseLogSchema.parse(entry);
}

/**
 * Compute the attempt number for a given phase.
 * If explicitAttempt is provided, return it directly.
 * Otherwise, count existing entries for the phase and return count + 1.
 */
export function computeAttempt(
  entries: EvalEntry[],
  phase: string,
  explicitAttempt?: number,
): number {
  if (explicitAttempt !== undefined) {
    if (!Number.isInteger(explicitAttempt) || explicitAttempt < 1) {
      throw new Error(`attempt 必须为正整数，但收到: ${explicitAttempt}`);
    }
    return explicitAttempt;
  }
  const phaseEntries = entries.filter((e) => e.phase === phase);
  return phaseEntries.length + 1;
}

/**
 * Recursively mark all downstream dependent entries as stale, walking the
 * dependency graph via `getDependents()`.
 *
 * For each dependent phase, ALL entries (pass and fail) are marked stale.
 * Uses a visited-set to prevent infinite loops (defensive — the dependency
 * graph is a DAG).
 *
 * Dependent phases with no entries in the array are silently skipped.
 */
function propagateStale(entries: EvalEntry[], phaseId: string, workflowType: string): void {
  const visited = new Set<string>();

  function propagate(pid: string): void {
    if (visited.has(pid)) return;
    visited.add(pid);

    const dependents = getDependents(pid, workflowType);
    for (const depId of dependents) {
      if (visited.has(depId)) continue;
      // Mark ALL entries for this dependent phase as stale
      for (const entry of entries) {
        if (entry.phase === depId) {
          entry.stale = true;
        }
      }
      propagate(depId);
    }
  }

  propagate(phaseId);
}

/**
 * Mark the latest pass entry for a given phase as stale, then immediately
 * propagate staleness to all downstream dependents.
 *
 * 1. Finds all entries for `phaseId`, sorted by timestamp descending.
 * 2. Marks the first entry with `verdict === 'pass'` as `stale: true`.
 * 3. Immediately calls `propagateStale()` to mark downstream dependents stale.
 *
 * If no pass entry exists, the function is a no-op (no propagation occurs).
 */
export function markPhaseStale(entries: EvalEntry[], phaseId: string, workflowType: string): void {
  const phaseEntries = entries
    .filter((e) => e.phase === phaseId)
    .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

  // Find the latest pass entry
  // We need to iterate through entries (which are mutated) to find a non-stale pass
  let found = false;
  for (const entry of phaseEntries) {
    if (entry.verdict === 'pass') {
      entry.stale = true;
      found = true;
      break;
    }
  }

  if (!found) {
    // No pass entry found — no-op
    return;
  }

  // Immediately propagate downstream
  propagateStale(entries, phaseId, workflowType);
}

/**
 * Write the full entries array to the `eval` field of `workflow.json` in the
 * given change directory.
 *
 * Preconditions — the file MUST already exist and MUST be a valid workflow
 * file; `change_create` is its only creator, so this function never creates
 * the directory, the file, or any default metadata:
 *
 * - missing file → throws (message carries the absolute path and the
 *   `change_create` hint); MUST NOT `mkdirSync` nor create the file;
 * - `JSON.parse` failure, non-object root, or `workflowFileSchema` failure →
 *   throws and leaves the on-disk content untouched.
 *
 * On success:
 * - `eval` is replaced by the complete `entries` array (never merged with a
 *   legacy `eval.json`);
 * - `workflow_type`, `created` and any unknown key are preserved as-is;
 * - output uses 2-space indentation with a trailing newline;
 * - a leftover legacy `eval.json` is deleted; it is NEVER written.
 */
export function writeEvalJson(changeDir: string, entries: EvalEntry[]): void {
  const filePath = path.join(changeDir, WORKFLOW_JSON_FILE);

  if (!fs.existsSync(filePath)) {
    throw new Error(
      `workflow.json 不存在: ${filePath}，无法写入评估记录。请先通过 change_create 创建 change。`,
    );
  }

  const doc = parseWorkflowJson(fs.readFileSync(filePath, 'utf-8'));

  const validated = workflowFileSchema.safeParse(doc);
  if (!validated.success) {
    throw new Error(`workflow.json 格式非法 (${filePath}): ${formatIssues(validated.error)}`);
  }

  if (!Array.isArray(entries)) {
    throw new Error(`workflow.json.eval 必须是数组，但实际类型为 ${typeof entries}`);
  }

  doc.eval = entries;

  fs.writeFileSync(filePath, `${JSON.stringify(doc, null, 2)}\n`, 'utf-8');

  const legacyPath = path.join(changeDir, LEGACY_EVAL_JSON_FILE);
  if (fs.existsSync(legacyPath)) {
    fs.unlinkSync(legacyPath);
  }
}

/**
 * Append an entry to the evaluation history in the given change directory.
 * - The directory and `workflow.json` MUST already exist (`change_create`
 *   creates them); this function never creates either.
 * - `entry` MUST be defined: a missing entry (null / undefined) would otherwise
 *   be pushed as-is and persisted as a non-object hole in the `eval` array,
 *   corrupting every later read — so it throws instead of writing. The value is
 *   NOT re-parsed through `phaseLogSchema` (that would strip unknown keys).
 * - Reads the current entries (authoritative `workflow.json.eval`, legacy
 *   `eval.json` fallback), pushes the entry, and writes the whole array back.
 * - Output uses 2-space indentation with trailing newline.
 */
export function appendEntry(changeDir: string, entry: EvalEntry): void {
  if (entry === undefined || entry === null) {
    throw new Error(`评估条目不能为空（收到 ${typeof entry}），无法追加到 workflow.json.eval`);
  }
  const entries = readEvalJson(changeDir);
  entries.push(entry);
  writeEvalJson(changeDir, entries);
}
