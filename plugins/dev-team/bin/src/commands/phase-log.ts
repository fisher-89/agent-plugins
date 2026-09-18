import type z from 'zod/v4';

import { resolveChangeDir } from '../lib/change';
import {
  readEvalJson,
  validateVerdict,
  validateReportLength,
  buildEntry,
  computeAttempt,
  appendEntry,
  type EvalEntry,
} from '../lib/eval-json';
import { readActivePhase, clearActivePhase } from '../modules/workflow';
import { type phaseLogInputSchema, type phaseLogOutputSchema } from '../schemas';

type PhaseLogOptions = z.input<typeof phaseLogInputSchema>;

type PhaseLogResult = z.output<typeof phaseLogOutputSchema>;

/**
 * Auto-calculate verdict from checklist items when not explicitly provided.
 * All pass → "pass", any fail → "fail".
 */
function resolveVerdict(checklist: { pass: boolean }[]): 'pass' | 'fail' {
  return checklist.every((i) => i.pass) ? 'pass' : 'fail';
}

/** Read the eval history, wrapping failures with the canonical message. */
function readEntries(changeDir: string): EvalEntry[] {
  try {
    return readEvalJson(changeDir);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`读取 workflow.json 失败: ${msg}`);
  }
}

/**
 * Stamp timing from the running state before the entry lands; a missing or
 * non-matching `active_phase` leaves the entry without `start_at`.
 */
function resolveStartAt(changeDir: string, phase: string): string | undefined {
  const active = readActivePhase(changeDir);
  return active && active.phase === phase ? active.start_at : undefined;
}

/**
 * Clear the running state after the entry landed (gate 2 of the recorder
 * closes). A failure is swallowed to stderr: the entry is already on disk
 * and a retry would duplicate it — a leftover running state is reclaimed by
 * the next `phase_start` (last-wins) or the UserPromptSubmit sweep.
 */
function clearRunningState(changeDir: string): void {
  try {
    clearActivePhase(changeDir);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    process.stderr.write(
      `phase-log: 清理 active_phase 失败（条目已落盘，遗留状态由下次 phase_start 或 sweep 回收）: ${msg}\n`,
    );
  }
}

/**
 * Core logic for phase-log: validate, build entry, and persist to the
 * evaluation history in `workflow.json` (`eval` field).
 *
 * Key responsibilities:
 * - Validates verdict and report length
 * - Stamps `start_at` from the change's running state (`active_phase`)
 *   when it matches this phase — read BEFORE the entry lands; without a
 *   match the entry carries no `start_at` and `active_phase` stays as-is
 * - Builds the entry and appends it to `workflow.json.eval` via `appendEntry`
 *   (which also deletes a leftover legacy `eval.json`)
 * - AFTER the entry landed, clears the matching `active_phase` (see
 *   `clearRunningState` for the failure policy)
 * - Does NOT handle backtrack (backtrack state is managed by standalone backtrack tool)
 * - Does NOT perform gate-check (gate logic is entirely owned by phase_next).
 * - MUST NOT write `eval.json` directly.
 *
 * `workflow.json` MUST already exist (`change_create` is its only creator): a
 * missing or malformed file fails inside `appendEntry` — no directory and no
 * file is ever created here — and that inner error already carries the absolute
 * path plus the `change_create` hint, surfaced verbatim through the
 * `写入 workflow.json 失败` wrapper below.
 */
export function runPhaseLog(options: PhaseLogOptions): PhaseLogResult {
  const verdict = resolveVerdict(options.checklist);
  validateVerdict(verdict, options.skipped === true);
  validateReportLength(options.report);

  const changeDir = resolveChangeDir(options.change, options.project_root);
  const attempt = computeAttempt(readEntries(changeDir), options.phase, options.attempt);
  const startAt = resolveStartAt(changeDir, options.phase);

  const entry = buildEntry({
    phase: options.phase,
    verdict,
    report: options.report,
    checklist: options.checklist,
    attempt,
    skipped: options.skipped === true ? true : undefined,
    start_at: startAt,
  });

  try {
    appendEntry(changeDir, entry);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`写入 workflow.json 失败: ${msg}`);
  }

  if (startAt !== undefined) {
    clearRunningState(changeDir);
  }

  return { written: true, phase: options.phase, attempt };
}
