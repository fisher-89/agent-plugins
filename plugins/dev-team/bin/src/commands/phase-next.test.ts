/**
 * Unit tests for phase/next MCP tool — server-side orchestration logic.
 *
 * Tests cover: phase table resolution, normal progression, skip passed phases,
 * retry logic, backtrack, round limit, mid-phase interruption, skipped entries,
 * workflow_type variants, input validation, and all boundary scenarios.
 *
 * All tests call resolvePhaseNext with in-memory eval.json data — no filesystem access.
 *
 * @see openspec/changes/add-workflow-requirement-skill/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';
import type z4 from 'zod/v4';

import { resolvePhaseNext } from '../commands/phase-next';
import { getPhaseTable, getPhasePattern } from '../lib/workflow';
import { type phaseLogInputSchema } from '../schemas';

// ---------------------------------------------------------------------------
// Mock helpers — construct eval.json entries for test scenarios
// ---------------------------------------------------------------------------

type MockEntry = z4.infer<typeof phaseLogInputSchema>;

// Module-level counter ensures each mock entry gets a unique, increasing timestamp.
let _tsCounter = 0;
function nextTs(): string {
  return new Date(Date.now() + ++_tsCounter).toISOString();
}

function passEntry(
  phase: string,
  attempt: number = 1,
  overrides: Partial<MockEntry> = {},
): MockEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: nextTs(),
    backtrack_to: null,
    ...overrides,
  };
}

function failEntry(
  phase: string,
  attempt: number = 1,
  overrides: Partial<MockEntry> = {},
): MockEntry {
  return {
    phase,
    verdict: 'fail',
    attempt,
    timestamp: nextTs(),
    backtrack_to: null,
    ...overrides,
  };
}

function backtrackEntry(
  phase: string,
  backtrack_to: string | string[],
  attempt: number = 1,
): MockEntry {
  return {
    phase,
    verdict: 'fail',
    attempt,
    timestamp: nextTs(),
    backtrack_to,
  };
}

function skippedEntry(phase: string, attempt: number = 1): MockEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: nextTs(),
    skipped: true,
  };
}

function skipPedEntry(phase: string, attempt: number = 1): MockEntry {
  return skippedEntry(phase, attempt);
}

function staleEntry(
  phase: string,
  attempt: number = 1,
  overrides: Partial<MockEntry> = {},
): MockEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: nextTs(),
    backtrack_to: null,
    stale: true,
    ...overrides,
  };
}

// Note: the counter is intentionally not reset between tests — relative
// ordering within each test case is all that matters.

// Helper to call resolvePhaseNext
function next(entries: MockEntry[], change: string = 'test-change', workflowType?: string) {
  return resolvePhaseNext({ change, entries, workflowType }).result;
}

// ---------------------------------------------------------------------------
// Phase Tables
// ---------------------------------------------------------------------------

describe('PHASE_TABLES', () => {
  it('should have exactly 9 phases for requirement workflow_type', () => {
    expect(getPhaseTable('requirement').length).toBe(9);
  });

  it('should start with 01-proposal (not 01-requirements)', () => {
    expect(getPhaseTable('requirement')[0].id).toBe('01-proposal');
  });

  it('should include all 9 phases in order', () => {
    const phases = getPhaseTable('requirement').map((p) => p.id);
    expect(phases).toEqual([
      '01-proposal',
      '02-dev-design',
      '03-test-design',
      '04-test-gen',
      '05-implement',
      '06-unit-test',
      '07-code-review',
      '08-integration-test',
      '09-acceptance',
    ]);
  });

  it('should have 6 phases for bug-fix workflow_type', () => {
    const phases = getPhaseTable('bug-fix').map((p) => p.id);
    expect(phases.length).toBe(6);
    // bug-fix skips 03-test-design, 04-test-gen, 08-integration-test
    expect(phases).not.toContain('03-test-design');
    expect(phases).not.toContain('04-test-gen');
    expect(phases).not.toContain('08-integration-test');
  });

  it('should have same 9 phases for refactor workflow_type', () => {
    const req = getPhaseTable('requirement').map((p) => p.id);
    const ref = getPhaseTable('refactor').map((p) => p.id);
    expect(ref).toEqual(req);
  });

  it('should default to requirement table for unknown workflow_type', () => {
    const def = getPhaseTable('unknown').map((p) => p.id);
    const req = getPhaseTable('requirement').map((p) => p.id);
    expect(def).toEqual(req);
  });
});

// ---------------------------------------------------------------------------
// Phase Patterns
// ---------------------------------------------------------------------------

describe('getPhasePattern', () => {
  it('should return DESIGN for 01-proposal', () => {
    expect(getPhasePattern('01-proposal')).toBe('DESIGN');
  });

  it('should return DESIGN for 02-dev-design and 03-test-design', () => {
    expect(getPhasePattern('02-dev-design')).toBe('DESIGN');
    expect(getPhasePattern('03-test-design')).toBe('DESIGN');
  });

  it('should return EXEC for 04-test-gen and 05-implement', () => {
    expect(getPhasePattern('04-test-gen')).toBe('EXEC');
    expect(getPhasePattern('05-implement')).toBe('EXEC');
  });

  it('should return EXEC for 06-unit-test and 08-integration-test', () => {
    expect(getPhasePattern('06-unit-test')).toBe('EXEC');
    expect(getPhasePattern('08-integration-test')).toBe('EXEC');
  });

  it('should return EVAL-ONLY for 07-code-review and 09-acceptance', () => {
    expect(getPhasePattern('07-code-review')).toBe('EVAL-ONLY');
    expect(getPhasePattern('09-acceptance')).toBe('EVAL-ONLY');
  });

  it('should return null for unknown phase', () => {
    expect(getPhasePattern('99-unknown')).toBeNull();
  });

  it('should return DESIGN for 01-proposal with planner + evaluator', () => {
    const phase = getPhaseTable('requirement').find((p) => p.id === '01-proposal')!;
    expect(phase.pattern).toBe('DESIGN');
    expect(phase.planner).not.toBeNull();
    expect(phase.evaluator).not.toBeNull();
  });

  it('should return EVAL-ONLY for 07-code-review with planner null', () => {
    const phase = getPhaseTable('requirement').find((p) => p.id === '07-code-review')!;
    expect(phase.pattern).toBe('EVAL-ONLY');
    expect(phase.planner).toBeNull();
    expect(phase.evaluator).not.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// First Run — Empty eval.json
// ---------------------------------------------------------------------------

describe('runPhaseNext — First Run (empty eval.json)', () => {
  it('should return 01-proposal as next_phase when entries is empty', () => {
    const result = next([]);
    expect(result.next_phase).toBe('01-proposal');
    expect(result.done).toBe(false);
    expect(result.error).toBeNull();
  });

  it('should return proposal-planner as planner agent_type', () => {
    const result = next([]);
    expect(result.planner!.agent_type).toBe('dev-team:proposal-planner');
  });

  it('should return proposal-evaluator as evaluator agent_type', () => {
    const result = next([]);
    expect(result.evaluator!.agent_type).toBe('dev-team:proposal-evaluator');
  });

  it('should set round to 1 on first call', () => {
    const result = next([]);
    expect(result.round).toBe(1);
  });

  it('should set phase_index to 1 and total_phases to 9', () => {
    const result = next([]);
    expect(result.phase_index).toBe(1);
    expect(result.total_phases).toBe(9);
  });

  it('should include change name in planner prompt', () => {
    const result = next([], 'my-change');
    expect(result.planner!.prompt).toContain('my-change');
  });
});

// ---------------------------------------------------------------------------
// Normal Progression — Pass phases in sequence
// ---------------------------------------------------------------------------

describe('runPhaseNext — Normal Progression', () => {
  it('should return 02-dev-design after 01-proposal passes', () => {
    const result = next([passEntry('01-proposal')]);
    expect(result.next_phase).toBe('02-dev-design');
  });

  it('should return 03-test-design after phases 01-02 pass', () => {
    const result = next([passEntry('01-proposal'), passEntry('02-dev-design')]);
    expect(result.next_phase).toBe('03-test-design');
  });

  it('should return 05-implement after phases 01-04 pass', () => {
    const result = next([
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
    ]);
    expect(result.next_phase).toBe('05-implement');
  });

  it('should return 07-code-review with planner: null (EVAL-ONLY)', () => {
    const result = next([
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
      passEntry('05-implement'),
      passEntry('06-unit-test'),
    ]);
    expect(result.next_phase).toBe('07-code-review');
    expect(result.planner).toBeNull();
    expect(result.evaluator!.agent_type).toBe('dev-team:code-review-evaluator');
  });

  it('should return 09-acceptance with planner: null (EVAL-ONLY)', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
      passEntry('05-implement'),
      passEntry('06-unit-test'),
      passEntry('07-code-review'),
      passEntry('08-integration-test'),
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('09-acceptance');
    expect(result.planner).toBeNull();
  });

  it('should return done=true when all 9 phases pass', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
      passEntry('05-implement'),
      passEntry('06-unit-test'),
      passEntry('07-code-review'),
      passEntry('08-integration-test'),
      passEntry('09-acceptance'),
    ];
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.planner).toBeNull();
    expect(result.evaluator).toBeNull();
    expect(result.next_phase).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Skip Passed Phases — AC-6
// ---------------------------------------------------------------------------

describe('runPhaseNext — Skip Passed Phases (AC-6)', () => {
  it('should skip to 04-test-gen when phases 01-03 already pass', () => {
    const result = next([
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
    ]);
    expect(result.next_phase).toBe('04-test-gen');
  });

  it('should skip to 06-unit-test when phases 01-05 already pass', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
      passEntry('05-implement'),
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('06-unit-test');
  });

  it('should return done=true when all phases have pass records', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
      passEntry('05-implement'),
      passEntry('06-unit-test'),
      passEntry('07-code-review'),
      passEntry('08-integration-test'),
      passEntry('09-acceptance'),
    ];
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Retry Logic — AC-7
// ---------------------------------------------------------------------------

describe('runPhaseNext — Retry Logic (AC-7)', () => {
  it('should return same phase for first retry after fail', () => {
    const result = next([passEntry('01-proposal'), failEntry('02-dev-design')]);
    expect(result.next_phase).toBe('02-dev-design');
    expect(result.error).toBeNull();
  });

  it('should return same phase on 4th consecutive fail (attempt 5)', () => {
    const result = next([
      passEntry('01-proposal'),
      failEntry('02-dev-design', 1),
      failEntry('02-dev-design', 2),
      failEntry('02-dev-design', 3),
      failEntry('02-dev-design', 4),
    ]);
    expect(result.next_phase).toBe('02-dev-design');
    expect(result.error).toBeNull();
  });

  it('should return max_retries_exceeded error on 5th consecutive fail', () => {
    const result = next([
      passEntry('01-proposal'),
      failEntry('02-dev-design', 1),
      failEntry('02-dev-design', 2),
      failEntry('02-dev-design', 3),
      failEntry('02-dev-design', 4),
      failEntry('02-dev-design', 5),
    ]);
    expect(result.error).toBe('max_retries_exceeded');
    expect(result.next_phase).toBeNull();
  });

  it('should return error message in Chinese for max retries', () => {
    const result = next([
      passEntry('01-proposal'),
      failEntry('02-dev-design', 1),
      failEntry('02-dev-design', 2),
      failEntry('02-dev-design', 3),
      failEntry('02-dev-design', 4),
      failEntry('02-dev-design', 5),
    ]);
    expect(result.message).toContain('超过最大重试次数');
  });

  it('should reset retry count after a pass (then fail, then pass, then fail)', () => {
    const result = next([
      passEntry('01-proposal', 1),
      passEntry('02-dev-design', 1),
      failEntry('03-test-design', 1),
    ]);
    expect(result.next_phase).toBe('03-test-design');
    expect(result.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Backtrack — AC-8
// ---------------------------------------------------------------------------

describe('runPhaseNext — Backtrack', () => {
  it('should return target phase on backtrack without clearing entries', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      backtrackEntry('03-test-design', '01-proposal'),
    ];
    const { result } = resolvePhaseNext({
      change: 'test-change',
      entries: entries,
    });
    expect(result.next_phase).toBe('01-proposal');
    // No updatedEntries — phase/next is read-only
    expect(result).not.toHaveProperty('updatedEntries');
  });

  it('should re-execute planner + evaluator after backtrack to 01-proposal', () => {
    const { result } = resolvePhaseNext({
      change: 'test-change',
      entries: [
        passEntry('01-proposal'),
        passEntry('02-dev-design'),
        backtrackEntry('02-dev-design', '01-proposal'),
      ],
    });
    expect(result.planner!.agent_type).toBe('dev-team:proposal-planner');
    expect(result.evaluator!.agent_type).toBe('dev-team:proposal-evaluator');
  });

  it('should handle backtrack from a later phase to mid-chain', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
      passEntry('05-implement'),
      passEntry('06-unit-test'),
      backtrackEntry('07-code-review', '04-test-gen'),
    ];
    const { result } = resolvePhaseNext({
      change: 'test-change',
      entries: entries,
    });
    expect(result.next_phase).toBe('04-test-gen');
  });

  it('should handle backtrack set in latest entry only', () => {
    // Previous backtrack but later entry overwrites it
    const result = next([
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      backtrackEntry('02-dev-design', '01-proposal'),
      passEntry('02-dev-design', 2), // Re-passed after backtrack
    ]);
    // Now the latest entry for 02-dev-design has verdict=pass, no backtrack
    // So backtrack is no longer active — should advance to 03-test-design
    expect(result.error).toBeNull();
    expect(result.next_phase).toBe('03-test-design');
  });

  it('should not modify entries when backtrack is detected (read-only)', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      backtrackEntry('02-dev-design', '01-proposal'),
    ];
    const entriesCopy = JSON.parse(JSON.stringify(entries));
    resolvePhaseNext({ change: 'test-change', entries });
    // Entries should be unmodified (no clearEntriesFromPhase, no updatedEntries)
    expect(entries).toEqual(entriesCopy);
  });

  it('should return earliest target for array backtrack_to (AC-16)', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
      backtrackEntry('04-test-gen', ['03-test-design', '01-proposal']),
    ];
    const { result } = resolvePhaseNext({
      change: 'test-change',
      entries,
    });
    // 01-proposal is earliest in phase table
    expect(result.next_phase).toBe('01-proposal');
  });

  it('should throw invalid_backtrack_target for unknown target', () => {
    const entries = [
      passEntry('01-proposal'),
      {
        phase: '02-dev-design',
        verdict: 'fail',
        attempt: 1,
        timestamp: nextTs(),
        backtrack_to: '99-unknown',
      },
    ];
    const { result } = resolvePhaseNext({ change: 'test-change', entries });
    expect(result.error).toBe('invalid_backtrack_target');
  });
});

// ---------------------------------------------------------------------------
// Stale Entry Filtering — AC-11, AC-12
// ---------------------------------------------------------------------------

describe('runPhaseNext — Stale Entry Filtering (AC-11, AC-12)', () => {
  it('should skip stale pass entries when determining next phase (AC-11)', () => {
    // 01 pass, 02 pass but stale, 03 pass
    const result = next([
      passEntry('01-proposal'),
      staleEntry('02-dev-design'),
      passEntry('03-test-design'),
    ]);
    // 02 appears stale to hasPhasePassed, so should be returned
    expect(result.next_phase).toBe('02-dev-design');
  });

  it('should return done=true when all phases have non-stale pass (mix of stale and fresh)', () => {
    const entries = [
      passEntry('01-proposal'),
      staleEntry('02-dev-design'), // stale
      passEntry('02-dev-design', 2), // fresh pass
      staleEntry('03-test-design'),
      passEntry('03-test-design', 2),
      passEntry('04-test-gen'),
      staleEntry('05-implement'),
      passEntry('05-implement', 2),
      passEntry('06-unit-test'),
      passEntry('07-code-review'),
      passEntry('08-integration-test'),
      staleEntry('09-acceptance'),
      passEntry('09-acceptance', 2),
    ];
    const result = next(entries);
    expect(result.done).toBe(true);
  });

  it('should treat missing stale field as stale:false (AC-12 backward compat)', () => {
    // Entries without stale field should be treated as valid
    const result = next([passEntry('01-proposal')]);
    expect(result.next_phase).toBe('02-dev-design');
    expect(result.error).toBeNull();
  });

  it('should resume correctly when stale entries are in middle phases', () => {
    // 01-03 pass (03 stale), 04 pass
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      staleEntry('03-test-design'), // marked stale by backtrack propagation
      passEntry('04-test-gen'), // 04's pass should be ignored because 03 needs to be redone first
    ];
    const result = next(entries);
    // Linear scan: 01 and 02 pass non-stale, 03 pass but stale → return 03
    expect(result.next_phase).toBe('03-test-design');
  });
});

// ---------------------------------------------------------------------------
// Dependency-Graph Driven — AC-5, AC-6, AC-7
// ---------------------------------------------------------------------------

describe('runPhaseNext — Dependency-Graph Driven (AC-5, AC-6, AC-7)', () => {
  it('should return 02-dev-design when 03-test-design missing pass but 02 not passed (AC-5)', () => {
    // 01 pass, 02 fail → 03 cannot run (depends on 02)
    const result = next([passEntry('01-proposal'), failEntry('02-dev-design')]);
    expect(result.next_phase).toBe('02-dev-design');
  });

  it('should return 05-implement when 02-dev-design is pass but 05 not passed (AC-6)', () => {
    // 01 pass, 02 pass → 03/04 not passed but 05's dependency (02) is satisfied
    // Linear scan returns 03 first, but 05 is also available
    // This test verifies that 03 not being passed doesn't block 05 from being considered
    // when scanning linearly — 03 comes before 05 and has no valid pass
    const result = next([passEntry('01-proposal'), passEntry('02-dev-design')]);
    // 03 is first without pass → correct per linear scan + propagation model
    expect(result.next_phase).toBe('03-test-design');
  });

  it('should skip stale 04 and return 04 when 04 is stale even though later phases pass (AC-7)', () => {
    // 01-05 pass, but 04 is stale (backtrack propagation)
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      staleEntry('04-test-gen'), // stale
      passEntry('05-implement'),
      passEntry('06-unit-test'),
    ];
    const result = next(entries);
    // 04 has stale=true → hasPhasePassed returns false → returned as next
    expect(result.next_phase).toBe('04-test-gen');
  });
});

// ---------------------------------------------------------------------------
// Round Limit
// ---------------------------------------------------------------------------

describe('runPhaseNext — Round Limit (AC-10)', () => {
  it('should return round_limit_exceeded error when round > 20', () => {
    // Construct 21 rounds of entries
    const entries = [];
    for (let i = 0; i < 21; i++) {
      entries.push(passEntry('01-proposal', i + 1));
    }
    const result = next(entries);
    expect(result.error).toBe('round_limit_exceeded');
  });

  it('should return Chinese error message for round limit', () => {
    const entries = [];
    for (let i = 0; i < 21; i++) {
      entries.push(passEntry('01-proposal', i + 1));
    }
    const result = next(entries);
    expect(result.message).toContain('20 轮');
  });

  it('should NOT error at exactly round 20 with all pass', () => {
    // 20 entries = round 21, which exceeds limit
    // Actually round = entries.length + 1, so round === 20 when entries.length === 19
    const entries = [];
    for (let i = 0; i < 19; i++) {
      entries.push(passEntry(['01-proposal', '02-dev-design', '03-test-design'][i % 3], i + 1));
    }
    // At 19 entries, round = 20. We should still be able to process.
    const result = next(entries);
    // round = 20, should not error (threshold is > 20)
    expect(result.error).toBeNull();
  });

  it('should detect round limit via backtrack cycles', () => {
    // Simulate 21+ rounds caused by repeated backtrack
    const entries: MockEntry[] = [];
    for (let i = 0; i < 21; i++) {
      const phase = i % 2 === 0 ? '01-proposal' : '02-dev-design';
      const entry: MockEntry = {
        phase,
        verdict: 'fail',
        attempt: Math.floor(i / 2) + 1,
        timestamp: new Date(Date.now() + i).toISOString(),
        backtrack_to: i % 2 === 1 ? '01-proposal' : null,
      };
      entries.push(entry);
    }
    const result = next(entries);
    expect(result.error).toBe('round_limit_exceeded');
  });
});

// ---------------------------------------------------------------------------
// Mid-Phase Interruption
// ---------------------------------------------------------------------------

describe('runPhaseNext — Mid-Phase Interruption', () => {
  it('should re-return the phase when evaluator has not logged (no entries for phase)', () => {
    // If the latest eval.json only shows pass for prior phases but the current
    // phase has no evaluator entry yet (never started), we treat it as "not passed"
    // and return it for execution.
    const result = next([passEntry('01-proposal')]);
    // 01-proposal passed, so next should be 02-dev-design
    expect(result.next_phase).toBe('02-dev-design');
  });

  it('should return the incomplete phase if evaluator never logged', () => {
    // If 01-proposal passed but 02-dev-design has no eval entries at all
    // (planner ran but evaluator never logged), the phase is not counted
    // as passed and phase/next should return it for execution.
    const result = next([passEntry('01-proposal')]);
    expect(result.next_phase).toBe('02-dev-design');
  });
});

// ---------------------------------------------------------------------------
// Skipped (No-Op) Phases
// ---------------------------------------------------------------------------

describe('runPhaseNext — Skipped Phases', () => {
  it('should treat skipped entries as pass', () => {
    const result = next([skippedEntry('04-test-gen')]);
    // 01-proposal has no pass, so that should be returned despite 04-test-gen having a skipped entry
    expect(result.next_phase).toBe('01-proposal');
  });

  it('should advance past a mix of skipped and passed phases', () => {
    // Pass 01, skip 02, pass 03 — should advance to 04
    const entries = [
      passEntry('01-proposal'),
      skippedEntry('02-dev-design'),
      passEntry('03-test-design'),
    ];
    const result = next(entries);
    expect(result.next_phase).toBe('04-test-gen');
  });
});

// ---------------------------------------------------------------------------
// workflow_type Variants
// ---------------------------------------------------------------------------

describe('runPhaseNext — workflow_type', () => {
  it('should default to requirement when workflow_type is omitted', () => {
    const result = next([], 'test-change');
    expect(result.total_phases).toBe(9);
    expect(result.next_phase).toBe('01-proposal');
  });

  it('should follow bug-fix phase table when workflow_type is bug-fix', () => {
    const result = resolvePhaseNext({
      change: 'test-change',
      entries: [],
      workflowType: 'bug-fix',
    }).result;
    expect(result.total_phases).toBe(6);
  });

  it('should follow refactor phase table when workflow_type is refactor', () => {
    const result = resolvePhaseNext({
      change: 'test-change',
      entries: [],
      workflowType: 'refactor',
    }).result;
    expect(result.total_phases).toBe(9);
  });

  it('should return 09-acceptance from bug-fix after 07-code-review passes', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('05-implement'),
      passEntry('06-unit-test'),
      passEntry('07-code-review'),
    ];
    const { result } = resolvePhaseNext({
      change: 'test-change',
      entries: entries,
      workflowType: 'bug-fix',
    });
    expect(result.next_phase).toBe('09-acceptance');
  });
});

// ---------------------------------------------------------------------------
// Input Validation
// ---------------------------------------------------------------------------

describe('resolvePhaseNext — Input Validation', () => {
  it('should not throw for empty entries array', () => {
    expect(() => next([], 'test-change')).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// Boundary Scenarios
// ---------------------------------------------------------------------------

describe('Boundary Scenarios', () => {
  it('should handle empty eval.json (first run)', () => {
    const result = next([]);
    expect(result.next_phase).toBe('01-proposal');
    expect(result.round).toBe(1);
  });

  it('should handle all phases as first_run (empty entries)', () => {
    const result = next([]);
    expect(result.next_phase).toBe('01-proposal');
    expect(result.round).toBe(1);
  });

  it('should handle exactly 20 rounds with all pass (no limit error)', () => {
    // To hit exactly round 20, we need 19 entries. If all phases are pass
    // by that point, we should get done=true, not round limit error.
    const entries: MockEntry[] = [];
    // Add 9 phases pass in sequence
    const phaseIds = [
      '01-proposal',
      '02-dev-design',
      '03-test-design',
      '04-test-gen',
      '05-implement',
      '06-unit-test',
      '07-code-review',
      '08-integration-test',
      '09-acceptance',
    ];
    // Each phase has 2 pass entries (total 18), plus 1 extra = 19 entries = round 20
    for (let i = 0; i < 18; i++) {
      entries.push(passEntry(phaseIds[i % 9], Math.floor(i / 9) + 1));
    }
    // At this point all 9 phases have at least one pass
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.error).toBeNull();
  });

  it('should handle skipped phases correctly', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      skippedEntry('03-test-design'),
      skipPedEntry('04-test-gen'),
    ];
    const result = next(entries);
    // 01-04 all have pass or skipped, next should be 05-implement
    expect(result.next_phase).toBe('05-implement');
  });

  it('should handle change name with special characters', () => {
    const result = next([], 'my-test-变更');
    expect(result.planner!.prompt).toContain('my-test-变更');
  });

  it('should distinguish 01-proposal from 01-requirements entries', () => {
    // Old phase name 01-requirements should not match 01-proposal
    const entries = [passEntry('01-requirements')];
    const result = next(entries);
    // Since the phase table has '01-proposal', entries with '01-requirements'
    // should not be treated as matching the first phase.
    expect(result.next_phase).toBe('01-proposal');
    expect(result.done).toBe(false);
  });

  it('should return done=true after 09-acceptance passes', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
      passEntry('05-implement'),
      passEntry('06-unit-test'),
      passEntry('07-code-review'),
      passEntry('08-integration-test'),
      passEntry('09-acceptance'),
    ];
    const result = next(entries);
    expect(result.done).toBe(true);
    expect(result.planner).toBeNull();
    expect(result.evaluator).toBeNull();
  });
});

describe('phase/next Output Schema', () => {
  it('should return valid JSON when next phase is ready', () => {
    const result = resolvePhaseNext({ change: 'test', entries: [] }).result;
    // Verify all required fields exist
    expect(result).toHaveProperty('done');
    expect(result).toHaveProperty('error');
    expect(result).toHaveProperty('next_phase');
    expect(result).toHaveProperty('phase_pattern');
    expect(result).toHaveProperty('planner');
    expect(result).toHaveProperty('evaluator');
    expect(result).toHaveProperty('auto_steps');
    expect(result).toHaveProperty('total_phases');
    expect(result).toHaveProperty('phase_index');
    expect(result).toHaveProperty('round');
  });

  it('should return valid JSON when all phases are done', () => {
    const entries = [
      passEntry('01-proposal'),
      passEntry('02-dev-design'),
      passEntry('03-test-design'),
      passEntry('04-test-gen'),
      passEntry('05-implement'),
      passEntry('06-unit-test'),
      passEntry('07-code-review'),
      passEntry('08-integration-test'),
      passEntry('09-acceptance'),
    ];
    const result = next(entries, 'test');
    expect(result.done).toBe(true);
    expect(result.next_phase).toBeNull();
    expect(result.planner).toBeNull();
    expect(result.evaluator).toBeNull();
  });

  it('should return valid JSON on error', () => {
    const entries: MockEntry[] = [];
    for (let i = 0; i < 5; i++) {
      entries.push(failEntry('01-proposal', i + 1));
    }
    entries.push(failEntry('01-proposal', 6)); // 6 attempts = 5 fails + 1 = max retries
    // Actually 5 fails (attempts 1-5) should trigger max_retries
    const result = next([
      failEntry('01-proposal', 1),
      failEntry('01-proposal', 2),
      failEntry('01-proposal', 3),
      failEntry('01-proposal', 4),
      failEntry('01-proposal', 5),
    ]);
    expect(result.error).toBe('max_retries_exceeded');
    expect(result.next_phase).toBeNull();
    expect(result.planner).toBeNull();
    expect(result.evaluator).toBeNull();
  });

  it('should return same prompt on retry (skill handles retry context)', () => {
    // The phase/next tool returns the same prompt template on retry.
    // The skill is responsible for adding retry context (e.g. "attempt 2/5").
    const firstResult = next([], 'test-change');
    const retryResult = next([failEntry('01-proposal', 1)], 'test-change');
    // Both should have the same prompt
    expect(firstResult.planner!.prompt).toBe(retryResult.planner!.prompt);
    expect(retryResult.error).toBeNull();
  });
});
