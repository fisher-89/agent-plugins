import * as fs from "fs";
import { readEvalJson, checkGate, GateResult } from "../lib/eval-json";
import { getPriorPhases, getPhaseIndex, PHASES } from "../lib/workflow";
import { getChangeDir } from "../lib/change";

export const SCHEMA_VERSION = "1.0";

export interface EvalCheckOptions {
  change: string;
  phase: string;
}

export interface EvalCheckResult {
  passed: boolean;
  phase: string;
  prior_phases: string[];
  block_reasons: string[];
  phase_state: "first_run" | "retry" | "passed";
  details: {
    prior_phase_gate: { passed: boolean; missing: string[] };
    timestamp_order: { passed: boolean; order_valid: boolean };
    backtrack: { passed: boolean; active_backtrack_phases: string[] };
  };
}

export interface TimestampOrderResult {
  passed: boolean;
  order_valid: boolean;
  issues?: string[];
}

export interface BacktrackResult {
  passed: boolean;
  active_backtrack_phases: string[];
}

export type PhaseState = "first_run" | "retry" | "passed";

export interface BuildEvalCheckResultOptions {
  phase: string;
  priorPhases: string[];
  gateResult: GateResult;
  timestampResult: TimestampOrderResult;
  backtrackResult: BacktrackResult;
  phaseState: PhaseState;
}

/**
 * Check that all prior phases have at least one pass record.
 * Delegates to checkGate from eval-json.ts.
 * Entries with `skipped: true` are treated as passing (they carry verdict "pass").
 */
export function checkPriorPhases(entries: any[], priorPhases: string[]): GateResult {
  return checkGate(entries, priorPhases);
}

/**
 * Check that the latest pass record timestamps for prior phases are monotonically
 * non-decreasing (uses >= comparison so identical timestamps are valid).
 *
 * If fewer than 2 prior phases have pass records, order is automatically valid.
 * Phases without any pass entry are skipped (they are already caught by gate check).
 */
export function checkTimestampOrder(entries: any[], priorPhases: string[]): TimestampOrderResult {
  if (priorPhases.length < 2) {
    return { passed: true, order_valid: true };
  }

  // Collect the latest pass (or skipped) timestamp for each prior phase
  const timestamps: { phase: string; ts: string }[] = [];

  for (const phase of priorPhases) {
    const passEntries = entries
      .filter((e: any) => e.phase === phase && e.verdict === "pass")
      .filter((e: any) => !e.skipped) // Skip no-op entries for timestamp ordering
      .sort((a: any, b: any) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

    if (passEntries.length === 0) continue;

    timestamps.push({ phase, ts: passEntries[0].timestamp });
  }

  // Verify timestamps are monotonically non-decreasing
  for (let i = 1; i < timestamps.length; i++) {
    const prev = new Date(timestamps[i - 1].ts).getTime();
    const curr = new Date(timestamps[i].ts).getTime();
    if (curr < prev) {
      return {
        passed: false,
        order_valid: false,
        issues: [
          `${timestamps[i].phase} (${timestamps[i].ts}) timestamp is earlier than ${timestamps[i - 1].phase} (${timestamps[i - 1].ts})`,
        ],
      };
    }
  }

  return { passed: true, order_valid: true };
}

/**
 * Check if any prior phase's latest entry has a non-null backtrack_to field.
 * Sorts entries by timestamp descending and checks only the most recent entry
 * for each prior phase.
 */
export function checkBacktrack(entries: any[], priorPhases: string[]): BacktrackResult {
  const activeBacktrackPhases: string[] = [];

  for (const phase of priorPhases) {
    const phaseEntries = entries
      .filter((e: any) => e.phase === phase)
      .sort((a: any, b: any) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

    if (phaseEntries.length === 0) continue;

    const latest = phaseEntries[0];
    if (latest.backtrack_to != null && latest.backtrack_to !== "") {
      activeBacktrackPhases.push(phase);
    }
  }

  return {
    passed: activeBacktrackPhases.length === 0,
    active_backtrack_phases: activeBacktrackPhases,
  };
}

/**
 * Determine the state of the current phase based on its eval entries.
 * - No entries for the phase: "first_run"
 * - Latest entry verdict is "fail": "retry"
 * - Latest entry verdict is "pass" (including skipped entries): "passed"
 * - Entries with `skipped: true` are treated as "passed" regardless of verdict.
 */
export function determinePhaseState(entries: any[], currentPhase: string): PhaseState {
  const phaseEntries = entries
    .filter((e: any) => e.phase === currentPhase)
    .sort((a: any, b: any) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

  if (phaseEntries.length === 0) return "first_run";

  const latest = phaseEntries[0];
  if (latest.skipped) return "passed";
  if (latest.verdict === "pass") return "passed";
  return "retry";
}

/**
 * Check if the current phase has any entry with skipped: true.
 * Returns "(skipped: no applicable tests)" if skipped, empty string otherwise.
 * Used in human-readable output to indicate no-op phases.
 */
export function isPhaseSkipped(entries: any[], currentPhase: string): string {
  const hasSkipped = entries.some((e: any) => e.phase === currentPhase && e.skipped === true);
  if (hasSkipped) {
    return " (skipped: no applicable tests)";
  }
  return "";
}

/**
 * Aggregate all check results into a single EvalCheckResult object.
 * Builds block_reasons from any failures across all checks.
 */
export function buildEvalCheckResult(options: BuildEvalCheckResultOptions): EvalCheckResult {
  const { phase, priorPhases, gateResult, timestampResult, backtrackResult, phaseState } = options;
  const blockReasons: string[] = [];

  if (!gateResult.passed) {
    blockReasons.push(`前置阶段门控未通过: 缺少 [${gateResult.missing.join(", ")}] 的 pass 记录`);
  }

  if (!timestampResult.passed) {
    if (timestampResult.issues && timestampResult.issues.length > 0) {
      blockReasons.push(
        `前置阶段 pass 记录时间戳未按阶段顺序单调递增: ${timestampResult.issues.join("; ")}`,
      );
    } else {
      blockReasons.push("前置阶段 pass 记录时间戳未按阶段顺序单调递增");
    }
  }

  if (!backtrackResult.passed) {
    for (const bp of backtrackResult.active_backtrack_phases) {
      blockReasons.push(`前置阶段 ${bp} 存在活跃 backtrack 标记`);
    }
  }

  return {
    passed: gateResult.passed && timestampResult.passed && backtrackResult.passed,
    phase,
    prior_phases: priorPhases,
    block_reasons: blockReasons,
    phase_state: phaseState,
    details: {
      prior_phase_gate: {
        passed: gateResult.passed,
        missing: gateResult.missing,
      },
      timestamp_order: {
        passed: timestampResult.passed,
        order_valid: timestampResult.order_valid,
      },
      backtrack: {
        passed: backtrackResult.passed,
        active_backtrack_phases: backtrackResult.active_backtrack_phases,
      },
    },
  };
}

/**
 * Check if any entry in the eval.json has a schema_version different from the
 * expected version. Returns an array of warning messages (empty if all match).
 */
export function checkSchemaVersion(
  entries: any[],
  expectedVersion: string = SCHEMA_VERSION,
): string[] {
  const warnings: string[] = [];
  for (const entry of entries) {
    if (entry.schema_version && entry.schema_version !== expectedVersion) {
      const msg = `警告: eval.json 中的 schema_version 为 "${entry.schema_version}"，当前 CLI 版本为 "${expectedVersion}"，可能存在不兼容`;
      if (!warnings.includes(msg)) {
        warnings.push(msg);
      }
    }
  }
  return warnings;
}

/**
 * Core logic for eval-check: validate args, read eval.json, run all checks,
 * and return a structured result. Extracted so both CLI and MCP server can call it.
 */
export function runEvalCheck(options: EvalCheckOptions): EvalCheckResult {
  if (!options.change || options.change === "") {
    throw new Error("缺少必填参数 --change");
  }
  if (!options.phase || options.phase === "") {
    throw new Error("缺少必填参数 --phase");
  }

  const phaseIndex = getPhaseIndex(options.phase);
  if (phaseIndex === -1) {
    throw new Error(
      `无效的阶段标识符 "${options.phase}"。合法阶段: ${PHASES.join(", ")}`,
    );
  }

  const changeDir = getChangeDir(options.change);
  if (!fs.existsSync(changeDir)) {
    throw new Error(`变更 "${options.change}" 的目录不存在: ${changeDir}`);
  }

  let entries: any[];
  try {
    entries = readEvalJson(changeDir);
  } catch (e: any) {
    throw new Error(`读取 eval.json 失败: ${e.message}`);
  }

  const priorPhases = getPriorPhases(options.phase);

  const gateResult = checkPriorPhases(entries, priorPhases);
  const timestampResult = checkTimestampOrder(entries, priorPhases);
  const backtrackResult = checkBacktrack(entries, priorPhases);
  const phaseState = determinePhaseState(entries, options.phase);

  return buildEvalCheckResult({
    phase: options.phase,
    priorPhases,
    gateResult,
    timestampResult,
    backtrackResult,
    phaseState,
  });
}
