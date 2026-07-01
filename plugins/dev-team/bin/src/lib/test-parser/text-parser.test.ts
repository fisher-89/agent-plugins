/**
 * Tests for lib/test-parser/text-parser -- multi-layer fallback text parser.
 *
 * Covers:
 * - bun: [PASS] / [FAIL] / [SKIP] markers
 * - cargo: "test result:" summary line
 * - node: "# pass" / "# fail" / "# skip" lines
 * - pytest: PASSED/FAILED/SKIPPED markers
 * - Layer 2: generic regex fallback
 * - Layer 3: heuristic line-by-line
 * - Edge: empty output, unknown format
 *
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import { parseTextOutput } from './text-parser';

// ===========================================================================
// Bun output
// ===========================================================================

describe('parseTextOutput -- bun output', () => {
  it('should parse bun [PASS] / [FAIL] / [SKIP] markers', () => {
    const output = [
      '1 [PASS] test 1 works',
      '2 [FAIL] test 2 fails',
      '3 [SKIP] test 3 skipped',
    ].join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(3);
    expect(result.passed).toBe(1);
    expect(result.failed).toBe(1);
    expect(result.skipped).toBe(1);
  });

  it('should handle all passing bun tests', () => {
    const output = ['1 [PASS] test a', '2 [PASS] test b'].join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(2);
    expect(result.passed).toBe(2);
    expect(result.failed).toBe(0);
    expect(result.skipped).toBe(0);
  });
});

// ===========================================================================
// Cargo output
// ===========================================================================

describe('parseTextOutput -- cargo test output', () => {
  it('should parse cargo test result: ok summary', () => {
    const output = [
      'test test_foo ... ok',
      'test test_bar ... ok',
      'test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out',
    ].join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(2);
    expect(result.passed).toBe(2);
    expect(result.failed).toBe(0);
    expect(result.skipped).toBe(0);
  });

  it('should parse cargo test result: FAILED summary', () => {
    const output = [
      'test test_foo ... ok',
      'test test_bar ... FAILED',
      'test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out',
    ].join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(2);
    expect(result.passed).toBe(1);
    expect(result.failed).toBe(1);
    expect(result.skipped).toBe(0);
  });
});

// ===========================================================================
// Node output
// ===========================================================================

describe('parseTextOutput -- node test output', () => {
  it('should parse node --test # pass / # fail / # skip lines', () => {
    const output = ['ok 1 test1', 'not ok 2 test2', '# pass 1', '# fail 1', '# skip 0'].join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(2);
    expect(result.passed).toBe(1);
    expect(result.failed).toBe(1);
    expect(result.skipped).toBe(0);
  });

  it('should handle all passing node tests', () => {
    const output = ['ok 1 test_a', 'ok 2 test_b', '# pass 2', '# fail 0', '# skip 0'].join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(2);
    expect(result.passed).toBe(2);
    expect(result.failed).toBe(0);
  });
});

// ===========================================================================
// Pytest output
// ===========================================================================

describe('parseTextOutput -- pytest output', () => {
  it('should parse pytest verbose PASSED/FAILED/SKIPPED markers', () => {
    const output = [
      'tests/test_foo.py::test_a PASSED',
      'tests/test_foo.py::test_b FAILED',
      'tests/test_foo.py::test_c SKIPPED',
    ].join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(3);
    expect(result.passed).toBe(1);
    expect(result.failed).toBe(1);
    expect(result.skipped).toBe(1);
  });
});

// ===========================================================================
// Generic regex fallback (Layer 2)
// ===========================================================================

describe('parseTextOutput -- generic regex fallback', () => {
  it('should parse "Tests: N passed, M failed, S total" format', () => {
    const output = 'Tests: 10 passed, 2 failed, 12 total';

    const result = parseTextOutput(output);
    // The third number in this pattern is "total" (not skipped), so:
    // total = 10 + 2 + 12 = 24
    expect(result.total).toBe(24);
    expect(result.passed).toBe(10);
    expect(result.failed).toBe(2);
  });

  it('should parse "N passed, M failed" format', () => {
    const output = '5 passed, 1 failed';

    const result = parseTextOutput(output);
    expect(result.total).toBe(6);
    expect(result.passed).toBe(5);
    expect(result.failed).toBe(1);
  });

  it('should parse "N tests passed" format', () => {
    const output = '42 tests passed';

    const result = parseTextOutput(output);
    expect(result.total).toBe(42);
    expect(result.passed).toBe(42);
    expect(result.failed).toBe(0);
  });
});

// ===========================================================================
// Heuristic fallback (Layer 3)
// ===========================================================================

describe('parseTextOutput -- heuristic fallback', () => {
  it('should count PASSED/FAILED lines heuristically', () => {
    const output = ['test_a PASSED', 'test_b FAILED', 'test_c SKIPPED'].join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(3);
    expect(result.passed).toBe(1);
    expect(result.failed).toBe(1);
    expect(result.skipped).toBe(1);
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('parseTextOutput -- edge cases', () => {
  it('should return error for empty output', () => {
    const result = parseTextOutput('');
    expect(result.total).toBe(0);
    expect(result.error).toBe('Empty output');
  });

  it('should return error for whitespace-only output', () => {
    const result = parseTextOutput('   \n  ');
    expect(result.total).toBe(0);
    expect(result.error).toBe('Empty output');
  });

  it('should return empty result for unknown format (no recognizable patterns)', () => {
    const result = parseTextOutput('some random output\nthat means nothing');
    expect(result.total).toBe(0);
    expect(result.error).toBe('Unable to parse test output');
  });

  it('should not throw on malformed input', () => {
    expect(() => parseTextOutput('foo\nbar\nbaz')).not.toThrow();
  });

  it('should handle output with only passed tests (no skipped)', () => {
    const output = ['1 [PASS] test ok', '2 [PASS] test ok2'].join('\n');

    const result = parseTextOutput(output);
    expect(result.passed).toBe(2);
    expect(result.skipped).toBe(0);
    expect(result.failed).toBe(0);
  });

  it('should handle output with only failed tests (no passed)', () => {
    const output = ['tests/test.py::test_a FAILED', 'tests/test.py::test_b FAILED'].join('\n');

    const result = parseTextOutput(output);
    expect(result.failed).toBe(2);
    expect(result.passed).toBe(0);
  });

  it('should handle mixed pass/fail/skip', () => {
    const output = [
      '1 [PASS] test_a',
      '2 [FAIL] test_b',
      '3 [SKIP] test_c',
      '4 [PASS] test_d',
    ].join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(4);
    expect(result.passed).toBe(2);
    expect(result.failed).toBe(1);
    expect(result.skipped).toBe(1);
  });

  it('should handle very long output', () => {
    const lines = Array.from({ length: 1000 }, (_, i) => `${i} [PASS] test_${i}`);
    const output = lines.join('\n');

    const result = parseTextOutput(output);
    expect(result.total).toBe(1000);
    expect(result.passed).toBe(1000);
  });
});
