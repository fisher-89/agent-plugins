/**
 * Ordered list of PGE workflow phases.
 * Index in this array defines the workflow order.
 * 9 phases: DESIGN (P->E), EXEC (G->E/AUTO), EXEC (Executor->Evaluator), EVAL-ONLY (E)
 */
export const PHASES: readonly string[] = [
  "01-requirements",
  "02-dev-design",
  "03-test-design",
  "04-test-gen",
  "05-implement",
  "06-unit-test",
  "07-code-review",
  "08-integration-test",
  "09-acceptance",
] as const;

/**
 * Return the index of a phase in the PHASES array.
 * Returns -1 if the phase is not found.
 */
export function getPhaseIndex(phase: string): number {
  return PHASES.indexOf(phase);
}

/**
 * Return the list of phases that come before the given phase.
 * - If phase is the first phase, returns an empty array.
 * - If phase is not in PHASES, returns an empty array (fault-tolerant).
 */
export function getPriorPhases(phase: string): string[] {
  const idx = getPhaseIndex(phase);
  if (idx <= 0) return [];
  return PHASES.slice(0, idx);
}
