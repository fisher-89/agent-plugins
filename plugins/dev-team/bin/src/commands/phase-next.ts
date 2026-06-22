/**
 * phase_next MCP tool — server-side orchestration logic.
 *
 * Determines the next phase to execute in a PGE workflow based on eval.json entries.
 * Handles: initial run, normal progression, retry, backtrack, round limit,
 * mid-phase interruption, and skipped entries.
 *
 * KEY CHANGE: phase_next is now READ-ONLY. It never modifies eval.json.
 * Stale marking and propagation are handled by phase_log when writing entries
 * with backtrack_to.
 *
 * The workflow skill calls phase_next in a loop and executes the returned
 * planner/evaluator agents without any hardcoded phase knowledge.
 */

import { getChangeDir } from '../lib/change';
import { readEvalJson, type EvalEntry } from '../lib/eval-json';
import { getPhaseTable, type PhaseAgentDef, type PhaseDefinition } from '../lib/workflow';

// ---------------------------------------------------------------------------
// Types (local to phase_next)
// ---------------------------------------------------------------------------

export interface PhaseNextOptions {
  change: string;
  workflow_type?: string;
}

export interface PhaseNextResult {
  done: boolean;
  error: string | null;
  message: string | null;
  next_phase: string | null;
  phase_pattern: string | null;
  planner: PhaseAgentDef | null;
  evaluator: PhaseAgentDef | null;
  auto_steps: string[];
  total_phases: number;
  phase_index: number;
  round: number;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const DEFAULT_WORKFLOW: string = 'requirement';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Replace '<change>' placeholder in a prompt string with the actual change name.
 */
function interpolatePrompt(template: string, change: string): string {
  return template.replace(/<change>/g, change);
}

/**
 * Build a phase definition with prompts interpolated for the given change name.
 */
function buildPhaseDef(def: PhaseDefinition, change: string): PhaseDefinition {
  return {
    ...def,
    planner: def.planner
      ? {
          agent_type: def.planner.agent_type,
          prompt: interpolatePrompt(def.planner.prompt, change),
        }
      : null,
    evaluator: def.evaluator
      ? {
          agent_type: def.evaluator.agent_type,
          prompt: interpolatePrompt(def.evaluator.prompt, change),
        }
      : null,
  };
}

/**
 * Build a normal (non-error, non-done) response.
 */
function buildPhaseResponse(
  phase: PhaseDefinition,
  round: number,
  totalPhases: number,
  phaseIndex: number,
  change: string,
): PhaseNextResult {
  const resolved = buildPhaseDef(phase, change);
  return {
    done: false,
    error: null,
    message: null,
    next_phase: phase.id,
    phase_pattern: phase.pattern,
    planner: resolved.planner,
    evaluator: resolved.evaluator,
    auto_steps: phase.auto_steps,
    total_phases: totalPhases,
    phase_index: phaseIndex,
    round,
  };
}

/**
 * Build a "done" response — all phases complete.
 */
function buildDoneResponse(round: number, totalPhases: number): PhaseNextResult {
  return {
    done: true,
    error: null,
    message: 'All phases have passed evaluation. Ready for archiving.',
    next_phase: null,
    phase_pattern: null,
    planner: null,
    evaluator: null,
    auto_steps: [],
    total_phases: totalPhases,
    phase_index: totalPhases,
    round,
  };
}

/**
 * Build an error response.
 */
function buildErrorResponse(
  error: string,
  message: string,
  round: number,
  totalPhases: number,
): PhaseNextResult {
  return {
    done: false,
    error,
    message,
    next_phase: null,
    phase_pattern: null,
    planner: null,
    evaluator: null,
    auto_steps: [],
    total_phases: totalPhases,
    phase_index: 0,
    round,
  };
}

// ---------------------------------------------------------------------------
// Core Logic
// ---------------------------------------------------------------------------

/**
 * Compute the current round number from eval.json entries.
 * Round = total entries + 1 (next round to execute).
 */
function computeRound(entries: EvalEntry[]): number {
  return entries.length + 1;
}

/**
 * Check if a phase has passed (has a pass or skipped entry) that is NOT stale.
 *
 * Entries with `stale: true` are ignored.
 * Entries without a `stale` field are treated as `stale: false` (backward compatible).
 */
function hasPhasePassed(entries: EvalEntry[], phaseId: string): boolean {
  return entries.some(
    (e) => e.phase === phaseId && (e.verdict === 'pass' || e.skipped === true) && !e.stale,
  );
}

/**
 * Check if the latest entry has a non-null backtrack_to.
 * Returns the raw value (string, string[], or null).
 */
function getLatestBacktrackTarget(entries: EvalEntry[]): string | string[] | null {
  if (entries.length === 0) return null;
  const sorted = [...entries].sort(
    (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
  );
  const latest = sorted[0];
  return latest.backtrack_to && latest.backtrack_to !== '' ? latest.backtrack_to : null;
}

/**
 * Count attempts for a specific phase from eval entries.
 */
function countAttempts(entries: EvalEntry[], phaseId: string): number {
  return entries.filter((e) => e.phase === phaseId).length;
}

/**
 * Check round limit. Returns an error result if exceeded, or null to continue.
 */
function checkRoundLimit(round: number, totalPhases: number): ResolvePhaseNextResult | null {
  if (round <= 20) {
    return null;
  }
  return {
    result: buildErrorResponse(
      'round_limit_exceeded',
      '超过 20 轮限制，可能存在循环回溯。请检查 eval.json 中的 backtrack 记录，或手动清理后重试。',
      round,
      totalPhases,
    ),
  };
}

/**
 * Handle backtrack detection. Returns a result if backtrack was detected,
 * or null if no backtrack is needed.
 */
function handleBacktrack(
  entries: EvalEntry[],
  phaseTable: PhaseDefinition[],
  change: string,
  round: number,
  totalPhases: number,
): ResolvePhaseNextResult | null {
  // phase_log already handled stale marking when the backtrack entry was written.
  // phase_next only reads the backtrack_to to determine the next phase to return.
  const backtrackTarget = getLatestBacktrackTarget(entries);
  if (!backtrackTarget) {
    return null;
  }

  const targets = Array.isArray(backtrackTarget) ? backtrackTarget : [backtrackTarget];

  // Find the earliest target in phase table order
  let earliestTarget: string | null = null;
  let earliestIdx = Infinity;
  for (const target of targets) {
    const idx = phaseTable.findIndex((p) => p.id === target);
    if (idx !== -1 && idx < earliestIdx) {
      earliestIdx = idx;
      earliestTarget = target;
    }
  }

  if (!earliestTarget) {
    return {
      result: buildErrorResponse(
        'invalid_backtrack_target',
        `回溯目标 "${JSON.stringify(backtrackTarget)}" 不包含有效的 phase 标识符`,
        round,
        totalPhases,
      ),
    };
  }

  const targetPhase = phaseTable[earliestIdx];

  return {
    result: buildPhaseResponse(targetPhase, round, totalPhases, earliestIdx + 1, change),
  };
}

/**
 * Check retry logic for the current phase. Returns a result if retry handling
 * applies, or null to continue with normal flow.
 */
function checkRetryLimit(
  entries: EvalEntry[],
  nextPhaseDef: PhaseDefinition,
  round: number,
  totalPhases: number,
  phaseIndex: number,
  change: string,
): ResolvePhaseNextResult | null {
  const attempts = countAttempts(entries, nextPhaseDef.id);

  const phaseEntries = entries
    .filter((e) => e.phase === nextPhaseDef.id)
    .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());

  if (phaseEntries.length > 0) {
    const latest = phaseEntries[0];

    if (latest.verdict === 'fail') {
      if (attempts >= 5) {
        return {
          result: buildErrorResponse(
            'max_retries_exceeded',
            `Phase "${nextPhaseDef.id}" 已失败 ${attempts} 次，超过最大重试次数（5 次）。请检查 artifact 质量或手动干预后重试。`,
            round,
            totalPhases,
          ),
        };
      }
      return {
        result: buildPhaseResponse(nextPhaseDef, round, totalPhases, phaseIndex, change),
      };
    }
  }

  return null;
}

// ---------------------------------------------------------------------------
// Main exported function
// ---------------------------------------------------------------------------

/**
 * Pure logic: determine the next phase to execute from in-memory eval entries.
 *
 * This is the core orchestrator, extracted from filesystem I/O so it can be
 * unit-tested without disk access.
 *
 * IMPORTANT: This function is READ-ONLY. It does NOT modify the entries array.
 * All stale marking is handled by phase_log.
 *
 * Returns the next phase config or a done/error response.
 */
export interface ResolvePhaseNextOptions {
  change: string;
  entries: EvalEntry[];
  workflowType?: string;
}

export interface ResolvePhaseNextResult {
  result: PhaseNextResult;
}

export function resolvePhaseNext(opts: ResolvePhaseNextOptions): ResolvePhaseNextResult {
  const { change, entries, workflowType } = opts;
  const phaseTable = getPhaseTable(workflowType);
  const totalPhases = phaseTable.length;
  const round = computeRound(entries);

  const roundLimitResult = checkRoundLimit(round, totalPhases);
  if (roundLimitResult) {
    return roundLimitResult;
  }

  // -- Backtrack detection --
  const backtrackResult = handleBacktrack(entries, phaseTable, change, round, totalPhases);
  if (backtrackResult) {
    return backtrackResult;
  }

  const passedPhases = phaseTable.filter((p) => hasPhasePassed(entries, p.id));
  if (passedPhases.length >= totalPhases) {
    return { result: buildDoneResponse(round, totalPhases) };
  }

  const nextPhaseDef = phaseTable.find((p) => !hasPhasePassed(entries, p.id));
  if (!nextPhaseDef) {
    return { result: buildDoneResponse(round, totalPhases) };
  }

  const phaseIndex = phaseTable.indexOf(nextPhaseDef) + 1;
  const retryResult = checkRetryLimit(
    entries,
    nextPhaseDef,
    round,
    totalPhases,
    phaseIndex,
    change,
  );
  if (retryResult) {
    return retryResult;
  }

  return { result: buildPhaseResponse(nextPhaseDef, round, totalPhases, phaseIndex, change) };
}

/**
 * Full phase_next: reads eval.json from disk, resolves next phase.
 *
 * This function is READ-ONLY — it never writes to eval.json.
 * Stale marking and propagation are handled entirely by phase_log.
 *
 * Called by the MCP tool handler.
 */
export function runPhaseNext(options: PhaseNextOptions): PhaseNextResult {
  // -- Input validation --
  if (!options.change || options.change === '') {
    throw new Error('Missing required parameter: change');
  }

  const change = options.change;
  const workflowType = options.workflow_type || DEFAULT_WORKFLOW;

  // -- Read eval.json --
  const changeDir = getChangeDir(change);
  let entries: EvalEntry[];
  try {
    entries = readEvalJson(changeDir);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`Failed to read eval.json: ${msg}`);
  }

  const { result } = resolvePhaseNext({
    change,
    entries,
    workflowType,
  });

  return result;
}
