// sweep-phase.ts — UserPromptSubmit hook protocol adapter: before each new
// user message, reclaim a leftover `active_phase` on the session-bound change
// (a phase interrupted mid-turn — the user broke out of a running phase).
// The leftover state is archived into `workflow.json.interrupted[]` and the
// running state is cleared; NO eval entry is written (a user interruption
// must not burn retry quota). Purely fail-open: any failure is a stderr
// diagnostic and exit 0 — this hook MUST NOT block the user message.

import { readFileSync } from 'node:fs';

import { resolveChangeDir } from '../lib/change';
import { getProjectDir } from '../lib/project-root';
import { lookupChange } from '../lib/session-registry';
import { interruptActivePhase } from '../modules/workflow';
import { isPlainObject } from '../utils';

/**
 * Sweep one UserPromptSubmit event: `session_id → change` binding lookup,
 * then archive the bound change's leftover `active_phase` when present. An
 * unbound session (or a missing binding) is a no-op.
 */
function sweepPhaseEvent(stdinRaw: string): void {
  if (!stdinRaw || !stdinRaw.trim()) return;

  let parsed: unknown;
  try {
    parsed = JSON.parse(stdinRaw);
  } catch {
    return;
  }
  if (!isPlainObject(parsed)) return;

  const sessionId = typeof parsed.session_id === 'string' ? parsed.session_id : undefined;
  if (!sessionId) return;

  const projectRoot = getProjectDir();
  const change = lookupChange(projectRoot, sessionId);
  if (!change) return;

  const changeDir = resolveChangeDir(change, projectRoot);
  if (interruptActivePhase(changeDir)) {
    process.stderr.write(
      `sweep-phase: change "${change}" 的遗留 active_phase 已归档至 interrupted\n`,
    );
  }
}

/**
 * sweep-phase subcommand entry point (UserPromptSubmit sweeper).
 *
 * Any error — unreadable stdin, unbound session, legacy change, archive
 * failure — is written to stderr only and the process exits 0: the sweep
 * MUST NOT block the user message.
 */
export function runSweepPhase(): void {
  let stdinRaw: string;
  try {
    stdinRaw = readFileSync(0, 'utf-8');
  } catch {
    return;
  }
  try {
    sweepPhaseEvent(stdinRaw);
  } catch (e: unknown) {
    const message = e instanceof Error ? e.message : String(e);
    process.stderr.write(`sweep-phase: ${message}\n`);
  }
}
