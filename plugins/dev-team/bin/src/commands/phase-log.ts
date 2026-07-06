import type z from 'zod/v4';

import { getChangeDir } from '../lib/change';
import { getWorkflowType } from '../lib/change-config';
import {
  readEvalJson,
  validateVerdict,
  validateReportLength,
  buildEntry,
  computeAttempt,
  markPhaseStale,
  writeEvalJson,
  appendEntry,
  type EvalEntry,
} from '../lib/eval-json';
import { getPhaseTable } from '../lib/workflow';
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
 * Handle backtrack stale marking when backtrack_to is set.
 * Returns true if entries were modified, false otherwise.
 */
function handleBacktrackMarking(entries: EvalEntry[], options: PhaseLogOptions): boolean {
  if (options.backtrack_to == null || options.backtrack_to === '') {
    return false;
  }

  const targets = Array.isArray(options.backtrack_to)
    ? options.backtrack_to
    : [options.backtrack_to];

  const workflowType = getWorkflowType(options.change);
  const phaseTable = getPhaseTable(workflowType);
  const currentIdx = phaseTable.findIndex((p) => p.id === options.phase);

  for (const target of targets) {
    const targetIdx = phaseTable.findIndex((p) => p.id === target);
    if (targetIdx === -1) {
      throw new Error(`工作流 ${workflowType} 不包含 phase '${target}'`);
    }
    if (currentIdx === -1 || targetIdx >= currentIdx) {
      throw new Error(`无效的回溯目标 phase: "${target}"。不支持回溯到当前或未来phase。`);
    }
  }

  // Mark stale for each target (handles propagation internally)
  for (const target of targets) {
    markPhaseStale(entries, target, workflowType);
  }
  return true;
}

/**
 * Core logic for phase-log: validate, handle backtrack stale marking,
 * build entry, and persist to eval.json.
 *
 * Key responsibilities:
 * - When `backtrack_to` is set (string or array), marks the target phase(s)
 *   stale AND propagates downstream BEFORE writing the new entry.
 * - Pass entries do NOT trigger any stale marking.
 * - Does NOT perform gate-check (gate logic is entirely owned by phase_next).
 */
export function runPhaseLog(options: PhaseLogOptions): PhaseLogResult {
  const verdict = resolveVerdict(options.checklist);
  validateVerdict(verdict, options.skipped === true);
  validateReportLength(options.report);

  const changeDir = getChangeDir(options.change);

  let entries: EvalEntry[];
  try {
    entries = readEvalJson(changeDir);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`读取 eval.json 失败: ${msg}`);
  }

  const modifiedByBacktrack = handleBacktrackMarking(entries, options);

  const attempt = computeAttempt(entries, options.phase, options.attempt);

  const entry = buildEntry({
    phase: options.phase,
    verdict,
    report: options.report,
    checklist: options.checklist,
    attempt,
    backtrack_to: options.backtrack_to ?? null,
    skipped: options.skipped === true ? true : undefined,
  });

  try {
    if (modifiedByBacktrack) {
      // If we modified entries (stale marking), push the new entry and write full array
      entry.stale = true;
      entries.push(entry);
      writeEvalJson(changeDir, entries);
    } else {
      // Normal path: no stale modifications, just append
      appendEntry(changeDir, entry);
    }
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`写入 eval.json 失败: ${msg}`);
  }

  return { written: true, phase: options.phase, attempt };
}
