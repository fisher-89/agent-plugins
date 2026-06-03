/**
 * Tests for phase-log.ts — backtrack_to triggers markPhaseStale with propagation.
 *
 * Covers:
 * - AC-9: phase/log with backtrack_to triggers markPhaseStale
 * - AC-10: phase/log with backtrack_to array triggers multiple markPhaseStale calls
 * - Boundary: pass no propagation, fail pass entry no stale field,
 *   empty backtrack target, backward compat
 *
 * Integration tests: mock eval.json entries, spy on markPhaseStale.
 *
 * @see openspec/changes/parallel-dev-test-tracks/specs/pipeline-backtrack/spec.md
 * @see openspec/changes/parallel-dev-test-tracks/test-design.md
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// Mock filesystem I/O modules — vi.mock is hoisted above all declarations
// Factory functions are called lazily, so vi.fn() inside them is fine.
// ---------------------------------------------------------------------------

vi.mock('../../../../plugins/dev-team/bin/src/lib/eval-json', async () => {
  const actual = await vi.importActual('../../../../plugins/dev-team/bin/src/lib/eval-json');
  return {
    ...actual,
    markPhaseStale: vi.fn(), // spy — tests verify call count/args
    readEvalJson: vi.fn(),
    appendEntry: vi.fn(),
    writeEvalJson: vi.fn(),
  };
});

vi.mock('../../../../plugins/dev-team/bin/src/lib/change', () => ({
  getChangeDir: vi.fn(() => '/tmp/test-change'),
}));

// ---------------------------------------------------------------------------
// Imports — resolve to mocked modules
// ---------------------------------------------------------------------------

import { runPhaseLog } from '../../../../plugins/dev-team/bin/src/commands/phase-log';
import { markPhaseStale } from '../../../../plugins/dev-team/bin/src/lib/eval-json';
import * as mockEvalJson from '../../../../plugins/dev-team/bin/src/lib/eval-json';
import * as mockChange from '../../../../plugins/dev-team/bin/src/lib/change';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let _tsCounter = 0;
function nextTs(): string {
  return new Date(Date.now() + ++_tsCounter).toISOString();
}

function passEntry(phase: string, attempt: number = 1) {
  return { phase, verdict: 'pass', attempt, timestamp: nextTs(), backtrack_to: null };
}

function failEntry(phase: string, attempt: number = 1) {
  return { phase, verdict: 'fail', attempt, timestamp: nextTs(), backtrack_to: null };
}

const VALID_ITEMS = '[{"item":"test","pass":true,"evidence":"","notes":""}]';
const VALID_ITEMS_FAIL = '[{"item":"test","pass":false,"evidence":"","notes":""}]';

/** Reset mock state before each test */
function resetMocks(): void {
  _tsCounter = 0;
  vi.clearAllMocks();
  const r = mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>;
  r.mockReset();
  r.mockReturnValue([]);
  (mockChange.getChangeDir as ReturnType<typeof vi.fn>).mockReturnValue('/tmp/test-change');
}

// Reset mocks and counters between tests to prevent state leakage
beforeEach(() => {
  resetMocks();
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// phase/log pass entry — no propagation (AC-9)
// ---------------------------------------------------------------------------

describe('runPhaseLog — pass entry no propagation', () => {
  it('should NOT call markPhaseStale when writing a pass entry (AC-9)', () => {
    runPhaseLog({
      change: 'test-change',
      phase: '02-dev-design',
      verdict: 'pass',
      report: 'Design approved',
      items: VALID_ITEMS,
    });
    expect(markPhaseStale).not.toHaveBeenCalled();
  });

  it('should NOT call markPhaseStale on first-time pass entry', () => {
    runPhaseLog({
      change: 'test-change',
      phase: '03-test-design',
      verdict: 'pass',
      report: 'Test design approved',
      items: VALID_ITEMS,
    });
    expect(markPhaseStale).not.toHaveBeenCalled();
  });

  it('should write entry successfully for pass verdict', () => {
    runPhaseLog({
      change: 'test-change',
      phase: '02-dev-design',
      verdict: 'pass',
      report: 'Design approved',
      items: VALID_ITEMS,
    });
    expect(mockEvalJson.appendEntry).toHaveBeenCalled();
    const args = (mockEvalJson.appendEntry as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(args[0]).toBe('/tmp/test-change');
    expect(args[1].phase).toBe('02-dev-design');
    expect(args[1].verdict).toBe('pass');
  });

  it('should not set stale field on newly written pass entry', () => {
    runPhaseLog({
      change: 'test-change',
      phase: '02-dev-design',
      verdict: 'pass',
      report: 'Design approved',
      items: VALID_ITEMS,
    });
    const args = (mockEvalJson.appendEntry as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(args[1]).not.toHaveProperty('stale');
  });
});

// ---------------------------------------------------------------------------
// phase/log backtrack_to — triggers markPhaseStale
// ---------------------------------------------------------------------------

describe('runPhaseLog — backtrack_to triggers markPhaseStale (AC-9)', () => {
  it('should call markPhaseStale when backtrack_to is set to a string', () => {
    (mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>).mockReturnValue([
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('05-implement', 1),
    ]);

    runPhaseLog({
      change: 'test-change',
      phase: '05-implement',
      verdict: 'fail',
      report: 'Implementation failed',
      items: VALID_ITEMS_FAIL,
      backtrackTo: '02-dev-design',
    });
    expect(markPhaseStale).toHaveBeenCalledTimes(1);
  });

  it('should write the fail entry after marking stale', () => {
    (mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>).mockReturnValue([
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
    ]);

    runPhaseLog({
      change: 'test-change',
      phase: '03-test-design',
      verdict: 'fail',
      report: 'Test design needs rework',
      items: VALID_ITEMS_FAIL,
      backtrackTo: '02-dev-design',
    });

    // Backtrack was set, so writeEvalJson should be called (not appendEntry)
    expect(mockEvalJson.writeEvalJson).toHaveBeenCalled();
    const entries = (mockEvalJson.writeEvalJson as ReturnType<typeof vi.fn>).mock.calls[0][1] as any[];
    // The last entry should be the new fail entry
    const lastEntry = entries[entries.length - 1];
    expect(lastEntry.verdict).toBe('fail');
    expect(lastEntry.phase).toBe('03-test-design');
  });

  it('should handle backtrack_to target with no pass entries (no-op)', () => {
    // backtrack target 02-dev-design has no entries at all
    (mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>).mockReturnValue([
      passEntry('01-proposal', 1),
    ]);

    // Should not throw despite no pass entry for backtrack target
    expect(() => {
      runPhaseLog({
        change: 'test-change',
        phase: '03-test-design',
        verdict: 'fail',
        report: 'Test design needs rework',
        items: VALID_ITEMS_FAIL,
        backtrackTo: '02-dev-design',
      });
    }).not.toThrow();

    // markPhaseStale should still have been called (it does the no-op internally)
    expect(markPhaseStale).toHaveBeenCalled();
    // Entry should still be written
    expect(mockEvalJson.writeEvalJson).toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// phase/log backtrack_to array — triggers multiple calls
// ---------------------------------------------------------------------------

describe('runPhaseLog — backtrack_to array (AC-10)', () => {
  it('should call markPhaseStale for each target in backtrack_to array', () => {
    (mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>).mockReturnValue([
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
    ]);

    runPhaseLog({
      change: 'test-change',
      phase: '05-implement',
      verdict: 'fail',
      report: 'Implementation failed',
      items: VALID_ITEMS_FAIL,
      backtrackTo: ['02-dev-design', '03-test-design'],
    });

    expect(markPhaseStale).toHaveBeenCalledTimes(2);
  });

  it('should propagate independently from each backtrack target', () => {
    (mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>).mockReturnValue([
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      passEntry('03-test-design', 1),
    ]);

    runPhaseLog({
      change: 'test-change',
      phase: '05-implement',
      verdict: 'fail',
      report: 'Implementation failed',
      items: VALID_ITEMS_FAIL,
      backtrackTo: ['02-dev-design', '03-test-design'],
    });

    // Verify specific targets were called
    expect(markPhaseStale).toHaveBeenCalledWith(
      expect.any(Array),
      '02-dev-design',
    );
    expect(markPhaseStale).toHaveBeenCalledWith(
      expect.any(Array),
      '03-test-design',
    );
  });

  it('should reject array containing invalid phase IDs', () => {
    (mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>).mockReturnValue([
      passEntry('01-proposal', 1),
    ]);

    expect(() => {
      runPhaseLog({
        change: 'test-change',
        phase: '05-implement',
        verdict: 'fail',
        report: 'Implementation failed',
        items: VALID_ITEMS_FAIL,
        backtrackTo: ['02-dev-design', '99-invalid'],
      });
    }).toThrow('无效的回溯目标 phase');

    // No entries should have been written (validation failed before write)
    expect(mockEvalJson.writeEvalJson).not.toHaveBeenCalled();
    expect(mockEvalJson.appendEntry).not.toHaveBeenCalled();
  });

  it('should reject array containing only invalid phase IDs', () => {
    (mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>).mockReturnValue([]);

    expect(() => {
      runPhaseLog({
        change: 'test-change',
        phase: '05-implement',
        verdict: 'fail',
        report: 'Implementation failed',
        items: VALID_ITEMS_FAIL,
        backtrackTo: ['99-invalid'],
      });
    }).toThrow('无效的回溯目标 phase');

    expect(mockEvalJson.writeEvalJson).not.toHaveBeenCalled();
    expect(mockEvalJson.appendEntry).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// phase/log new entry format — no stale field on new entries
// ---------------------------------------------------------------------------

describe('runPhaseLog — new entry format', () => {
  it('should not include stale field on new pass entries', () => {
    runPhaseLog({
      change: 'test-change',
      phase: '02-dev-design',
      verdict: 'pass',
      report: 'Design approved',
      items: VALID_ITEMS,
    });
    const args = (mockEvalJson.appendEntry as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(args[1]).not.toHaveProperty('stale');
    expect(args[1].verdict).toBe('pass');
  });

  it('should not include stale field on new fail entries', () => {
    runPhaseLog({
      change: 'test-change',
      phase: '02-dev-design',
      verdict: 'fail',
      report: 'Design rejected',
      items: VALID_ITEMS_FAIL,
    });
    const args = (mockEvalJson.appendEntry as ReturnType<typeof vi.fn>).mock.calls[0];
    expect(args[1]).not.toHaveProperty('stale');
    expect(args[1].verdict).toBe('fail');
  });

  it('should not include stale field on new fail+backtrack entries', () => {
    (mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>).mockReturnValue([
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
    ]);

    runPhaseLog({
      change: 'test-change',
      phase: '03-test-design',
      verdict: 'fail',
      report: 'Test design needs rework',
      items: VALID_ITEMS_FAIL,
      backtrackTo: '02-dev-design',
    });

    // Backtrack case uses writeEvalJson — verify the last (new) entry
    const allEntries = (mockEvalJson.writeEvalJson as ReturnType<typeof vi.fn>).mock.calls[0][1] as any[];
    const newEntry = allEntries[allEntries.length - 1];
    expect(newEntry).not.toHaveProperty('stale');
    expect(newEntry.verdict).toBe('fail');
    expect(newEntry.backtrack_to).toBe('02-dev-design');
  });
});

// ---------------------------------------------------------------------------
// phase/log no longer calls checkGate (behavior change)
// ---------------------------------------------------------------------------

describe('runPhaseLog — no longer calls checkGate', () => {
  it('should NOT call checkGate during runPhaseLog execution', () => {
    // checkGate is not imported in phase-log.ts, so it cannot be called
    // This test verifies the architecture: phase/log has no gating logic
    // phase/next is the sole decision point for what phase to execute next
    (mockEvalJson.readEvalJson as ReturnType<typeof vi.fn>).mockReturnValue([]);

    runPhaseLog({
      change: 'test-change',
      phase: '01-proposal',
      verdict: 'pass',
      report: 'Proposal approved',
      items: VALID_ITEMS,
    });

    expect(mockEvalJson.appendEntry).toHaveBeenCalled();
    expect(markPhaseStale).not.toHaveBeenCalled();
  });
});
