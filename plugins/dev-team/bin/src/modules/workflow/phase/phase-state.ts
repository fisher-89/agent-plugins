/**
 * phase-state.ts — `active_phase` / `interrupted` running-state operations
 * for `workflow.json` (module layer). Read, write (last-wins), clear and
 * interrupt-archive; read/write discipline matches `../doc-io.ts` (file must
 * already exist, schema-validated load, unknown keys preserved, 2-space
 * indentation + trailing newline).
 *
 * Zero lib / commands dependencies on purpose: phase-table membership and
 * attempt derivation are the command layer's concern (`phase_start`); this
 * module only owns the persisted running state.
 */

import { type z } from 'zod/v4';

import { activePhaseSchema, interruptedEntrySchema } from '../../../schemas';
import { loadWorkflowDoc, saveWorkflowDoc } from '../doc-io';

/** Running phase state persisted at `workflow.json.active_phase`. */
export type ActivePhase = z.infer<typeof activePhaseSchema>;

/**
 * Archived interrupted attempt persisted at `workflow.json.interrupted[]`.
 */
type InterruptedPhase = z.infer<typeof interruptedEntrySchema>;

/**
 * Read the running phase state. A missing field or explicit `null` means no
 * running state. A missing/invalid `workflow.json` throws (the caller's
 * swallow policy decides what that means — the hook layers exit 0).
 */
export function readActivePhase(changeDir: string): ActivePhase | null {
  const doc = loadWorkflowDoc(changeDir);
  const value = doc.active_phase;
  if (value === undefined || value === null) {
    return null;
  }
  // Schema-parse instead of a cast: `loadWorkflowDoc` already validated the
  // document, so this never throws — it only narrows the raw value.
  return activePhaseSchema.parse(value);
}

/**
 * Write the running phase state (last-wins overwrite on re-entry); unknown
 * keys and every other field are preserved.
 */
export function writeActivePhase(changeDir: string, active: ActivePhase): void {
  const doc = loadWorkflowDoc(changeDir);
  doc.active_phase = active;
  saveWorkflowDoc(changeDir, doc);
}

/**
 * Clear the running phase state (set `active_phase` to `null`) without
 * archiving — used by `phase_log` after the entry has landed. `interrupted`
 * is untouched.
 */
export function clearActivePhase(changeDir: string): void {
  const doc = loadWorkflowDoc(changeDir);
  doc.active_phase = null;
  saveWorkflowDoc(changeDir, doc);
}

/**
 * Archive a leftover running state: when `active_phase` is present, append
 * `{phase, attempt, start_at, end_at}` to `interrupted[]` (end_at defaults
 * to now) and clear the running state. Never writes eval entries (the sweep
 * must not burn retry quota). Returns whether an archive happened.
 */
export function interruptActivePhase(changeDir: string, endedAt?: string): boolean {
  const doc = loadWorkflowDoc(changeDir);
  const value = doc.active_phase;
  if (value === undefined || value === null) {
    return false;
  }
  const active = activePhaseSchema.parse(value);
  const entry: InterruptedPhase = {
    ...active,
    end_at: endedAt ?? new Date().toISOString(),
  };
  // Schema-parse instead of a cast (same rationale as `readActivePhase`).
  const interrupted = interruptedEntrySchema
    .array()
    .parse(Array.isArray(doc.interrupted) ? doc.interrupted : []);
  interrupted.push(entry);
  doc.interrupted = interrupted;
  doc.active_phase = null;
  saveWorkflowDoc(changeDir, doc);
  return true;
}
