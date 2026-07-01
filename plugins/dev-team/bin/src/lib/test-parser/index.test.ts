/**
 * Tests for lib/test-parser/index -- parseTestOutput dispatch function.
 *
 * Covers:
 * - Dispatch to json-parser for vitest/jest/vite-plus
 * - Dispatch to go-parser for go
 * - Dispatch to text-parser for other frameworks (rust, bun, node-test, pytest)
 * - Edge: empty framework string, unknown framework, case sensitivity
 *
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import { parseTestOutput } from './index';

// ===========================================================================
// Dispatch to json-parser
// ===========================================================================

describe('parseTestOutput -- dispatch to json-parser', () => {
  it('should dispatch vitest to json-parser', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'test.ts',
          assertionResults: [{ title: 'test', fullName: 'test', status: 'passed' }],
        },
      ],
    });

    const result = parseTestOutput(stdout, '', 'vitest');
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });

  it('should dispatch jest to json-parser', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'test.js',
          assertionResults: [{ title: 'test', fullName: 'test', status: 'passed' }],
        },
      ],
    });

    const result = parseTestOutput(stdout, '', 'jest');
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });

  it('should dispatch vite-plus to json-parser', () => {
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'test.ts',
          assertionResults: [{ title: 'test', fullName: 'test', status: 'passed' }],
        },
      ],
    });

    const result = parseTestOutput(stdout, '', 'vite-plus');
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });
});

// ===========================================================================
// Dispatch to go-parser
// ===========================================================================

describe('parseTestOutput -- dispatch to go-parser', () => {
  it('should dispatch go to go-parser', () => {
    const stdout = [JSON.stringify({ Action: 'pass', Test: 'TestFoo' })].join('\n');

    const result = parseTestOutput(stdout, '', 'go');
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });
});

// ===========================================================================
// Dispatch to text-parser (default)
// ===========================================================================

describe('parseTestOutput -- dispatch to text-parser', () => {
  it('should dispatch rust to text-parser', () => {
    const stdout = 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out';
    const result = parseTestOutput(stdout, '', 'rust');
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });

  it('should dispatch bun to text-parser', () => {
    const stdout = '1 [PASS] test_works';
    const result = parseTestOutput(stdout, '', 'bun');
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });

  it('should dispatch node-test to text-parser', () => {
    const stdout = '# pass 1\n# fail 0\n# skip 0';
    const result = parseTestOutput(stdout, '', 'node-test');
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });

  it('should dispatch pytest to text-parser', () => {
    const stdout = 'tests/test_foo.py::test_a PASSED';
    const result = parseTestOutput(stdout, '', 'pytest');
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });
});

// ===========================================================================
// Edge cases
// ===========================================================================

describe('parseTestOutput -- edge cases', () => {
  it('should not throw for unknown framework, fallback to text-parser', () => {
    const result = parseTestOutput('some output', '', 'unknown-framework');
    // Should not throw, text-parser will try its best
    expect(result).toBeDefined();
    expect(result.error).toBeDefined();
  });

  it('should use stderr when stdout is empty for text-parser fallback', () => {
    const result = parseTestOutput('', '1 [PASS] test_from_stderr', 'bun');
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });
});

// ===========================================================================
// Dispatch edge cases
// ===========================================================================

describe('parseTestOutput -- dispatch edge cases', () => {
  it('format 为空字符串时 fallback 到 text-parser', () => {
    const result = parseTestOutput('1 [PASS] test_a', '', '');
    // 空字符串 framework 不会匹配任何特定 parser，应 fallback 到 text-parser
    expect(result).toBeDefined();
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });

  it('format 大小写敏感："JSON" 不匹配 "json"，fallback 到 text-parser', () => {
    // parseTestOutput 使用精确比较，'JSON' !== 'json'
    const stdout = JSON.stringify({
      testResults: [
        {
          name: 'test.ts',
          assertionResults: [{ title: 'test', fullName: 'test', status: 'passed' }],
        },
      ],
    });
    const result = parseTestOutput(stdout, '', 'JSON');
    // JSON 不会匹配 json-parser，应该 fallback 到 text-parser
    // text-parser 可能无法解析 JSON 字符串，所以 error 应被定义
    expect(result).toBeDefined();
  });

  it('未知 format 字符串时不抛出，fallback 到 text-parser', () => {
    const result = parseTestOutput('random text', '', 'non-existent-framework');
    expect(result).toBeDefined();
    // 应该不抛出异常
    expect(() => parseTestOutput('random text', '', 'non-existent-framework')).not.toThrow();
  });
});
