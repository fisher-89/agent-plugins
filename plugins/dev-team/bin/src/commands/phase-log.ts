import { getChangeDir } from '../lib/change';
import {
  readEvalJson,
  validateVerdict,
  validateReportLength,
  validateItemsJson,
  buildEntry,
  computeAttempt,
  markPhaseStale,
  writeEvalJson,
  appendEntry,
} from '../lib/eval-json';
import { getPhaseIndex } from '../lib/workflow';

export interface PhaseLogOptions {
  change: string;
  phase: string;
  verdict: string;
  report: string;
  items: string;
  attempt?: string;
  backtrackTo?: string | string[];
  skipped?: boolean;
  findings?: string;
}

export interface PhaseLogResult {
  written: boolean;
  phase: string;
  attempt: number;
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
  const REQUIRED_ARGS = ['change', 'phase', 'verdict', 'report', 'items'] as const;
  const missing = REQUIRED_ARGS.filter((r) => !options[r] || options[r] === '');
  if (missing.length > 0) {
    throw new Error(`缺少必填参数: --${missing.join(', --')}`);
  }

  validateVerdict(options.verdict, options.skipped === true);
  validateReportLength(options.report);

  const itemsList = validateItemsJson(options.items);

  const changeDir = getChangeDir(options.change);

  let entries: any[];
  try {
    entries = readEvalJson(changeDir);
  } catch (e: any) {
    throw new Error(`读取 eval.json 失败: ${e.message}`);
  }

  // -- Handle backtrack: mark stale targets BEFORE writing new entry --
  let modifiedByBacktrack = false;
  if (options.backtrackTo != null && options.backtrackTo !== '') {
    const targets = Array.isArray(options.backtrackTo)
      ? options.backtrackTo
      : [options.backtrackTo];

    // Validate each target is a known phase ID
    for (const target of targets) {
      const idx = getPhaseIndex(target);
      if (idx === -1) {
        throw new Error(`无效的回溯目标 phase: "${target}"。请使用有效的 phase 标识符。`);
      }
    }

    // Mark stale for each target (handles propagation internally)
    for (const target of targets) {
      markPhaseStale(entries, target);
    }
    modifiedByBacktrack = true;
  }

  const explicitAttempt = options.attempt ? parseInt(options.attempt, 10) : undefined;
  const attempt = computeAttempt(entries, options.phase, explicitAttempt);

  const entry = buildEntry({
    phase: options.phase,
    verdict: options.verdict,
    report: options.report,
    items: itemsList,
    attempt,
    backtrack_to: options.backtrackTo || null,
    skipped: options.skipped === true ? true : undefined,
    findings: options.findings || undefined,
  });

  try {
    if (modifiedByBacktrack) {
      // If we modified entries (stale marking), push the new entry and write full array
      entries.push(entry);
      writeEvalJson(changeDir, entries);
    } else {
      // Normal path: no stale modifications, just append
      appendEntry(changeDir, entry);
    }
  } catch (e: any) {
    throw new Error(`写入 eval.json 失败: ${e.message}`);
  }

  return { written: true, phase: options.phase, attempt };
}
