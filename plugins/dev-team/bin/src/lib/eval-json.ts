import * as fs from 'fs';
import * as path from 'path';

import { getDependents } from './workflow';

const EVAL_JSON_FILE = 'eval.json';
const SCHEMA_VERSION = '1.0';

export interface Item {
  item: string;
  pass: boolean;
  evidence: string;
  notes: string;
}

export interface BuildEntryParams {
  phase: string;
  verdict: string;
  report: string;
  items: Item[];
  attempt: number;
  backtrack_to?: string | string[] | null;
  skipped?: boolean;
  findings?: string;
  phase_suffix?: string;
}

export interface GateResult {
  passed: boolean;
  missing: string[];
}

/**
 * Read eval.json from the change directory.
 * Returns an empty array if the file does not exist.
 * Throws an error if JSON parsing fails.
 */
export function readEvalJson(changeDir: string): any[] {
  const filePath = path.join(changeDir, EVAL_JSON_FILE);
  if (!fs.existsSync(filePath)) {
    return [];
  }
  const raw = fs.readFileSync(filePath, 'utf-8');
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) {
      throw new Error(`eval.json 根元素必须是数组，但实际类型为 ${typeof parsed}`);
    }
    return parsed;
  } catch (e: any) {
    if (e instanceof SyntaxError) {
      throw new Error(`eval.json 解析失败: ${e.message}`);
    }
    throw e;
  }
}

/**
 * Validate that verdict is exactly "pass" or "fail".
 * When skipped is true, "pass" verdict is always allowed (no-op skip).
 * Throws an error if invalid.
 */
export function validateVerdict(verdict: any, skipped?: boolean): void {
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
 * Validate that itemsStr is a valid JSON array string.
 * Returns the parsed array.
 * Throws an error if parsing fails.
 */
export function validateItemsJson(itemsStr: string): any[] {
  let parsed: any;
  try {
    parsed = JSON.parse(itemsStr);
  } catch (e: any) {
    throw new Error(
      `items 参数不是有效的 JSON 数组。请确保使用单引号包裹 JSON 字符串，例如: --items '[{"item":"问题描述清晰，包含背景和动机","pass":true}]'。解析错误: ${e.message}`,
    );
  }
  if (!Array.isArray(parsed)) {
    throw new Error(`items 参数必须是一个 JSON 数组，但收到: ${typeof parsed}`);
  }
  return parsed;
}

/**
 * Build an EvalEntry object with auto-generated fields.
 * - timestamp: current ISO 8601 string
 * - schema_version: "1.0"
 * - backtrack_to: included only if explicitly provided in params
 */
export function buildEntry(params: BuildEntryParams): Record<string, any> {
  const entry: Record<string, any> = {
    phase: params.phase,
    timestamp: new Date().toISOString(),
    attempt: params.attempt,
    verdict: params.verdict,
    report: params.report,
    items: params.items,
    backtrack_to: params.backtrack_to !== undefined ? params.backtrack_to : null,
    schema_version: SCHEMA_VERSION,
  };
  // Extended fields: only include when explicitly set
  if (params.skipped !== undefined) {
    entry.skipped = params.skipped;
  }
  if (params.findings !== undefined) {
    entry.findings = params.findings;
  }
  if (params.phase_suffix !== undefined) {
    entry.phase_suffix = params.phase_suffix;
  }
  return entry;
}

/**
 * Compute the attempt number for a given phase.
 * If explicitAttempt is provided, return it directly.
 * Otherwise, count existing entries for the phase and return count + 1.
 */
export function computeAttempt(entries: any[], phase: string, explicitAttempt?: number): number {
  if (explicitAttempt !== undefined) {
    if (!Number.isInteger(explicitAttempt) || explicitAttempt < 1) {
      throw new Error(`attempt 必须为正整数，但收到: ${explicitAttempt}`);
    }
    return explicitAttempt;
  }
  const phaseEntries = entries.filter((e: any) => e.phase === phase);
  return phaseEntries.length + 1;
}

/**
 * Check that all prerequisite phases have at least one non-stale entry with verdict "pass".
 * Returns { passed: true } if all pass, or { passed: false, missing: [...] } listing
 * phases without a valid pass record.
 *
 * Entries with `stale: true` are ignored (treated as not passed).
 * Entries without a `stale` field are treated as `stale: false` (backward compatible).
 */
export function checkGate(entries: any[], prerequisites: string[]): GateResult {
  const missing: string[] = [];
  for (const phase of prerequisites) {
    const hasPass = entries.some((e: any) => e.phase === phase && e.verdict === 'pass' && !e.stale);
    if (!hasPass) {
      missing.push(phase);
    }
  }
  return { passed: missing.length === 0, missing };
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
export function propagateStale(entries: any[], phaseId: string, workflowType?: string): void {
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
export function markPhaseStale(entries: any[], phaseId: string): void {
  const phaseEntries = entries
    .filter((e: any) => e.phase === phaseId)
    .sort((a: any, b: any) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

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
  propagateStale(entries, phaseId);
}

/**
 * Write the full entries array to eval.json in the given change directory.
 * Creates the directory if it does not exist.
 * Output uses 2-space indentation with trailing newline.
 */
export function writeEvalJson(changeDir: string, entries: any[]): void {
  const filePath = path.join(changeDir, EVAL_JSON_FILE);
  fs.mkdirSync(changeDir, { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(entries, null, 2) + '\n', 'utf-8');
}

/**
 * Append an entry to eval.json in the given change directory.
 * - Creates the directory if it does not exist.
 * - Creates eval.json with `[entry]` if it does not exist.
 * - Appends to the existing array otherwise.
 * - Output uses 2-space indentation with trailing newline.
 */
export function appendEntry(changeDir: string, entry: object): void {
  // Ensure change directory exists
  fs.mkdirSync(changeDir, { recursive: true });

  const filePath = path.join(changeDir, EVAL_JSON_FILE);
  let data: any[];

  if (fs.existsSync(filePath)) {
    data = readEvalJson(changeDir);
  } else {
    data = [];
  }

  data.push(entry);
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n', 'utf-8');
}
