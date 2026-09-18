import { resolveChangeDir } from '../lib/change';
import { getWorkflowType } from '../lib/change-config';
import { computeAttempt, readEvalJson } from '../lib/eval-json';
import { getPhaseTable } from '../lib/workflow';
import { writeActivePhase } from '../modules/workflow';
import { type PhaseStartOptions, type PhaseStartResult } from '../schemas';

/**
 * Core logic for phase_start: explicitly open a change's running phase
 * state.
 *
 * Thin adapter — the `active_phase` read/write lives in the workflow module
 * (`writeActivePhase`); this layer only validates inputs and derives the
 * attempt:
 * - the change dir and `workflow.json` must exist and be valid
 *   (`getWorkflowType` throws otherwise — a missing or malformed file is an
 *   error, not a default);
 * - `phase` MUST belong to the change `workflow_type`'s phase table (an
 *   invalid phase throws and `workflow.json` stays byte-for-byte unchanged);
 * - attempt = `computeAttempt(readEvalJson(changeDir), phase)` (existing
 *   eval entries for the phase + 1 — the same rule `phase_log` uses when no
 *   explicit attempt is given), so a retry naturally increments;
 * - the write is last-wins: re-entry (retry / protocol-duplicate call)
 *   overwrites the running state with a fresh `start_at` and the same
 *   derived attempt.
 */
export function runPhaseStart(options: PhaseStartOptions): PhaseStartResult {
  const changeDir = resolveChangeDir(options.change, options.project_root);

  const workflowType = getWorkflowType(options.change);
  const phaseTable = getPhaseTable(workflowType);
  if (!phaseTable.some((def) => def.id === options.phase)) {
    throw new Error(
      `phase "${options.phase}" 不属于 workflow_type "${workflowType}" 的 phase 表 (change: ${options.change})，active_phase 未写入。`,
    );
  }

  const attempt = computeAttempt(readEvalJson(changeDir), options.phase);
  const start_at = new Date().toISOString();

  writeActivePhase(changeDir, { phase: options.phase, attempt, start_at });

  return { started: true, phase: options.phase, attempt, start_at };
}
