import * as fs from "fs";
import * as path from "path";

const EVAL_JSON_FILE = "eval.json";
const SCHEMA_VERSION = "1.0";

export interface Item {
  item_id: string;
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
  backtrack_to?: string | null;
}

export interface GateResult {
  passed: boolean;
  missing: string[];
}

/**
 * Read eval.json from the phases directory.
 * Returns an empty array if the file does not exist.
 * Throws an error if JSON parsing fails.
 */
export function readEvalJson(phasesDir: string): any[] {
  const filePath = path.join(phasesDir, EVAL_JSON_FILE);
  if (!fs.existsSync(filePath)) {
    return [];
  }
  const raw = fs.readFileSync(filePath, "utf-8");
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
 * Throws an error if invalid.
 */
export function validateVerdict(verdict: any): void {
  if (verdict !== "pass" && verdict !== "fail") {
    throw new Error(`verdict 必须为 "pass" 或 "fail"，但收到: ${JSON.stringify(verdict)}`);
  }
}

/**
 * Validate that report does not exceed 500 characters.
 * Throws an error if too long.
 */
export function validateReportLength(report: string): void {
  if (report.length > 500) {
    throw new Error(
      `报告长度超过 500 字符限制（当前 ${report.length} 字符）。请精简报告内容。`
    );
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
      `items 参数不是有效的 JSON 数组。请确保使用单引号包裹 JSON 字符串，例如: --items '[{"item_id":"R1","pass":true}]'。解析错误: ${e.message}`
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
  return entry;
}

/**
 * Compute the attempt number for a given phase.
 * If explicitAttempt is provided, return it directly.
 * Otherwise, count existing entries for the phase and return count + 1.
 */
export function computeAttempt(
  entries: any[],
  phase: string,
  explicitAttempt?: number
): number {
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
 * Check that all prior phases have at least one entry with verdict "pass".
 * Returns { passed: true } if all pass, or { passed: false, missing: [...] } listing
 * phases without a pass record.
 */
export function checkGate(entries: any[], priorPhases: string[]): GateResult {
  const missing: string[] = [];
  for (const phase of priorPhases) {
    const hasPass = entries.some(
      (e: any) => e.phase === phase && e.verdict === "pass"
    );
    if (!hasPass) {
      missing.push(phase);
    }
  }
  return { passed: missing.length === 0, missing };
}

/**
 * Append an entry to eval.json in the given phases directory.
 * - Creates the directory if it does not exist.
 * - Creates eval.json with `[entry]` if it does not exist.
 * - Appends to the existing array otherwise.
 * - Output uses 2-space indentation with trailing newline.
 */
export function appendEntry(phasesDir: string, entry: object): void {
  // Ensure phases directory exists
  fs.mkdirSync(phasesDir, { recursive: true });

  const filePath = path.join(phasesDir, EVAL_JSON_FILE);
  let data: any[];

  if (fs.existsSync(filePath)) {
    data = readEvalJson(phasesDir);
  } else {
    data = [];
  }

  data.push(entry);
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + "\n", "utf-8");
}
