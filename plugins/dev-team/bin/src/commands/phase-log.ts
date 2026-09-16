import type z from 'zod/v4';

import { getChangeDir } from '../lib/change';
import {
  readEvalJson,
  validateVerdict,
  validateReportLength,
  buildEntry,
  computeAttempt,
  appendEntry,
  type EvalEntry,
} from '../lib/eval-json';
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

/**
 * Core logic for phase-log: validate, build entry, and persist to the
 * evaluation history in `workflow.json` (`eval` field).
 *
 * Key responsibilities:
 * - Validates verdict and report length
 * - Builds the entry and appends it to `workflow.json.eval` via `appendEntry`
 *   (which also deletes a leftover legacy `eval.json`)
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

  const changeDir = getChangeDir(options.change, options.project_root);

  let entries: EvalEntry[];
  try {
    entries = readEvalJson(changeDir);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`读取 workflow.json 失败: ${msg}`);
  }

  const attempt = computeAttempt(entries, options.phase, options.attempt);

  const entry = buildEntry({
    phase: options.phase,
    verdict,
    report: options.report,
    checklist: options.checklist,
    attempt,
    skipped: options.skipped === true ? true : undefined,
  });

  try {
    appendEntry(changeDir, entry);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`写入 workflow.json 失败: ${msg}`);
  }

  return { written: true, phase: options.phase, attempt };
}
