/**
 * phase_next MCP tool — server-side orchestration logic.
 *
 * Determines the next phase to execute in a PGE workflow based on eval.json entries.
 * Handles: initial run, normal progression, retry, backtrack, round limit,
 * mid-phase interruption, and skipped entries.
 *
 * KEY CHANGE: phase_next is now READ-ONLY. It never modifies eval.json.
 * Stale marking and propagation are handled by the standalone backtrack tool.
 *
 * Responses now include `last_result` — a snapshot of the latest eval entry
 * (phase, verdict, report, timestamp) so skills can make backtrack decisions
 * without re-reading eval.json.
 *
 * The workflow skill calls phase_next in a loop and executes the returned
 * planner/evaluator agents without any hardcoded phase knowledge.
 */

import * as fs from 'fs';

import { type z } from 'zod/v4';

import { getChangeDir } from '../lib/change';
import { getWorkflowType } from '../lib/change-config';
import { readEvalJson, type EvalEntry } from '../lib/eval-json';
import { getPhaseTable, type PhaseDefinition } from '../lib/workflow';
import { type phaseNextInputSchema, type phaseNextOutputSchema } from '../schemas';

export type PhaseNextOptions = z.input<typeof phaseNextInputSchema>;

export type PhaseNextResult = z.output<typeof phaseNextOutputSchema>;

const MAX_RETRY_TIMES = 5;

/** Process-local session anchors: (change, run_id) → entries.length at first sight. */
const sessionAnchors = new Map<string, number>();

function makeSessionKey(change: string, runId: string): string {
  return `${change}\0${runId}`;
}

function getOrCreateAnchor(change: string, runId: string, entriesLength: number): number {
  const key = makeSessionKey(change, runId);
  const existing = sessionAnchors.get(key);
  if (existing !== undefined) {
    return existing;
  }
  sessionAnchors.set(key, entriesLength);
  return entriesLength;
}

/**
 * Replace '<change>' and '<phase>' placeholders in a prompt string with
 * the actual change name and phase ID.
 */
function interpolatePrompt(template: string, change: string, phase?: string): string {
  let result = template.replace(/<change>/g, change);
  if (phase) {
    result = result.replace(/<phase>/g, phase);
  }
  return result;
}

/**
 * Compute phases that the evaluator can backtrack to from the given phase.
 * All phases that appear before the current phase in the phase table.
 */
function computeAllowedBacktrackPhases(
  currentPhaseId: string,
  phaseTable: PhaseDefinition[],
): { id: string; description: string }[] {
  const idx = phaseTable.findIndex((p) => p.id === currentPhaseId);
  if (idx <= 0) return [];
  return phaseTable.slice(0, idx + 1).map((p) => ({ id: p.id, description: p.description }));
}

/**
 * Build a phase definition with prompts interpolated for the given change name.
 * When backtrackReason is provided, appends a reason suffix to both planners
 * and evaluator prompts.
 */
function buildPhaseDef(
  def: PhaseDefinition,
  change: string,
  backtrackReason?: string | null,
): PhaseDefinition {
  // Build reason suffix when backtrack reason is provided
  const reasonSuffix = backtrackReason ? `\n\n⚠️ 回溯原因: ${backtrackReason}` : '';

  return {
    ...def,
    executor: def.executor
      ? {
          agent_type: def.executor.agent_type,
          prompt: interpolatePrompt(def.executor.prompt, change, def.id) + reasonSuffix,
        }
      : null,
    evaluator: def.evaluator
      ? {
          agent_type: def.evaluator.agent_type,
          prompt: interpolatePrompt(def.evaluator.prompt, change, def.id) + reasonSuffix,
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
  change: string,
  phaseTable: PhaseDefinition[],
  backtrackReason?: string | null,
  lastResult?: PhaseNextResult['last_result'],
): PhaseNextResult {
  const resolved = buildPhaseDef(phase, change, backtrackReason);
  const allowedBacktrack = computeAllowedBacktrackPhases(phase.id, phaseTable);
  const phaseIndex = phaseTable.findIndex((p) => p.id === phase.id) + 1;
  return {
    done: false,
    error: null,
    message: null,
    next_phase: phase.id,
    executor: resolved.executor,
    evaluator: resolved.evaluator,
    allowed_backtrack_phases: allowedBacktrack,
    last_result: lastResult ?? null,
    total_phases: totalPhases,
    phase_index: phaseIndex,
    round,
  };
}

/**
 * Build a "done" response — all phases complete.
 */
function buildDoneResponse(
  round: number,
  totalPhases: number,
  lastResult?: PhaseNextResult['last_result'],
): PhaseNextResult {
  return {
    done: true,
    error: null,
    message: 'All phases have passed evaluation. Ready for archiving.',
    next_phase: null,
    executor: null,
    evaluator: null,
    allowed_backtrack_phases: [],
    last_result: lastResult ?? null,
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
  lastResult?: PhaseNextResult['last_result'],
): PhaseNextResult {
  return {
    done: false,
    error,
    message,
    next_phase: null,
    executor: null,
    evaluator: null,
    allowed_backtrack_phases: [],
    last_result: lastResult ?? null,
    total_phases: totalPhases,
    phase_index: 0,
    round,
  };
}

// ---------------------------------------------------------------------------
// Core Logic
// ---------------------------------------------------------------------------

/**
 * Compute the current session round from window entries.
 * Round = window length + 1 (next round to execute in this session).
 */
function computeRound(window: EvalEntry[]): number {
  return window.length + 1;
}

/**
 * Check if a phase has passed (has a pass or skipped entry) that is NOT stale.
 *
 * Entries with `stale: true` are ignored.
 * Entries without a `stale` field are treated as `stale: false` (backward compatible).
 */
export function hasPhasePassed(entries: EvalEntry[], phaseId: string): boolean {
  return entries.some(
    (e) => e.phase === phaseId && (e.verdict === 'pass' || e.skipped === true) && !e.stale,
  );
}

/**
 * Check if the latest entry has a non-null backtrack_to.
 * Also returns the backtrack_reason if present.
 * Returns { target, reason } where reason is null if the field is missing.
 */
function getLatestBacktrackInfo(entries: EvalEntry[]): {
  target: string | string[] | null;
  reason: string | null;
} {
  if (entries.length === 0) return { target: null, reason: null };
  const sorted = [...entries].sort(
    (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
  );
  const latest = sorted[0];
  const target = latest.backtrack_to && latest.backtrack_to !== '' ? latest.backtrack_to : null;
  const reason =
    latest.backtrack_reason !== undefined && latest.backtrack_reason !== null
      ? latest.backtrack_reason
      : null;
  return { target, reason };
}

/**
 * Get the latest eval entry as a last_result snapshot.
 * Returns null if no entries exist.
 */
function getLatestEntry(entries: EvalEntry[]): PhaseNextResult['last_result'] {
  if (entries.length === 0) return null;
  const sorted = [...entries].sort(
    (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
  );
  const latest = sorted[0];
  return {
    phase: latest.phase,
    verdict: latest.verdict,
    report: latest.report,
    timestamp: latest.timestamp,
  };
}

/**
 * Check round limit. Returns an error result if exceeded, or null to continue.
 */
function checkRoundLimit(
  round: number,
  totalPhases: number,
  lastResult?: PhaseNextResult['last_result'],
): ResolvePhaseNextResult | null {
  if (round <= 20) {
    return null;
  }
  return {
    result: buildErrorResponse(
      'round_limit_exceeded',
      '超过 20 轮限制，可能存在循环回溯。请检查 eval.json 中的 backtrack 记录，或手动清理后重试。',
      round,
      totalPhases,
      lastResult,
    ),
  };
}

/**
 * Resolve the earliest valid backtrack target from entries.
 * Returns the target phase ID and its table index, or null if invalid.
 */
function resolveEarliestBacktrack(
  backtrackTarget: string | string[] | null,
  phaseTable: PhaseDefinition[],
): { target: string; idx: number } | null {
  if (!backtrackTarget) return null;
  const targets = Array.isArray(backtrackTarget) ? backtrackTarget : [backtrackTarget];
  let earliest: { target: string; idx: number } | null = null;
  for (const t of targets) {
    const idx = phaseTable.findIndex((p) => p.id === t);
    if (idx !== -1 && (earliest === null || idx < earliest.idx)) {
      earliest = { target: t, idx };
    }
  }
  return earliest;
}

/**
 * Handle backtrack detection. Returns a result if backtrack was detected,
 * or null if no backtrack is needed.
 * Preserved for backward compatibility with older eval.json entries that
 * have backtrack_to set by the old phase_log mechanism.
 */
function handleBacktrack(
  entries: EvalEntry[],
  phaseTable: PhaseDefinition[],
  change: string,
  round: number,
  lastResult?: PhaseNextResult['last_result'],
): ResolvePhaseNextResult | null {
  const { target: backtrackTarget, reason: backtrackReason } = getLatestBacktrackInfo(entries);
  const resolved = resolveEarliestBacktrack(backtrackTarget, phaseTable);

  if (!resolved) {
    if (!backtrackTarget) return null;
    return {
      result: buildErrorResponse(
        'invalid_backtrack_target',
        `回溯目标 "${JSON.stringify(backtrackTarget)}" 不包含有效的 phase 标识符`,
        round,
        phaseTable.length,
        lastResult,
      ),
    };
  }

  return {
    result: buildPhaseResponse(
      phaseTable[resolved.idx],
      round,
      phaseTable.length,
      change,
      phaseTable,
      backtrackReason,
      lastResult,
    ),
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
  lastResult?: PhaseNextResult['last_result'],
): ResolvePhaseNextResult | null {
  const attempts = entries.filter(
    (e) => e.phase === nextPhaseDef.id && e.verdict === 'fail',
  ).length;

  if (attempts >= MAX_RETRY_TIMES) {
    return {
      result: buildErrorResponse(
        'max_retries_exceeded',
        `Phase "${nextPhaseDef.id}" 失败超过最大重试次数（${MAX_RETRY_TIMES} 次）。请检查 artifact 质量或手动干预后重试。`,
        round,
        totalPhases,
        lastResult,
      ),
    };
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
 * All stale marking is handled by the standalone backtrack tool.
 *
 * Returns the next phase config or a done/error response.
 */
interface ResolvePhaseNextOptions {
  change: string;
  entries: EvalEntry[];
  workflowType: string;
  anchor: number;
}

interface ResolvePhaseNextResult {
  result: PhaseNextResult;
}

/**
 * Find the next unpassed phase and determine whether to retry or run fresh.
 */
function resolveDefaultPhase(
  entries: EvalEntry[],
  window: EvalEntry[],
  phaseTable: PhaseDefinition[],
  round: number,
  change: string,
  lastResult: PhaseNextResult['last_result'],
): ResolvePhaseNextResult {
  const totalPhases = phaseTable.length;
  const allPassed = phaseTable.every((p) => hasPhasePassed(entries, p.id));
  if (allPassed) {
    return { result: buildDoneResponse(round, totalPhases, lastResult) };
  }

  const nextPhaseIndex = phaseTable.findIndex((p) => !hasPhasePassed(entries, p.id));
  const nextPhaseDef = phaseTable[nextPhaseIndex];

  const retryResult = checkRetryLimit(window, nextPhaseDef, round, totalPhases, lastResult);
  if (retryResult) {
    return retryResult;
  }

  return {
    result: buildPhaseResponse(
      nextPhaseDef,
      round,
      phaseTable.length,
      change,
      phaseTable,
      undefined,
      lastResult,
    ),
  };
}

function resolvePhaseNext(opts: ResolvePhaseNextOptions): ResolvePhaseNextResult {
  const { change, entries, workflowType, anchor } = opts;
  const phaseTable = getPhaseTable(workflowType);
  const window = entries.slice(anchor);
  const round = computeRound(window);
  const lastResult = getLatestEntry(entries);

  const roundLimitResult = checkRoundLimit(round, phaseTable.length, lastResult);
  if (roundLimitResult) return roundLimitResult;

  const backtrackResult = handleBacktrack(entries, phaseTable, change, round, lastResult);
  if (backtrackResult) return backtrackResult;

  return resolveDefaultPhase(entries, window, phaseTable, round, change, lastResult);
}

/**
 * Full phase_next: reads eval.json from disk, resolves next phase.
 *
 * This function is READ-ONLY — it never writes to eval.json.
 * Stale marking and propagation are handled entirely by the standalone backtrack tool.
 *
 * Called by the MCP tool handler.
 */
export function runPhaseNext(options: PhaseNextOptions): PhaseNextResult {
  // -- Input validation --
  if (!options.change || options.change === '') {
    throw new Error('Missing required parameter: change');
  }

  const runId = typeof options.run_id === 'string' ? options.run_id.trim() : '';
  if (!runId) {
    return buildErrorResponse('missing_run_id', 'Missing required parameter: run_id', 0, 0, null);
  }

  const change = options.change;
  const changeDir = getChangeDir(change, options.project_root);
  if (!fs.existsSync(changeDir)) {
    throw new Error(`Change "${change}" does not exist: ${changeDir}`);
  }

  const workflowType = getWorkflowType(change);

  // -- Read eval.json --
  let entries: EvalEntry[];
  try {
    entries = readEvalJson(changeDir);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`Failed to read eval.json: ${msg}`);
  }

  const anchor = getOrCreateAnchor(change, runId, entries.length);

  const { result } = resolvePhaseNext({
    change,
    entries,
    workflowType,
    anchor,
  });

  return result;
}
