import { CAC } from "cac";
import * as fs from "fs";
import { readEvalJson, checkGate, GateResult } from "../lib/eval-json";
import { getPriorPhases, getPhaseIndex, PHASES } from "../lib/workflow";
import { getPhasesDir } from "../lib/change";

export const SCHEMA_VERSION = "1.0";

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

  // Collect the latest pass timestamp for each prior phase
  const timestamps: { phase: string; ts: string }[] = [];

  for (const phase of priorPhases) {
    const passEntries = entries
      .filter((e: any) => e.phase === phase && e.verdict === "pass")
      .sort(
        (a: any, b: any) =>
          new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
      );

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
      .sort(
        (a: any, b: any) =>
          new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
      );

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
 * - Latest entry verdict is "pass": "passed"
 */
export function determinePhaseState(entries: any[], currentPhase: string): PhaseState {
  const phaseEntries = entries
    .filter((e: any) => e.phase === currentPhase)
    .sort(
      (a: any, b: any) =>
        new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
    );

  if (phaseEntries.length === 0) return "first_run";

  const latest = phaseEntries[0];
  if (latest.verdict === "pass") return "passed";
  return "retry";
}

/**
 * Aggregate all check results into a single EvalCheckResult object.
 * Builds block_reasons from any failures across all checks.
 */
export function buildEvalCheckResult(options: BuildEvalCheckResultOptions): EvalCheckResult {
  const { phase, priorPhases, gateResult, timestampResult, backtrackResult, phaseState } =
    options;
  const blockReasons: string[] = [];

  if (!gateResult.passed) {
    blockReasons.push(
      `前置阶段门控未通过: 缺少 [${gateResult.missing.join(", ")}] 的 pass 记录`
    );
  }

  if (!timestampResult.passed) {
    if (timestampResult.issues && timestampResult.issues.length > 0) {
      blockReasons.push(
        `前置阶段 pass 记录时间戳未按阶段顺序单调递增: ${timestampResult.issues.join("; ")}`
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
  expectedVersion: string = SCHEMA_VERSION
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
 * Register the eval-check subcommand on the given cac CLI instance.
 * Defines --change (required), --phase (required), and --json (optional) options.
 *
 * Action handler:
 * 1. Validates arguments
 * 2. Validates phase name against known phases
 * 3. Verifies change phases directory exists
 * 4. Reads eval entries
 * 5. Checks schema_version consistency (warning only)
 * 6. Runs all checks (gate, timestamp order, backtrack, phase state)
 * 7. Outputs result (JSON or human-readable)
 * 8. Exits with appropriate code (0 = pass, 1 = block)
 */
export function registerEvalCheckCommand(cli: CAC): void {
  cli
    .command(
      "eval-check",
      "Check if all prior phases have passed evaluation for a given phase"
    )
    .option(
      "--change <name>",
      "Change name (corresponds to openspec/changes/<name>)"
    )
    .option(
      "--phase <phase>",
      "Phase identifier (e.g. 03-dev-proposal)"
    )
    .option("--json", "Output structured JSON result instead of human-readable text")
    .action((options: Record<string, any>) => {
      // 1) Validate required arguments
      if (!options.change || options.change === "") {
        console.error("错误: 缺少必填参数 --change");
        process.exit(1);
      }
      if (!options.phase || options.phase === "") {
        console.error("错误: 缺少必填参数 --phase");
        process.exit(1);
      }

      // 2) Validate phase is a known workflow phase
      const phaseIndex = getPhaseIndex(options.phase);
      if (phaseIndex === -1) {
        console.error(
          `错误: 无效的阶段标识符 "${options.phase}"。合法阶段: ${PHASES.join(", ")}`
        );
        process.exit(1);
      }

      // 3) Resolve phases directory and verify it exists
      const phasesDir = getPhasesDir(options.change);
      if (!fs.existsSync(phasesDir)) {
        console.error(
          `错误: 变更 "${options.change}" 的 phases 目录不存在: ${phasesDir}`
        );
        process.exit(1);
      }

      // 4) Read eval entries
      let entries: any[];
      try {
        entries = readEvalJson(phasesDir);
      } catch (e: any) {
        console.error(`错误: 读取 eval.json 失败: ${e.message}`);
        process.exit(1);
      }

      // 5) Schema version warning (non-blocking)
      const schemaWarnings = checkSchemaVersion(entries);
      for (const warn of schemaWarnings) {
        console.error(warn);
      }

      // 6) Identify prior phases
      const priorPhases = getPriorPhases(options.phase);

      // 7) Run all checks
      const gateResult = checkPriorPhases(entries, priorPhases);
      const timestampResult = checkTimestampOrder(entries, priorPhases);
      const backtrackResult = checkBacktrack(entries, priorPhases);
      const phaseState = determinePhaseState(entries, options.phase);

      // 8) Build result
      const result = buildEvalCheckResult({
        phase: options.phase,
        priorPhases,
        gateResult,
        timestampResult,
        backtrackResult,
        phaseState,
      });

      // 9) Output
      if (options.json) {
        console.log(JSON.stringify(result));
      } else {
        if (result.passed) {
          const priorDesc =
            priorPhases.length > 0
              ? `所有前置阶段 (${priorPhases.join(", ")}) 已通过评估`
              : "无前置阶段需要检查";
          console.log(`检查通过: ${priorDesc}`);
          console.log(`当前阶段状态: ${result.phase_state}`);
        } else {
          console.error("检查阻断: 存在以下问题:");
          for (const reason of result.block_reasons) {
            console.error(`  - ${reason}`);
          }
        }
      }

      // 10) Exit with appropriate code
      if (!result.passed) {
        process.exit(1);
      }
    });
}
