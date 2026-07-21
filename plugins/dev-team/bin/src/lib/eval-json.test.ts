import { describe, it, expect } from 'vite-plus/test';

import { phaseLogSchema } from '../schemas';
import {
  validateVerdict,
  buildEntry,
  markPhaseStale,
  type BuildEntryParams,
  type EvalEntry,
} from './eval-json';

describe('validateVerdict', () => {
  it("should accept 'pass'", () => {
    expect(() => validateVerdict('pass')).not.toThrow();
  });

  it("should accept 'fail'", () => {
    expect(() => validateVerdict('fail')).not.toThrow();
  });

  it('should reject invalid verdict', () => {
    expect(() => validateVerdict('invalid')).toThrow();
  });

  it('should reject empty string', () => {
    expect(() => validateVerdict('')).toThrow();
  });

  it("should accept 'pass' when skipped is true", () => {
    expect(() => validateVerdict('pass', true)).not.toThrow();
  });

  it("should reject 'fail' when skipped is true", () => {
    expect(() => validateVerdict('fail', true)).toThrow(/skipped=true 时 verdict 必须为 "pass"/);
  });

  it("should reject 'pass' when skipped is false (same behavior as undefined)", () => {
    expect(() => validateVerdict('pass', false)).not.toThrow();
  });
});

describe('buildEntry', () => {
  const baseParams: BuildEntryParams = {
    phase: 'test-execution',
    verdict: 'pass',
    report: 'All tests passed',
    checklist: [{ item: '测试覆盖率达到80%', pass: true, evidence: 'ok' }],
    attempt: 1,
  };

  it('should build a basic entry with required fields', () => {
    const entry = buildEntry(baseParams);
    expect(entry.phase).toBe('test-execution');
    expect(entry.verdict).toBe('pass');
    expect(entry.attempt).toBe(1);
    expect(entry.timestamp).toBeDefined();
    // backtrack_to not set by buildEntry (handled by standalone backtrack tool)
    expect(entry.backtrack_to).toBeUndefined();
    // Extended fields not set
    expect(entry.skipped).toBeUndefined();
  });

  it('should include skipped field when set', () => {
    const entry = buildEntry({ ...baseParams, skipped: true });
    expect(entry.skipped).toBe(true);
  });

  it('should include all extended fields simultaneously', () => {
    const params: BuildEntryParams = {
      ...baseParams,
      skipped: true,
      report: 'No tests found, skipping',
    };
    const entry = buildEntry(params);
    expect(entry.skipped).toBe(true);
    expect(entry.report).toBe('No tests found, skipping');
  });

  it('should build entry without backtrack fields (now handled by standalone backtrack tool)', () => {
    const entry = buildEntry(baseParams);
    expect(entry.phase).toBe('test-execution');
    expect(entry.backtrack_to).toBeUndefined();
    expect(entry.backtrack_reason).toBeUndefined();
  });

  it('phase 为 "integration-test" 时 zod parse 应失败（不在枚举中）', () => {
    const result = phaseLogSchema.safeParse({
      phase: 'integration-test',
      attempt: 1,
      verdict: 'pass',
      report: 'test',
      checklist: [],
      timestamp: new Date().toISOString(),
      backtrack_to: null,
    });
    expect(result.success).toBe(false);
  });

  it('phase 为 "unit-test" 时 zod parse 应失败', () => {
    const result = phaseLogSchema.safeParse({
      phase: 'unit-test',
      attempt: 1,
      verdict: 'pass',
      report: 'test',
      checklist: [],
      timestamp: new Date().toISOString(),
      backtrack_to: null,
    });
    expect(result.success).toBe(false);
  });

  it('report 长度 > 500 字符时 zod parse 应失败', () => {
    expect(() => buildEntry({ ...baseParams, report: 'x'.repeat(501) })).toThrow();
  });

  it('checklist 为空数组时应通过', () => {
    const entry = buildEntry({ ...baseParams, checklist: [] });
    expect(entry.checklist).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// markPhaseStale — AC-8, AC-14
// ---------------------------------------------------------------------------

function makePassEntry(phase: EvalEntry['phase'], attempt: number = 1, ts?: string): EvalEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: ts || new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
    report: '',
    checklist: [],
  };
}

function makeFailEntry(phase: EvalEntry['phase'], attempt: number = 1): EvalEntry {
  return {
    phase,
    verdict: 'fail',
    attempt,
    timestamp: new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
    report: '',
    checklist: [],
  };
}

describe('markPhaseStale', () => {
  it('should mark latest pass entry stale and propagate downstream (AC-8)', () => {
    const entries = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
    ];
    markPhaseStale(entries, 'dev-design', 'requirement');

    // 02 should be stale
    const e2 = entries.find((e) => e.phase === 'dev-design')!;
    expect(e2.stale).toBe(true);

    // Downstream should be propagated: 03 (dep on 02), 05 (dep on 02)
    const e3 = entries.find((e) => e.phase === 'test-design')!;
    expect(e3.stale).toBe(true);
    const e5 = entries.find((e) => e.phase === 'implement')!;
    expect(e5.stale).toBe(true);

    // 01 should NOT be stale (no dependency on 02)
    const e1 = entries.find((e) => e.phase === 'proposal')!;
    expect(e1.stale).toBeUndefined();
  });

  it('should mark latest of multiple pass entries', () => {
    const entries = [
      makePassEntry('dev-design', 1, '2026-01-01T00:00:00.000Z'),
      makePassEntry('dev-design', 2, '2026-01-02T00:00:00.000Z'),
    ];
    markPhaseStale(entries, 'dev-design', 'requirement');

    const attempt2 = entries.find((e) => e.phase === 'dev-design' && e.attempt === 2)!;
    expect(attempt2.stale).toBe(true);
  });

  it('should be no-op when no pass entry exists', () => {
    const entries = [makeFailEntry('dev-design', 1)];
    expect(() => markPhaseStale(entries, 'dev-design', 'requirement')).not.toThrow();
    expect(entries[0].stale).toBeUndefined();
  });

  it('should propagate to full transitive closure', () => {
    const entries = [];
    const phases: EvalEntry['phase'][] = [
      'proposal',
      'dev-design',
      'test-design',
      'test-gen',
      'implement',
      'test-execution',
      'code-review',
      'acceptance',
    ];
    for (const phase of phases) {
      entries.push(makePassEntry(phase, 1));
    }
    markPhaseStale(entries, 'dev-design', 'requirement');

    // 02 stale
    expect(entries.find((e) => e.phase === 'dev-design')!.stale).toBe(true);
    // 03 stale (dep on 02)
    expect(entries.find((e) => e.phase === 'test-design')!.stale).toBe(true);
    // 04 stale (transitive: 03 → 04)
    expect(entries.find((e) => e.phase === 'test-gen')!.stale).toBe(true);
    // 05 stale (dep on 02)
    expect(entries.find((e) => e.phase === 'implement')!.stale).toBe(true);
    // 06 stale (dep on 04+05, both in chain)
    expect(entries.find((e) => e.phase === 'test-execution')!.stale).toBe(true);
    // 07 stale
    expect(entries.find((e) => e.phase === 'code-review')!.stale).toBe(true);
    // 08 stale (dep on 01+02+05)
    expect(entries.find((e) => e.phase === 'acceptance')!.stale).toBe(true);
    // 01 should NOT be stale
    expect(entries.find((e) => e.phase === 'proposal')!.stale).toBeUndefined();
  });

  it('should not mark same-phase new entry as stale', () => {
    // This simulates: old entry marked stale, new entry written after markPhaseStale
    const entries = [makePassEntry('dev-design', 1)];
    markPhaseStale(entries, 'dev-design', 'requirement');
    // Old entry stale
    expect(entries[0].stale).toBe(true);

    // Simulate new entry being added after markPhaseStale
    entries.push(makePassEntry('dev-design', 2));
    const newEntry = entries.find((e) => e.phase === 'dev-design' && e.attempt === 2)!;
    expect(newEntry.stale).toBeUndefined();
  });

  it('should mark test-gen stale when marking implement — test-gen 为直接 downstream（AC-8）', () => {
    const entries = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
    ];
    markPhaseStale(entries, 'implement', 'requirement');

    expect(entries.find((e) => e.phase === 'implement')!.stale).toBe(true);
    expect(entries.find((e) => e.phase === 'test-gen')!.stale).toBe(true);
    expect(entries.find((e) => e.phase === 'test-design')!.stale).toBeUndefined();
  });

  it('should propagate from implement to test-execution/code-review/acceptance', () => {
    const entries = [];
    const phases: EvalEntry['phase'][] = [
      'proposal',
      'dev-design',
      'test-design',
      'test-gen',
      'implement',
      'test-execution',
      'code-review',
      'acceptance',
    ];
    for (const phase of phases) {
      entries.push(makePassEntry(phase, 1));
    }
    markPhaseStale(entries, 'implement', 'requirement');

    for (const phase of ['implement', 'test-gen', 'test-execution', 'code-review', 'acceptance']) {
      expect(entries.find((e) => e.phase === phase)!.stale).toBe(true);
    }
    expect(entries.find((e) => e.phase === 'test-design')!.stale).toBeUndefined();
  });

  it('should mark test-gen and downstream stale when marking test-design', () => {
    const entries = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
      makePassEntry('test-execution', 1),
    ];
    markPhaseStale(entries, 'test-design', 'requirement');

    for (const phase of ['test-design', 'test-gen', 'test-execution']) {
      expect(entries.find((e) => e.phase === phase)!.stale).toBe(true);
    }
  });

  it('should NOT mark implement stale when marking test-design（AC-9）', () => {
    const entries = [
      makePassEntry('proposal', 1),
      makePassEntry('dev-design', 1),
      makePassEntry('test-design', 1),
      makePassEntry('test-gen', 1),
      makePassEntry('implement', 1),
    ];
    markPhaseStale(entries, 'test-design', 'requirement');

    expect(entries.find((e) => e.phase === 'implement')!.stale).toBeUndefined();
    expect(entries.find((e) => e.phase === 'test-gen')!.stale).toBe(true);
  });
});
