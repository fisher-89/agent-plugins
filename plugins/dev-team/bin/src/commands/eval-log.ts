import {
  readEvalJson,
  validateVerdict,
  validateReportLength,
  validateItemsJson,
  buildEntry,
  computeAttempt,
  checkGate,
  appendEntry,
} from "../lib/eval-json";
import { getPriorPhases } from "../lib/workflow";
import { getChangeDir } from "../lib/change";

export interface EvalLogOptions {
  change: string;
  phase: string;
  verdict: string;
  report: string;
  items: string;
  attempt?: string;
  backtrackTo?: string;
  skipped?: boolean;
  findings?: string;
}

export interface EvalLogResult {
  written: boolean;
  phase: string;
  attempt: number;
}

/**
 * Core logic for eval-log: validate, gate-check, build entry, and append to eval.json.
 * Extracted so both CLI and MCP server can call the same logic.
 */
export function runEvalLog(options: EvalLogOptions): EvalLogResult {
  const REQUIRED_ARGS = ["change", "phase", "verdict", "report", "items"] as const;
  const missing = REQUIRED_ARGS.filter((r) => !(options as any)[r] || (options as any)[r] === "");
  if (missing.length > 0) {
    throw new Error(`缺少必填参数: --${missing.join(", --")}`);
  }

  validateVerdict(options.verdict, options.skipped === true);
  validateReportLength(options.report);

  const itemsList = validateItemsJson(options.items);

  const changeDir = getChangeDir(options.change);

  let entries: any[];
  try {
    entries = readEvalJson(changeDir);
  } catch (e: any) {
    throw new Error(`读取 eval.json 失败: ${e.message}`);
  }

  const priorPhases = getPriorPhases(options.phase);
  if (priorPhases.length > 0) {
    const gate = checkGate(entries, priorPhases);
    if (!gate.passed) {
      throw new Error(
        `门控检查未通过 - 以下前置阶段缺少 pass 记录: ${gate.missing.join(", ")}`,
      );
    }
  }

  const explicitAttempt = options.attempt ? parseInt(options.attempt, 10) : undefined;
  const attempt = computeAttempt(entries, options.phase, explicitAttempt);

  const entry = buildEntry({
    phase: options.phase,
    verdict: options.verdict,
    report: options.report,
    items: itemsList,
    attempt,
    backtrack_to: options.backtrackTo || null,
    skipped: options.skipped === true ? true : undefined,
    findings: options.findings || undefined,
  });

  try {
    appendEntry(changeDir, entry);
  } catch (e: any) {
    throw new Error(`写入 eval.json 失败: ${e.message}`);
  }

  return { written: true, phase: options.phase, attempt };
}
