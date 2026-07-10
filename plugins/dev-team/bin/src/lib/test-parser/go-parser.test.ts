/**
 * Tests for lib/test-parser/go-parser -- parses `go test -json` line-delimited JSON.
 */

import { describe, it, expect } from 'vite-plus/test';

import { parseGoOutput } from './go-parser';

function goEvent(event: {
  Action: string;
  Test?: string;
  Elapsed?: number;
  Output?: string;
}): string {
  return JSON.stringify(event);
}

// ===========================================================================
// Basic parsing
// ===========================================================================

describe('parseGoOutput -- basic parsing', () => {
  it('should parse line-delimited JSON and count pass/fail/skip', () => {
    const stdout = [
      goEvent({ Action: 'run', Test: 'TestFoo' }),
      goEvent({ Action: 'pass', Test: 'TestFoo', Elapsed: 0.1 }),
      goEvent({ Action: 'run', Test: 'TestBar' }),
      goEvent({ Action: 'fail', Test: 'TestBar', Elapsed: 0.2 }),
      goEvent({ Action: 'run', Test: 'TestBaz' }),
      goEvent({ Action: 'skip', Test: 'TestBaz' }),
    ].join('\n');

    const result = parseGoOutput(stdout);
    expect(result.total).toBe(3);
    expect(result.passed).toBe(1);
    expect(result.failed).toBe(1);
    expect(result.skipped).toBe(1);
  });

  it('should extract test_cases with name/status/duration_ms', () => {
    const stdout = [
      goEvent({ Action: 'pass', Test: 'TestFoo', Elapsed: 0.05 }),
      goEvent({ Action: 'fail', Test: 'TestBar', Elapsed: 0.15 }),
    ].join('\n');

    const result = parseGoOutput(stdout);
    expect(result.testCases).toHaveLength(2);

    const foo = result.testCases.find((t) => t.name === 'TestFoo');
    expect(foo?.status).toBe('passed');
    expect(foo?.durationMs).toBe(50); // 0.05 * 1000

    const bar = result.testCases.find((t) => t.name === 'TestBar');
    expect(bar?.status).toBe('failed');
    expect(bar?.durationMs).toBe(150);
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('parseGoOutput -- edge cases', () => {
  it('should return empty result for empty input', () => {
    const result = parseGoOutput('');
    expect(result.total).toBe(0);
    expect(result.error).toBe('Empty stdout');
  });

  it('should ignore Action=run and Action=output lines', () => {
    const stdout = [
      goEvent({ Action: 'run', Test: 'TestFoo' }),
      goEvent({ Action: 'output', Test: 'TestFoo', Output: '=== RUN TestFoo' }),
      goEvent({ Action: 'pass', Test: 'TestFoo' }),
    ].join('\n');

    const result = parseGoOutput(stdout);
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });

  it('should skip non-JSON lines without crashing', () => {
    const stdout = [
      'not json at all',
      '=== RUN TestFoo',
      goEvent({ Action: 'pass', Test: 'TestFoo' }),
    ].join('\n');

    const result = parseGoOutput(stdout);
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });

  it('should handle large output with many test lines', () => {
    const events = Array.from({ length: 500 }, (_, i) =>
      goEvent({ Action: 'pass', Test: `TestLarge${i}`, Elapsed: 0.01 }),
    );
    const stdout = events.join('\n');

    const result = parseGoOutput(stdout);
    expect(result.total).toBe(500);
    expect(result.passed).toBe(500);
  });

  it('should keep the final status when a test name appears multiple times', () => {
    // Go outputs pass then fail for the same test in some cases
    const stdout = [
      goEvent({ Action: 'pass', Test: 'TestFlaky' }),
      goEvent({ Action: 'fail', Test: 'TestFlaky' }),
    ].join('\n');

    const result = parseGoOutput(stdout);
    // Should keep the more recent status (fail)
    const flaky = result.testCases.find((t) => t.name === 'TestFlaky');
    expect(flaky?.status).toBe('failed');
  });

  it('should ignore package-level events without Test field', () => {
    const stdout = [
      goEvent({ Action: 'pass' }), // package-level, no Test
      goEvent({ Action: 'pass', Test: 'TestReal' }),
    ].join('\n');

    const result = parseGoOutput(stdout);
    expect(result.total).toBe(1);
  });
});
