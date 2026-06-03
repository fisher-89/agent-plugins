/**
 * Unit tests for eval-json.ts — stale marking, propagation, and gate-check.
 *
 * Covers:
 * - AC-8: markPhaseStale marks target stale and propagates downstream
 * - AC-12: old entries without stale field are treated as stale: false
 * - AC-14: transitive closure propagation
 * - Boundary scenarios for markPhaseStale, propagateStale, checkGate
 *
 * @see openspec/changes/parallel-dev-test-tracks/specs/pipeline-backtrack/spec.md
 * @see openspec/changes/parallel-dev-test-tracks/test-design.md
 */

import { describe, it, expect, beforeEach } from 'vite-plus/test';

import {
  markPhaseStale,
  propagateStale,
  checkGate,
  buildEntry,
} from '../../../../plugins/dev-team/bin/src/lib/eval-json';

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

function staleFailEntry(phase: string, attempt: number = 1): MockEntry {
  return { phase, verdict: 'fail', attempt, timestamp: nextTs(), stale: true };
}

// Reset counters between tests to prevent state leakage
beforeEach(() => {
  _tsCounter = 0;
});

// ---------------------------------------------------------------------------
// markPhaseStale
// ---------------------------------------------------------------------------

describe('markPhaseStale', () => {
  it('should mark the latest pass entry as stale for the target phase (AC-8)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('02-dev-design', 2),
      passEntry('03-test-design', 1),
    ];
    markPhaseStale(entries, '02-dev-design');

    const phase2Entries = entries.filter((e) => e.phase === '02-dev-design');
    // Latest pass entry (attempt 2) should be stale
    const latest = phase2Entries.find((e) => e.attempt === 2);
    // TODO: verify latest.stale === true
    expect(latest?.stale).toBe(true);
  });

  it('should propagate downstream after marking (AC-8)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      passEntry('04-test-gen', 1),
      passEntry('05-implement', 1),
    ];
    markPhaseStale(entries, '02-dev-design');

    // 02-dev-design should be stale
    // TODO: verify entries for 02 and downstream are stale
    expect(entries.find((e) => e.phase === '02-dev-design')?.stale).toBe(true);
  });

  it('should do nothing when target phase has no pass entry (no-op)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      failEntry('02-dev-design', 1),
    ];
    // 02-dev-design has only fail entries — no-op expected
    expect(() => markPhaseStale(entries, '02-dev-design')).not.toThrow();
    // No entry should be modified
    expect(entries.find((e) => e.phase === '02-dev-design')?.stale).toBeUndefined();
  });

  it('should do nothing when target phase has no entries at all (no-op)', () => {
    const entries: MockEntry[] = [passEntry('01-proposal', 1)];
    expect(() => markPhaseStale(entries, '02-dev-design')).not.toThrow();
  });

  it('should mark the latest pass entry when multiple pass entries exist (boundary)', () => {
    const entries: MockEntry[] = [
      passEntry('02-dev-design', 1, { timestamp: '2026-01-01T00:00:00.000Z' }),
      passEntry('02-dev-design', 2, { timestamp: '2026-02-01T00:00:00.000Z' }),
      passEntry('02-dev-design', 3, { timestamp: '2026-03-01T00:00:00.000Z' }),
    ];
    markPhaseStale(entries, '02-dev-design');
    const latestEntry = entries.find((e) => e.attempt === 3);
    // TODO: verify only attempt 3 is marked stale
    expect(latestEntry?.stale).toBe(true);
  });

  it('should not throw when entries array is empty', () => {
    expect(() => markPhaseStale([], '02-dev-design')).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// propagateStale — transitive downstream stale marking
// ---------------------------------------------------------------------------

describe('propagateStale', () => {
  it('should mark all downstream dependents stale from 02-dev-design (AC-14)', () => {
    // Full 9-phase dataset
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      passEntry('04-test-gen', 1),
      passEntry('05-implement', 1),
      passEntry('06-unit-test', 1),
      passEntry('07-code-review', 1),
      passEntry('08-integration-test', 1),
      passEntry('09-acceptance', 1),
    ];
    propagateStale(entries, '02-dev-design');

    // 01 should NOT be stale (upstream unaffected)
    expect(entries.find((e) => e.phase === '01-proposal')?.stale).toBeUndefined();

    // 02 should NOT be stale (propagateStale only marks dependents, not source)
    expect(entries.find((e) => e.phase === '02-dev-design')?.stale).toBeUndefined();

    // Direct dependents: 03, 05, 09 should be stale
    expect(entries.find((e) => e.phase === '03-test-design')?.stale).toBe(true);
    expect(entries.find((e) => e.phase === '05-implement')?.stale).toBe(true);
    expect(entries.find((e) => e.phase === '09-acceptance')?.stale).toBe(true);

    // Transitive: 04, 06, 07, 08 should be stale
    expect(entries.find((e) => e.phase === '04-test-gen')?.stale).toBe(true);
    expect(entries.find((e) => e.phase === '06-unit-test')?.stale).toBe(true);
    expect(entries.find((e) => e.phase === '07-code-review')?.stale).toBe(true);
    expect(entries.find((e) => e.phase === '08-integration-test')?.stale).toBe(true);
  });

  it('should propagate from 03-test-design along test track only', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      passEntry('04-test-gen', 1),
      passEntry('05-implement', 1),
      passEntry('06-unit-test', 1),
    ];
    propagateStale(entries, '03-test-design');

    // 01, 02, 05 should NOT be stale
    expect(entries.find((e) => e.phase === '01-proposal')?.stale).toBeUndefined();
    expect(entries.find((e) => e.phase === '02-dev-design')?.stale).toBeUndefined();
    expect(entries.find((e) => e.phase === '05-implement')?.stale).toBeUndefined();

    // 03 is the source — propagateStale does NOT mark source phase itself
    expect(entries.find((e) => e.phase === '03-test-design')?.stale).toBeUndefined();
    // 04, 06 should be stale (downstream test track)
    expect(entries.find((e) => e.phase === '04-test-gen')?.stale).toBe(true);
    expect(entries.find((e) => e.phase === '06-unit-test')?.stale).toBe(true);
  });

  it('should be no-op from 06-unit-test (leaf node, no dependents)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('06-unit-test', 1),
    ];
    propagateStale(entries, '06-unit-test');
    expect(entries.find((e) => e.phase === '06-unit-test')?.stale).toBeUndefined();
  });

  it('should be no-op from 09-acceptance (last phase, no dependents)', () => {
    const entries: MockEntry[] = [
      passEntry('09-acceptance', 1),
    ];
    propagateStale(entries, '09-acceptance');
    expect(entries.find((e) => e.phase === '09-acceptance')?.stale).toBeUndefined();
  });

  it('should skip phases with no entries and continue propagation', () => {
    // 04-test-gen has no entries, but 06-unit-test does
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      // 04-test-gen: no entries
      passEntry('05-implement', 1),
      passEntry('06-unit-test', 1),
    ];
    propagateStale(entries, '03-test-design');
    // 06-unit-test should be marked stale (transitive via 04 -> 06)
    // 04 has no entries so it's skipped, but propagation continues to 06
    expect(entries.find((e) => e.phase === '06-unit-test')?.stale).toBe(true);
    // 05-implement is NOT a dependent of 03, so it should not be stale
    expect(entries.find((e) => e.phase === '05-implement')?.stale).toBeUndefined();
  });

  it('should not throw when entries array is empty', () => {
    expect(() => propagateStale([], '02-dev-design')).not.toThrow();
  });

  it('should use visited-set to prevent re-traversal (defensive)', () => {
    // This is a structural test: calling propagateStale multiple times on overlapping
    // targets should not cause infinite loops or exceptions
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      passEntry('05-implement', 1),
      passEntry('06-unit-test', 1),
    ];
    // Call twice — should not throw
    propagateStale(entries, '02-dev-design');
    expect(() => propagateStale(entries, '03-test-design')).not.toThrow();
  });

  it('should propagate with bug-fix workflow_type', () => {
    // For bug-fix, dependents of 02-dev-design should only include 05-implement
    // (not 03-test-design)
    const entries: MockEntry[] = [
      passEntry('02-dev-design', 1),
      passEntry('05-implement', 1),
    ];
    propagateStale(entries, '02-dev-design', 'bug-fix');
    // 05-implement is the only dependent of 02-dev-design in bug-fix workflow
    expect(entries.find((e) => e.phase === '05-implement')?.stale).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// checkGate — modified to filter stale entries
// ---------------------------------------------------------------------------

describe('checkGate (modified — stale filtering)', () => {
  it('should pass when all prerequisites have non-stale pass entries', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
    ];
    const result = checkGate(entries, ['01-proposal', '02-dev-design']);
    expect(result.passed).toBe(true);
    expect(result.missing).toEqual([]);
  });

  it('should fail when a prerequisite has only stale pass entries', () => {
    const entries: MockEntry[] = [
      stalePassEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
    ];
    const result = checkGate(entries, ['01-proposal']);
    expect(result.passed).toBe(false);
    expect(result.missing).toContain('01-proposal');
  });

  it('should treat entries without stale field as valid (backward compat, AC-12)', () => {
    // Old-format entries: no stale field
    const entries: MockEntry[] = [
      { phase: '01-proposal', verdict: 'pass', attempt: 1, timestamp: nextTs() },
      { phase: '02-dev-design', verdict: 'pass', attempt: 1, timestamp: nextTs() },
    ];
    const result = checkGate(entries, ['01-proposal', '02-dev-design']);
    expect(result.passed).toBe(true);
    expect(result.missing).toEqual([]);
  });

  it('should treat explicit stale:false same as missing stale field (AC-12)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1, { stale: false }),
      passEntry('02-dev-design', 1),
    ];
    const result = checkGate(entries, ['01-proposal']);
    expect(result.passed).toBe(true);
  });

  it('should pass when prerequisites array is empty', () => {
    const result = checkGate([], []);
    expect(result.passed).toBe(true);
    expect(result.missing).toEqual([]);
  });

  it('should fail when a prerequisite has only fail entries', () => {
    const entries: MockEntry[] = [
      failEntry('01-proposal', 1),
    ];
    const result = checkGate(entries, ['01-proposal']);
    expect(result.passed).toBe(false);
    expect(result.missing).toContain('01-proposal');
  });

  it('should fail when a prerequisite has no entries at all', () => {
    const entries: MockEntry[] = [passEntry('02-dev-design', 1)];
    const result = checkGate(entries, ['01-proposal']);
    expect(result.passed).toBe(false);
    expect(result.missing).toContain('01-proposal');
  });

  it('should report multiple missing prerequisites', () => {
    const entries: MockEntry[] = [passEntry('04-test-gen', 1)];
    const result = checkGate(entries, ['01-proposal', '02-dev-design', '03-test-design']);
    expect(result.passed).toBe(false);
    expect(result.missing).toContain('01-proposal');
    expect(result.missing).toContain('02-dev-design');
    expect(result.missing).toContain('03-test-design');
  });
});

// ---------------------------------------------------------------------------
// Integration: markPhaseStale + propagateStale combined
// ---------------------------------------------------------------------------

describe('markPhaseStale + propagateStale integration', () => {
  it('should mark all downstream phases through transitive closure (AC-14)', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      passEntry('04-test-gen', 1),
      passEntry('05-implement', 1),
      passEntry('06-unit-test', 1),
      passEntry('07-code-review', 1),
      passEntry('08-integration-test', 1),
      passEntry('09-acceptance', 1),
    ];
    markPhaseStale(entries, '02-dev-design');

    // 01 should not be stale
    expect(entries.find((e) => e.phase === '01-proposal')?.stale).toBeUndefined();

    // All phases 02-09 should be stale
    const downstream = ['02-dev-design', '03-test-design', '04-test-gen', '05-implement',
      '06-unit-test', '07-code-review', '08-integration-test', '09-acceptance'];
    for (const phase of downstream) {
      // TODO: verify each phase entry is stale
      const entry = entries.find((e) => e.phase === phase);
      expect(entry?.stale).toBe(true);
    }
  });

  it('should not mark entries of phases that are not downstream', () => {
    const entries: MockEntry[] = [
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
      passEntry('04-test-gen', 1),
    ];
    markPhaseStale(entries, '03-test-design');
    // 01 and 02 should NOT be stale
    expect(entries.find((e) => e.phase === '01-proposal')?.stale).toBeUndefined();
    expect(entries.find((e) => e.phase === '02-dev-design')?.stale).toBeUndefined();
    // 03 and 04 should be stale
    expect(entries.find((e) => e.phase === '03-test-design')?.stale).toBe(true);
    expect(entries.find((e) => e.phase === '04-test-gen')?.stale).toBe(true);
  });
});
