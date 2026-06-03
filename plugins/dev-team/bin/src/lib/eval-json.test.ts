import { describe, it, expect } from 'vite-plus/test';

import {
  validateVerdict,
  buildEntry,
  checkGate,
  markPhaseStale,
  propagateStale,
  type BuildEntryParams,
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
    items: [{ item: '测试覆盖率达到80%', pass: true, evidence: 'ok', notes: '' }],
    attempt: 1,
  };

  it('should build a basic entry with required fields', () => {
    const entry = buildEntry(baseParams);
    expect(entry.phase).toBe('06-unit-test');
    expect(entry.verdict).toBe('pass');
    expect(entry.attempt).toBe(1);
    expect(entry.schema_version).toBe('1.0');
    expect(entry.timestamp).toBeDefined();
    expect(entry.backtrack_to).toBeNull();
    // Extended fields not set
    expect(entry.skipped).toBeUndefined();
    expect(entry.findings).toBeUndefined();
    expect(entry.phase_suffix).toBeUndefined();
  });

  it('should include skipped field when set', () => {
    const entry = buildEntry({ ...baseParams, skipped: true });
    expect(entry.skipped).toBe(true);
  });

  it('should include findings field when set', () => {
    const entry = buildEntry({
      ...baseParams,
      findings: 'Root cause: syntax error in test file',
    });
    expect(entry.findings).toBe('Root cause: syntax error in test file');
  });

  it('should include phase_suffix field when set', () => {
    const entry = buildEntry({ ...baseParams, phase_suffix: 'static-check' });
    expect(entry.phase_suffix).toBe('static-check');
  });

  it('should include all extended fields simultaneously', () => {
    const params: BuildEntryParams = {
      ...baseParams,
      skipped: true,
      findings: 'No tests found, skipping',
      phase_suffix: 'noop-check',
    };
    const entry = buildEntry(params);
    expect(entry.skipped).toBe(true);
    expect(entry.findings).toBe('No tests found, skipping');
    expect(entry.phase_suffix).toBe('noop-check');
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
// checkGate — stale filtering
// ---------------------------------------------------------------------------

describe('checkGate', () => {
  const entries = [
    { phase: '01-proposal', verdict: 'pass', timestamp: '2026-01-01T00:00:00.000Z' },
    { phase: '02-dev-design', verdict: 'pass', timestamp: '2026-01-02T00:00:00.000Z' },
    {
      phase: '03-test-design',
      verdict: 'pass',
      timestamp: '2026-01-03T00:00:00.000Z',
      stale: true,
    },
  ];

  it('should return passed=true when all prerequisites have non-stale pass', () => {
    const result = checkGate(entries, ['01-proposal']);
    expect(result.passed).toBe(true);
  });

  it('should return passed=false when prerequisite has stale pass only', () => {
    const result = checkGate(entries, ['03-test-design']);
    expect(result.passed).toBe(false);
    expect(result.missing).toContain('03-test-design');
  });

  it('should treat missing stale field as stale:false (backward compat)', () => {
    const result = checkGate(entries, ['02-dev-design']);
    expect(result.passed).toBe(true);
  });

  it('should return passed=false for missing prerequisite', () => {
    const result = checkGate(entries, ['04-test-gen']);
    expect(result.passed).toBe(false);
    expect(result.missing).toContain('04-test-gen');
  });
});

// ---------------------------------------------------------------------------
// markPhaseStale — AC-8, AC-14
// ---------------------------------------------------------------------------

function makePassEntry(phase: string, attempt: number = 1, ts?: string): Record<string, any> {
  return {
    phase,
    verdict: 'pass',
    attempt,
    timestamp: ts || new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
  };
}

function makeFailEntry(phase: string, attempt: number = 1): Record<string, any> {
  return {
    phase,
    verdict: 'fail',
    attempt,
    timestamp: new Date(Date.now() + attempt).toISOString(),
    backtrack_to: null,
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
    const e2 = entries.find((e) => e.phase === '02-dev-design');
    expect(e2.stale).toBe(true);

    // Downstream should be propagated: 03 (dep on 02), 05 (dep on 02)
    const e3 = entries.find((e) => e.phase === '03-test-design');
    expect(e3.stale).toBe(true);
    const e5 = entries.find((e) => e.phase === '05-implement');
    expect(e5.stale).toBe(true);

    // 01 should NOT be stale (no dependency on 02)
    const e1 = entries.find((e) => e.phase === '01-proposal');
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
});

// ---------------------------------------------------------------------------
// propagateStale
// ---------------------------------------------------------------------------

describe('propagateStale', () => {
  it('should propagate from 01-proposal to all phases', () => {
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
    propagateStale(entries, '01-proposal', 'requirement');

    // Everything should be stale
    for (const phase of [
      '02-dev-design',
      '03-test-design',
      '04-test-gen',
      '05-implement',
      '06-unit-test',
      '07-code-review',
      '08-integration-test',
      '09-acceptance',
    ]) {
      expect(entries.find((e) => e.phase === phase)!.stale).toBe(true);
    }
  });

  it('should not mark source phase itself if not a dependent of itself', () => {
    const entries = [makePassEntry('01-proposal', 1)];
    propagateStale(entries, '01-proposal', 'requirement');
    // 01 is not in its own dependents list
    expect(entries[0].stale).toBeUndefined();
  });

  it('should propagate from 03-test-design to test track only', () => {
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
    propagateStale(entries, '03-test-design', 'requirement');

    // Test track phases should be stale
    expect(entries.find((e) => e.phase === '04-test-gen')!.stale).toBe(true);
    expect(entries.find((e) => e.phase === '06-unit-test')!.stale).toBe(true);
    expect(entries.find((e) => e.phase === '07-code-review')!.stale).toBe(true);
    expect(entries.find((e) => e.phase === '08-integration-test')!.stale).toBe(true);

    // Dev track phases should NOT be stale
    expect(entries.find((e) => e.phase === '01-proposal')!.stale).toBeUndefined();
    expect(entries.find((e) => e.phase === '02-dev-design')!.stale).toBeUndefined();
    expect(entries.find((e) => e.phase === '05-implement')!.stale).toBeUndefined();
    expect(entries.find((e) => e.phase === '09-acceptance')!.stale).toBeUndefined();
  });

  it('should be no-op for leaf phases', () => {
    const entries = [makePassEntry('06-unit-test', 1)];
    propagateStale(entries, '06-unit-test', 'requirement');
    expect(entries[0].stale).toBeUndefined();
  });

  it('should handle no downstream entries gracefully', () => {
    const entries = [makePassEntry('02-dev-design', 1)];
    expect(() => propagateStale(entries, '02-dev-design', 'requirement')).not.toThrow();
  });
});
