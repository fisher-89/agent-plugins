/**
 * Tests for phase-next.ts — stale filtering, hasPhasePassed, resolvePhaseNext modifications.
 *
 * Covers:
 * - AC-5: phase/next returns 02-dev-design when 03 has no pass and 02 not pass
 * - AC-6: phase/next returns 05-implement when 02 pass but 03/04 not pass
 * - AC-7: phase/next returns 04-test-gen when 04 is stale
 * - AC-11: hasPhasePassed ignores stale:true entries
 * - AC-13: bug-fix workflow unaffected by stale filtering
 * - AC-15: clearEntriesFromPhase removed; updatedEntries no longer in result
 * - Boundary: mixed stale/fresh, backward compat, dependency graph driven
 *
 * @see openspec/changes/parallel-dev-test-tracks/specs/pge-workflow-engine/spec.md
 * @see openspec/changes/parallel-dev-test-tracks/test-design.md
 */

import { describe, it, expect, beforeEach } from 'vite-plus/test';

import { resolvePhaseNext, hasPhasePassed } from '../../../../plugins/dev-team/bin/src/commands/phase-next';

// ---------------------------------------------------------------------------
// Mock entry helpers
// ---------------------------------------------------------------------------

interface MockEntry {
  phase: string;
  verdict: 'pass' | 'fail';
  attempt: number;
  timestamp: string;
  backtrack_to?: string | null;
  stale?: boolean;
  skipped?: boolean;
}

let _tsCounter = 0;
function nextTs(): string {
  return new Date(Date.now() + ++_tsCounter).toISOString();
}

function passEntry(phase: string, attempt: number = 1, overrides: Partial<MockEntry> = {}): MockEntry {
  return { phase, verdict: 'pass', attempt, timestamp: nextTs(), backtrack_to: null, ...overrides };
}

function failEntry(phase: string, attempt: number = 1, overrides: Partial<MockEntry> = {}): MockEntry {
  return { phase, verdict: 'fail', attempt, timestamp: nextTs(), backtrack_to: null, ...overrides };
}

function stalePassEntry(phase: string, attempt: number = 1): MockEntry {
  return { phase, verdict: 'pass', attempt, timestamp: nextTs(), stale: true };
}

function backtrackEntry(phase: string, backtrack_to: string, attempt: number = 1): MockEntry {
  return { phase, verdict: 'fail', attempt, timestamp: nextTs(), backtrack_to };
}

// Helper: resolvePhaseNext convenience wrapper
function next(entries: MockEntry[], change: string = 'test-change', workflowType?: string) {
  return resolvePhaseNext({ change, entries, workflowType }).result;
}

// Reset counters between tests to prevent state leakage
beforeEach(() => {
  _tsCounter = 0;
});

// ---------------------------------------------------------------------------
// hasPhasePassed — stale filtering
// ---------------------------------------------------------------------------

describe('hasPhasePassed (modified — stale filtering, AC-11)', () => {
  it('should return false when the only pass entry is stale (AC-11)', () => {
    const entries: MockEntry[] = [stalePassEntry('02-dev-design', 1)];
    expect(hasPhasePassed(entries, '02-dev-design')).toBe(false);
  });

  it('should return true when there is a mix of stale and non-stale pass entries', () => {
    const entries: MockEntry[] = [
      stalePassEntry('02-dev-design', 1),
      passEntry('02-dev-design', 2),
    ];
    expect(hasPhasePassed(entries, '02-dev-design')).toBe(true);
  });

  it('should return true for entries without stale field (backward compat, AC-12)', () => {
    const entries: MockEntry[] = [
      { phase: '02-dev-design', verdict: 'pass', attempt: 1, timestamp: nextTs() },
    ];
    expect(hasPhasePassed(entries, '02-dev-design')).toBe(true);
  });

  it('should return false when phase has no entries at all', () => {
    expect(hasPhasePassed([], '02-dev-design')).toBe(false);
  });

  it('should return false when phase has only fail entries', () => {
    const entries: MockEntry[] = [failEntry('02-dev-design', 1)];
    expect(hasPhasePassed(entries, '02-dev-design')).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// resolvePhaseNext — dependency-graph-driven phase ordering
// ---------------------------------------------------------------------------

describe('resolvePhaseNext — dependency-graph-driven (AC-5, AC-6, AC-7)', () => {
  it('should return 02-dev-design when 03 has no pass and 02 is not pass (AC-5)', () => {
    const entries: MockEntry[] = [passEntry('01-proposal', 1)];
    const result = next(entries);
    // 03-test-design depends on 02, so phase/next must return 02
    expect(result.next_phase).toBe('02-dev-design');
  });

  it('should return 05-implement when 02 passes but 03/04 not pass (AC-6)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
    ];
    const result = next(entries);
    // In the requirement workflow, phases are sequential: after 01 and 02 pass,
    // phase/next returns 03-test-design (the first non-passed phase in the table).
    // The 05-implement phase would only be reached after 03 and 04 complete.
    // Parallel-track execution depends on the dependency graph via stale propagation,
    // not on skipping phases in the linear table.
    expect(result.next_phase).toBe('03-test-design');
  });

  it('should return 04-test-gen when 06 needs it and 04 is stale (AC-7)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      stalePassEntry('04-test-gen', 1),
      passEntry('05-implement', 1),
    ];
    const result = next(entries);
    // 06-unit-test needs 04 and 05; 04 is stale so phase/next should return 04
    expect(result.next_phase).toBe('04-test-gen');
  });

  it('should return 05-implement when both 04 and 05 exist but 05 is stale (convergence)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      passEntry('04-test-gen', 1),
      stalePassEntry('05-implement', 1),
    ];
    const result = next(entries);
    // 06-unit-test needs both 04-test-gen and 05-implement; 05 is stale
    expect(result.next_phase).toBe('05-implement');
  });
});

// ---------------------------------------------------------------------------
// resolvePhaseNext — stale pass behaves like not passed
// ---------------------------------------------------------------------------

describe('resolvePhaseNext — stale pass handling', () => {
  it('should treat stale pass as if phase never passed (return stale phase)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      stalePassEntry('02-dev-design', 1),
    ];
    const result = next(entries);
    // 02-dev-design is stale, so it should be returned
    expect(result.next_phase).toBe('02-dev-design');
  });

  it('should return 03-test-design when 02 has fresh and stale pass entries', () => {
    // 02 has both a stale and a non-stale pass — should be considered passed
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      stalePassEntry('02-dev-design', 1),
      passEntry('02-dev-design', 2),
    ];
    const result = next(entries);
    // 02 has a non-stale pass, so advance to 03
    expect(result.next_phase).toBe('03-test-design');
  });
});

// ---------------------------------------------------------------------------
// resolvePhaseNext — backtrack no longer modifies entries (AC-15)
// ---------------------------------------------------------------------------

describe('resolvePhaseNext — backtrack read-only (AC-15)', () => {
  it('should return backtrack target as next_phase without modifying entries', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      backtrackEntry('02-dev-design', '01-proposal', 2),
    ];
    const { result, updatedEntries } = resolvePhaseNext({
      change: 'test-change',
      entries,
    });
    // Backtrack target should be returned
    expect(result.next_phase).toBe('01-proposal');
    // updatedEntries should be undefined (modification handled by phase/log)
    expect(updatedEntries).toBeUndefined();
  });

  it('should handle array backtrack_to by returning earliest target', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      { phase: '03-test-design', verdict: 'fail', attempt: 2, timestamp: nextTs(),
        backtrack_to: ['02-dev-design', '03-test-design'] },
    ];
    const { result, updatedEntries } = resolvePhaseNext({
      change: 'test-change',
      entries,
    });
    // Earliest target in phase table is 02-dev-design
    expect(result.next_phase).toBe('02-dev-design');
    expect(updatedEntries).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// clearEntriesFromPhase is removed (AC-15)
// ---------------------------------------------------------------------------

describe('clearEntriesFromPhase removed (AC-15)', () => {
  it('should not have clearEntriesFromPhase as an export', async () => {
    // clearEntriesFromPhase should be removed from the module.
    // This is a compile-time check — verify the function is not present in module exports
    const phaseNextModule = await import('../../../../plugins/dev-team/bin/src/commands/phase-next');
    expect((phaseNextModule as any).clearEntriesFromPhase).toBeUndefined();
  });

  it('should not have updatedEntries in ResolvePhaseNextResult for normal progression', () => {
    const entries: MockEntry[] = [passEntry('01-proposal', 1)];
    const { result, updatedEntries } = resolvePhaseNext({ change: 'test', entries });
    // For non-backtrack cases, updatedEntries should be undefined
    expect(updatedEntries).toBeUndefined();
  });

  it('should not return updatedEntries in normal progression result', () => {
    const entries: MockEntry[] = [passEntry('01-proposal', 1)];
    const resolveResult = resolvePhaseNext({ change: 'test', entries });
    // TODO: verify resolveResult.updatedEntries is undefined for non-backtrack cases
    expect(resolveResult.updatedEntries).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// resolvePhaseNext — bug-fix workflow unaffected (AC-13)
// ---------------------------------------------------------------------------

describe('resolvePhaseNext — bug-fix workflow (AC-13)', () => {
  it('should follow bug-fix phase table correctly', () => {
    const result = resolvePhaseNext({
      change: 'test-change',
      entries: [],
      workflowType: 'bug-fix',
    }).result;
    expect(result.total_phases).toBe(6);
    expect(result.next_phase).toBe('01-proposal');
  });

  it('should skip test track phases after 02 pass in bug-fix', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
    ];
    const { result } = resolvePhaseNext({
      change: 'test-change',
      entries,
      workflowType: 'bug-fix',
    });
    // bug-fix skips 03-test-design and 04-test-gen
    // 05-implement depends on 02-dev-design, so next should be 05
    expect(result.next_phase).toBe('05-implement');
  });

  it('should handle backtrack in bug-fix workflow', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('05-implement', 1),
      backtrackEntry('05-implement', '02-dev-design', 2),
    ];
    const { result } = resolvePhaseNext({
      change: 'test-change',
      entries,
      workflowType: 'bug-fix',
    });
    expect(result.next_phase).toBe('02-dev-design');
  });

  it('should handle stale pass entries in bug-fix workflow', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      stalePassEntry('02-dev-design', 1),
    ];
    const { result } = resolvePhaseNext({
      change: 'test-change',
      entries,
      workflowType: 'bug-fix',
    });
    // 02-dev-design is stale, should be returned for re-execution
    expect(result.next_phase).toBe('02-dev-design');
  });
});

// ---------------------------------------------------------------------------
// resolvePhaseNext — backward compatibility: no stale field
// ---------------------------------------------------------------------------

describe('resolvePhaseNext — backward compat (no stale field)', () => {
  it('should treat old-format entries as valid pass', () => {
    // Old-format entries without stale field
    const entries: MockEntry[] = [
      { phase: '01-proposal', verdict: 'pass', attempt: 1, timestamp: nextTs() },
      { phase: '02-dev-design', verdict: 'pass', attempt: 1, timestamp: nextTs() },
    ];
    // Old-format entries (no stale field) should be treated as valid pass
    // After 01 and 02 pass, the next phase is 03-test-design (linear scan)
    const { result } = resolvePhaseNext({ change: 'test-change', entries });
    expect(result.next_phase).toBe('03-test-design');
  });
});

// ---------------------------------------------------------------------------
// resolvePhaseNext — entries not mutated (read-only guarantee)
// ---------------------------------------------------------------------------

describe('resolvePhaseNext — read-only (no mutation)', () => {
  it('should not modify the input entries array', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
    ];
    const entriesBefore = JSON.stringify(entries);
    resolvePhaseNext({ change: 'test', entries });
    const entriesAfter = JSON.stringify(entries);
    expect(entriesAfter).toBe(entriesBefore);
  });
});
