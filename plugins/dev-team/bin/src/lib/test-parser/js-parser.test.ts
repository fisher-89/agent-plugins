/**
 * Tests for lib/test-parser/json-parser -- parses vitest/jest JSON reporter output.
 *
 * Covers:
 * - Parse vitest JSON with testResults/assertionResults
 * - Parse jest JSON format
 * - Extract test cases with name/status/duration_ms
 * - Failed test cases with errorType/errorMessage/stackTrace
 * - Edge: empty stdout, invalid JSON, missing testResults, empty testResults
 * - Boundary: all passed, large output
 *
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { parsePlanArtifacts } from './index';
import { parseJsOutput } from './js-parser';

// ===========================================================================
// Basic parsing
// ===========================================================================

describe('parseJsonOutput -- basic parsing', () => {
  it('should parse vitest JSON with testResults and assertionResults', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'src/foo.test.ts',
          assertionResults: [
            { title: 'test1', fullName: 'suite > test1', status: 'passed', duration: 100 },
            { title: 'test2', fullName: 'suite > test2', status: 'failed', duration: 200 },
            { title: 'test3', fullName: 'suite > test3', status: 'skipped' },
          ],
        },
      ],
    });

    const result = parseJsOutput(stdout);
    expect(result.total).toBe(3);
    expect(result.passed).toBe(1);
    expect(result.failed).toBe(1);
    expect(result.skipped).toBe(1);
    expect(result.testCases).toHaveLength(3);
    expect(result.testCases[0].name).toBe('suite > test1');
    expect(result.testCases[0].status).toBe('passed');
    expect(result.testCases[0].durationMs).toBe(100);
    expect(result.testFiles).toContain('src/foo.test.ts');
  });

  it('should parse jest JSON format', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'src/bar.test.js',
          assertionResults: [
            { title: 'bar test', fullName: 'bar test', status: 'passed', duration: 50 },
          ],
        },
      ],
    });

    const result = parseJsOutput(stdout);
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
    expect(result.testCases[0].name).toBe('bar test');
  });

  it('should extract test_cases with name/status/duration_ms', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'src/test.test.ts',
          assertionResults: [
            { title: 'fast', fullName: 'fast', status: 'passed', duration: 5 },
            { title: 'slow', fullName: 'slow', status: 'passed', duration: 5000 },
          ],
        },
      ],
    });

    const result = parseJsOutput(stdout);
    expect(result.testCases).toHaveLength(2);
    expect(result.testCases[0].durationMs).toBe(5);
    expect(result.testCases[1].durationMs).toBe(5000);
  });

  it('should extract error details from failed test cases', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'src/fail.test.ts',
          assertionResults: [
            {
              title: 'failing test',
              fullName: 'failing test',
              status: 'failed',
              failureMessages: ['AssertionError: Expected true to be false\n    at line 42'],
            },
          ],
        },
      ],
    });

    const result = parseJsOutput(stdout);
    const failed = result.testCases.find((t) => t.status === 'failed');
    expect(failed).toBeDefined();
    expect(failed!.errorType).toBe('AssertionError');
    expect(failed!.errorMessage).toBe('Expected true to be false');
    expect(failed!.stackTrace).toContain('AssertionError: Expected true to be false');
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('parseJsonOutput -- edge cases', () => {
  it('should return error for empty stdout', () => {
    const result = parseJsOutput('');
    expect(result.total).toBe(0);
    expect(result.error).toBe('Empty results content');
  });

  it('should return error for whitespace-only stdout', () => {
    const result = parseJsOutput('   \n  ');
    expect(result.total).toBe(0);
    expect(result.error).toBe('Empty results content');
  });

  it('should return error for invalid JSON', () => {
    const result = parseJsOutput('not json');
    expect(result.total).toBe(0);
    expect(result.error).toBe('Failed to parse JSON output');
  });

  it('should return error for JSON missing testResults array', () => {
    const result = parseJsOutput(JSON.stringify({ foo: 'bar' }));
    expect(result.total).toBe(0);
    expect(result.error).toBe('Missing testResults array');
  });

  it('should return total=0 when testResults array is empty', () => {
    const result = parseJsOutput(JSON.stringify({ testResults: [] }));
    expect(result.total).toBe(0);
    expect(result.passed).toBe(0);
    expect(result.failed).toBe(0);
    expect(result.skipped).toBe(0);
    expect(result.error).toBeUndefined();
  });

  it('should handle large JSON input', () => {
    const assertionResults = Array.from({ length: 100 }, (_, i) => ({
      title: `test${i}`,
      fullName: `suite > test${i}`,
      status: i === 0 ? ('failed' as const) : ('passed' as const),
      duration: i * 10,
    }));

    const stdout = JSON.stringify({
      testResults: [{ name: 'src/large.test.ts', assertionResults }],
    });

    const result = parseJsOutput(stdout);
    expect(result.total).toBe(100);
    expect(result.passed).toBe(99);
    expect(result.failed).toBe(1);
  });

  it('should return all passed when all assertions pass', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'src/all-pass.test.ts',
          assertionResults: [
            { title: 'a', fullName: 'a', status: 'passed' },
            { title: 'b', fullName: 'b', status: 'passed' },
          ],
        },
      ],
    });

    const result = parseJsOutput(stdout);
    expect(result.total).toBe(2);
    expect(result.passed).toBe(2);
    expect(result.failed).toBe(0);
  });

  it('should handle nullish values in assertion results', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'src/nullish.test.ts',
          assertionResults: [
            { title: null, fullName: null, status: 'passed' },
            { title: 'valid', fullName: 'valid', status: 'passed' },
          ],
        },
      ],
    });

    const result = parseJsOutput(stdout);
    // null titles become "unknown"
    expect(result.total).toBe(2);
    expect(result.testCases[0].name).toBe('unknown');
  });
});

// ===========================================================================
// Source file derivation
// ===========================================================================

describe('parseJsonOutput -- source file derivation', () => {
  it('should derive source files from test files', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'src/foo.test.ts',
          assertionResults: [{ title: 'test1', fullName: 'test1', status: 'passed' }],
        },
      ],
    });

    const result = parseJsOutput(stdout);
    expect(result.sourceFiles).toContain('src/foo.ts');
  });

  it('should derive multiple source files', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'src/foo.test.ts',
          assertionResults: [{ title: 't1', fullName: 't1', status: 'passed' }],
        },
        {
          name: 'src/bar.spec.ts',
          assertionResults: [{ title: 't2', fullName: 't2', status: 'passed' }],
        },
      ],
    });

    const result = parseJsOutput(stdout);
    expect(result.sourceFiles).toContain('src/foo.ts');
    expect(result.sourceFiles).toContain('src/bar.ts');
  });
});

// ===========================================================================
// 文件通道（results.json）— AC-4 / AC-11 / AC-12
// ===========================================================================

describe('parseJsOutput / 文件读 — results.json 通道', () => {
  it('合法 vitest/jest results.json 文件内容解析出 passed/failed', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'js-parser-file-'));
    try {
      const payload = JSON.stringify({
        testResults: [
          {
            name: 'a.test.ts',
            assertionResults: [
              { title: 'p', fullName: 'p', status: 'passed' },
              { title: 'f', fullName: 'f', status: 'failed', failureMessages: ['e'] },
            ],
          },
        ],
      });
      fs.writeFileSync(path.join(dir, 'results.json'), payload, 'utf-8');
      const result = parsePlanArtifacts('vitest', dir);
      expect(result.passed).toBe(1);
      expect(result.failed).toBe(1);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('有 coverage-summary 时由横向库解析（本文件专注 results）', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'js-parser-cov-'));
    try {
      const payload = JSON.stringify({
        testResults: [
          {
            name: 'a.test.ts',
            assertionResults: [{ title: 'ok', fullName: 'ok', status: 'passed' }],
          },
        ],
      });
      fs.writeFileSync(path.join(dir, 'results.json'), payload, 'utf-8');
      fs.writeFileSync(
        path.join(dir, 'coverage-summary.json'),
        JSON.stringify({
          total: { lines: { pct: 92 }, branches: { pct: 80 }, functions: { pct: 85 } },
        }),
        'utf-8',
      );
      // results 由 js 通道解析；coverage-summary 由横向 coverage-parser 解析
      const result = parsePlanArtifacts('jest', dir);
      expect(result.passed).toBe(1);
      expect(result.coverage).not.toBeNull();
      expect(result.coverage!.lines).toBe(92);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('非法 JSON 文件返回 error，不抛未捕获异常', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'js-parser-bad-'));
    try {
      fs.writeFileSync(path.join(dir, 'results.json'), '{not json', 'utf-8');
      expect(() => parsePlanArtifacts('jest', dir)).not.toThrow();
      expect(parsePlanArtifacts('jest', dir).error).toBeTruthy();
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('空文件 / 缺失文件', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'js-parser-empty-'));
    try {
      fs.writeFileSync(path.join(dir, 'results.json'), '', 'utf-8');
      expect(parsePlanArtifacts('vitest', dir).error).toBeTruthy();
      fs.rmSync(path.join(dir, 'results.json'));
      expect(parsePlanArtifacts('vitest', dir).error).toBeTruthy();
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('文件通道不受脏 stdout 前缀噪声影响', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'js-parser-noise-'));
    try {
      const clean = JSON.stringify({
        testResults: [
          {
            name: 'a.test.ts',
            assertionResults: [{ title: 'ok', fullName: 'ok', status: 'passed' }],
          },
        ],
      });
      fs.writeFileSync(path.join(dir, 'results.json'), clean, 'utf-8');
      // 脏 stdout 会失败
      expect(parseJsOutput('>>> loading\n' + clean).error).toBeTruthy();
      // 文件通道成功
      expect(parsePlanArtifacts('vitest', dir).passed).toBe(1);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('超大 JSON（>1000 cases）可解析', () => {
    const cases = Array.from({ length: 1001 }, (_, i) => ({
      title: `t${i}`,
      fullName: `t${i}`,
      status: 'passed',
    }));
    const stdout = JSON.stringify({
      testResults: [{ name: 'big.test.ts', assertionResults: cases }],
    });
    const result = parseJsOutput(stdout);
    expect(result.total).toBe(1001);
  });
});
