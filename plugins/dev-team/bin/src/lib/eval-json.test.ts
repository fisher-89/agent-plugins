import { describe, it, expect } from 'vite-plus/test';

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
    phase: '06-unit-test',
    verdict: 'pass',
    report: 'All tests passed',
    items: [{ item: '测试覆盖率达到80%', pass: true, evidence: 'ok' }],
    attempt: 1,
    backtrack_to: null,
  };

  it('should build a basic entry with required fields', () => {
    const entry = buildEntry(baseParams);
    expect(entry.phase).toBe('06-unit-test');
    expect(entry.verdict).toBe('pass');
    expect(entry.attempt).toBe(1);
    expect(entry.timestamp).toBeDefined();
    expect(entry.backtrack_to).toBeNull();
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

  it('should include backtrack_to when set', () => {
    const entry = buildEntry({ ...baseParams, backtrack_to: '04-test-gen' });
    expect(entry.backtrack_to).toBe('04-test-gen');
  });

  it('should set backtrack_to null when undefined', () => {
    const entry = buildEntry(baseParams);
    expect(entry.backtrack_to).toBeNull();
  });

  it('should accept backtrack_to as array', () => {
    const entry = buildEntry({ ...baseParams, backtrack_to: ['02-dev-design', '03-test-design'] });
    expect(entry.backtrack_to).toEqual(['02-dev-design', '03-test-design']);
  });
});

// ---------------------------------------------------------------------------
// markPhaseStale — AC-8, AC-14
// ---------------------------------------------------------------------------

function makePassEntry(phase: string, attempt: number = 1, ts?: string): EvalEntry {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: ts || new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
    report: '',
    items: [],
  };
}

function makeFailEntry(phase: string, attempt: number = 1): EvalEntry {
  return {
    phase,
    verdict: 'fail',
    attempt,
    timestamp: new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
    report: '',
    items: [],
  };
}

describe('markPhaseStale', () => {
  it('should mark latest pass entry stale and propagate downstream (AC-8)', () => {
    const entries = [
      makePassEntry('01-proposal', 1),
      makePassEntry('02-dev-design', 1),
      makePassEntry('03-test-design', 1),
      makePassEntry('04-test-gen', 1),
      makePassEntry('05-implement', 1),
    ];
    markPhaseStale(entries, '02-dev-design');

    // 02 should be stale
    const e2 = entries.find((e) => e.phase === '02-dev-design')!;
    expect(e2.stale).toBe(true);

    // Downstream should be propagated: 03 (dep on 02), 05 (dep on 02)
    const e3 = entries.find((e) => e.phase === '03-test-design')!;
    expect(e3.stale).toBe(true);
    const e5 = entries.find((e) => e.phase === '05-implement')!;
    expect(e5.stale).toBe(true);

    // 01 should NOT be stale (no dependency on 02)
    const e1 = entries.find((e) => e.phase === '01-proposal')!;
    expect(e1.stale).toBeUndefined();
  });

  it('should mark latest of multiple pass entries', () => {
    const entries = [
      makePassEntry('02-dev-design', 1, '2026-01-01T00:00:00.000Z'),
      makePassEntry('02-dev-design', 2, '2026-01-02T00:00:00.000Z'),
    ];
    markPhaseStale(entries, '02-dev-design');

    const attempt2 = entries.find((e) => e.phase === '02-dev-design' && e.attempt === 2)!;
    expect(attempt2.stale).toBe(true);
  });

  it('should be no-op when no pass entry exists', () => {
    const entries = [makeFailEntry('02-dev-design', 1)];
    expect(() => markPhaseStale(entries, '02-dev-design')).not.toThrow();
    expect(entries[0].stale).toBeUndefined();
  });

  it('should propagate to full transitive closure (AC-14)', () => {
    const entries = [];
    for (const phase of [
      '01-proposal',
      '02-dev-design',
      '03-test-design',
      '04-test-gen',
      '05-implement',
      '06-unit-test',
      '07-code-review',
      '08-integration-test',
      '09-acceptance',
    ]) {
      entries.push(makePassEntry(phase, 1));
    }
    markPhaseStale(entries, '02-dev-design');

    // 02 stale
    expect(entries.find((e) => e.phase === '02-dev-design')!.stale).toBe(true);
    // 03 stale (dep on 02)
    expect(entries.find((e) => e.phase === '03-test-design')!.stale).toBe(true);
    // 04 stale (transitive: 03 → 04)
    expect(entries.find((e) => e.phase === '04-test-gen')!.stale).toBe(true);
    // 05 stale (dep on 02)
    expect(entries.find((e) => e.phase === '05-implement')!.stale).toBe(true);
    // 06 stale (dep on 04+05, both in chain)
    expect(entries.find((e) => e.phase === '06-unit-test')!.stale).toBe(true);
    // 07 stale
    expect(entries.find((e) => e.phase === '07-code-review')!.stale).toBe(true);
    // 08 stale
    expect(entries.find((e) => e.phase === '08-integration-test')!.stale).toBe(true);
    // 09 stale (dep on 01+02+05)
    expect(entries.find((e) => e.phase === '09-acceptance')!.stale).toBe(true);
    // 01 should NOT be stale
    expect(entries.find((e) => e.phase === '01-proposal')!.stale).toBeUndefined();
  });

  it('should not mark same-phase new entry as stale', () => {
    // This simulates: old entry marked stale, new entry written after markPhaseStale
    const entries = [makePassEntry('02-dev-design', 1)];
    markPhaseStale(entries, '02-dev-design');
    // Old entry stale
    expect(entries[0].stale).toBe(true);

    // Simulate new entry being added after markPhaseStale
    entries.push(makePassEntry('02-dev-design', 2));
    const newEntry = entries.find((e) => e.phase === '02-dev-design' && e.attempt === 2)!;
    expect(newEntry.stale).toBeUndefined();
  });

  it('should mark 04-test-gen stale when marking 05-implement — 04 为直接 downstream（AC-8）', () => {
    const entries = [
      makePassEntry('01-proposal', 1),
      makePassEntry('02-dev-design', 1),
      makePassEntry('03-test-design', 1),
      makePassEntry('04-test-gen', 1),
      makePassEntry('05-implement', 1),
    ];
    markPhaseStale(entries, '05-implement');

    expect(entries.find((e) => e.phase === '05-implement')!.stale).toBe(true);
    expect(entries.find((e) => e.phase === '04-test-gen')!.stale).toBe(true);
    expect(entries.find((e) => e.phase === '03-test-design')!.stale).toBeUndefined();
  });

  it('should propagate from 05-implement to 06/07/08/09（AC-8）', () => {
    const entries = [];
    for (const phase of [
      '01-proposal',
      '02-dev-design',
      '03-test-design',
      '04-test-gen',
      '05-implement',
      '06-unit-test',
      '07-code-review',
      '08-integration-test',
      '09-acceptance',
    ]) {
      entries.push(makePassEntry(phase, 1));
    }
    markPhaseStale(entries, '05-implement');

    for (const phase of [
      '05-implement',
      '04-test-gen',
      '06-unit-test',
      '07-code-review',
      '08-integration-test',
      '09-acceptance',
    ]) {
      expect(entries.find((e) => e.phase === phase)!.stale).toBe(true);
    }
    expect(entries.find((e) => e.phase === '03-test-design')!.stale).toBeUndefined();
  });

  it('should mark 04 and downstream stale when marking 03-test-design（AC-9）', () => {
    const entries = [
      makePassEntry('01-proposal', 1),
      makePassEntry('02-dev-design', 1),
      makePassEntry('03-test-design', 1),
      makePassEntry('04-test-gen', 1),
      makePassEntry('05-implement', 1),
      makePassEntry('06-unit-test', 1),
    ];
    markPhaseStale(entries, '03-test-design');

    for (const phase of ['03-test-design', '04-test-gen', '06-unit-test']) {
      expect(entries.find((e) => e.phase === phase)!.stale).toBe(true);
    }
  });

  it('should NOT mark 05-implement stale when marking 03-test-design（AC-9）', () => {
    const entries = [
      makePassEntry('01-proposal', 1),
      makePassEntry('02-dev-design', 1),
      makePassEntry('03-test-design', 1),
      makePassEntry('04-test-gen', 1),
      makePassEntry('05-implement', 1),
    ];
    markPhaseStale(entries, '03-test-design');

    expect(entries.find((e) => e.phase === '05-implement')!.stale).toBeUndefined();
    expect(entries.find((e) => e.phase === '04-test-gen')!.stale).toBe(true);
  });
});
