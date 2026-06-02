import { describe, it, expect } from 'vite-plus/test';

import { validateVerdict, buildEntry, type BuildEntryParams } from './eval-json';

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
});
