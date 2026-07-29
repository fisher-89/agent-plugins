import { type z } from 'zod/v4';

import { getChangeDir } from '../lib/change';
import { getWorkflowType } from '../lib/change-config';
import { readEvalJson, markPhaseStale, writeEvalJson, type EvalEntry } from '../lib/eval-json';
import { getPhaseTable, type PhaseDefinition } from '../lib/workflow';
import { type backtrackInputSchema, type backtrackOutputSchema } from '../schemas';

type BacktrackOptions = z.input<typeof backtrackInputSchema>;

type BacktrackResult = z.output<typeof backtrackOutputSchema>;

/**
 * Validate that the phase and backtrack_to exist in the phase table and that
 * backtrack_to appears before phase. Returns the phase table indices.
 */
function validatePhaseTarget(
  phase: string,
  backtrackTo: string,
  workflowType: string,
): { phaseIdx: number; phaseTable: PhaseDefinition[] } {
  const phaseTable = getPhaseTable(workflowType);
  const phaseIdx = phaseTable.findIndex((p) => p.id === phase);
  if (phaseIdx === -1) {
    throw new Error(`工作流 "${workflowType}" 不包含 phase "${phase}"`);
  }
  const targetIdx = phaseTable.findIndex((p) => p.id === backtrackTo);
  if (targetIdx === -1) {
    throw new Error(`工作流 "${workflowType}" 不包含 phase "${backtrackTo}"`);
  }
  if (targetIdx > phaseIdx) {
    throw new Error(`无效的回溯目标 phase: "${backtrackTo}"。不支持回溯到未来 phase。`);
  }
  return { phaseIdx, phaseTable };
}

/**
 * Find the latest entry for a given phase from entries array.
 * Throws if no entry exists.
 */
function findLatestPhaseEntry(entries: EvalEntry[], phase: string): EvalEntry {
  const phaseEntries = entries
    .filter((e) => e.phase === phase)
    .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime());
  if (phaseEntries.length === 0) {
    throw new Error(
      `Phase "${phase}" 没有 eval.json 条目，无法设置回溯。请先执行该 phase 并记录评估结果。`,
    );
  }
  return phaseEntries[0];
}

/**
 * Read eval.json from the change directory, returning entries.
 */
function readEvalEntries(changeDir: string): EvalEntry[] {
  try {
    return readEvalJson(changeDir);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`读取 eval.json 失败: ${msg}`);
  }
}

/**
 * Persist the modified entries array to eval.json.
 */
function persistEvalJson(changeDir: string, entries: EvalEntry[]): void {
  try {
    writeEvalJson(changeDir, entries);
  } catch (e: unknown) {
    const msg = e instanceof Error ? e.message : String(e);
    throw new Error(`写入 eval.json 失败: ${msg}`);
  }
}

/**
 * Core logic for backtrack: set backtrack target and reason for a phase entry.
 *
 * This is the ONLY function that should modify backtrack state in eval.json.
 */
export function runBacktrack(options: BacktrackOptions): BacktrackResult {
  const { change, phase, backtrack_to, backtrack_reason, project_root } = options;

  const changeDir = getChangeDir(change, project_root);
  const workflowType = getWorkflowType(change);

  // Validate phase and target positions in the workflow
  validatePhaseTarget(phase, backtrack_to, workflowType);

  // Read, modify, mark stale, persist
  const entries = readEvalEntries(changeDir);
  const latestEntry = findLatestPhaseEntry(entries, phase);

  latestEntry.backtrack_to = backtrack_to;
  latestEntry.backtrack_reason = backtrack_reason;

  markPhaseStale(entries, backtrack_to, workflowType);
  persistEvalJson(changeDir, entries);

  return { modified: true, phase, target: backtrack_to };
}
