/**
 * phase/check MCP tool — DEPRECATED debugging utility.
 *
 * Previously used as a multi-gate check in the workflow loop. The workflow loop
 * now uses phase_next as the single decision point.
 *
 * Retained for debugging purposes only. Not called in the workflow loop.
 */

import * as fs from 'fs';

import { z } from 'zod/v4';

import { getChangeDir } from '../lib/change';
import { readEvalJson, checkGate, type GateResult, type EvalEntry } from '../lib/eval-json';
import { getPriorPhases, getPhaseIndex, PHASES } from '../lib/workflow';
import { phaseCheckInputSchema, phaseCheckOutputSchema } from '../schemas';

type PhaseCheckOptions = z.input<typeof phaseCheckInputSchema>;

type PhaseCheckResult = z.output<typeof phaseCheckOutputSchema>;

export type PhaseState = 'first_run' | 'retry' | 'passed';

export interface BuildPhaseCheckResultOptions {
  phase: string;
  priorPhases: string[];
  gateResult: GateResult;
  phaseState: PhaseState;
}

/**
 * Check that all prior phases have at least one non-stale pass record.
 * Delegates to checkGate from eval-json.ts.
 */
function checkPriorPhases(entries: EvalEntry[], priorPhases: string[]): GateResult {
  return checkGate(entries, priorPhases);
}

/**
 * Determine the state of the current phase based on its eval entries.
 * - No entries for the phase: "first_run"
 * - Latest entry verdict is "fail": "retry"
 * - Latest entry verdict is "pass" (including skipped entries): "passed"
 * - Entries with `skipped: true` are treated as "passed" regardless of verdict.
 */
function determinePhaseState(entries: EvalEntry[], currentPhase: string): PhaseState {
  const phaseEntries = entries
    .filter((e) => e.phase === currentPhase)
    .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

  if (phaseEntries.length === 0) return 'first_run';

  const latest = phaseEntries[0];
  if (latest.skipped) return 'passed';
  if (latest.verdict === 'pass') return 'passed';
  return 'retry';
}

/**
 * Aggregate check results into a simplified PhaseCheckResult object.
 * Builds block_reasons from gate failures.
 */
function buildPhaseCheckResult(options: BuildPhaseCheckResultOptions): PhaseCheckResult {
  const { phase, priorPhases, gateResult, phaseState } = options;
  const blockReasons: string[] = [];

  if (!gateResult.passed) {
    blockReasons.push(`前置阶段门控未通过: 缺少 [${gateResult.missing.join(', ')}] 的 pass 记录`);
  }

  return {
    passed: gateResult.passed,
    phase,
    prior_phases: priorPhases,
    block_reasons: blockReasons,
    phase_state: phaseState,
    details: {
      prior_phase_gate: {
        passed: gateResult.passed,
        missing: gateResult.missing,
      },
    },
  };
}

/**
 * Core logic for phase-check: validate args, read eval.json, run gate check,
 * and return a structured result.
 *
 * DEPRECATED: This function is retained for debugging only.
 * All gate-check logic has been merged into phase_next.
 */
export function runPhaseCheck(options: PhaseCheckOptions): PhaseCheckResult {
  if (!options.change || options.change === '') {
    throw new Error('缺少必填参数 --change');
  }
  if (!options.phase || options.phase === '') {
    throw new Error('缺少必填参数 --phase');
  }

  const phaseIndex = getPhaseIndex(options.phase);
  if (phaseIndex === -1) {
    throw new Error(`无效的阶段标识符 "${options.phase}"。合法阶段: ${PHASES.join(', ')}`);
  }

  const changeDir = getChangeDir(options.change);
  if (!fs.existsSync(changeDir)) {
    throw new Error(`变更 "${options.change}" 的目录不存在: ${changeDir}`);
  }

  let entries: EvalEntry[];
  try {
    entries = readEvalJson(changeDir);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`读取 eval.json 失败: ${msg}`);
  }

  const priorPhases = getPriorPhases(options.phase);

  const gateResult = checkPriorPhases(entries, priorPhases);
  const phaseState = determinePhaseState(entries, options.phase);

  return buildPhaseCheckResult({
    phase: options.phase,
    priorPhases,
    gateResult,
    phaseState,
  });
}
