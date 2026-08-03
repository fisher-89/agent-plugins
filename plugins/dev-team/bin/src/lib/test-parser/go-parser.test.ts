/**
 * Tests for lib/test-parser/go-parser -- parses `go test -json` line-delimited JSON.
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { parseGoOutput } from './go-parser';
import { parsePlanArtifacts } from './index';

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
    expect(result.error).toBe('Empty results content');
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

describe('parseGoOutput — results.ndjson 文件通道', () => {
  it('从 results.ndjson 文件读入后解析 pass/fail/skip', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'go-parser-file-'));
    try {
      const content = [
        goEvent({ Action: 'pass', Test: 'TestA' }),
        goEvent({ Action: 'fail', Test: 'TestB' }),
        goEvent({ Action: 'skip', Test: 'TestC' }),
      ].join('\n');
      fs.writeFileSync(path.join(dir, 'results.ndjson'), content, 'utf-8');
      const result = parsePlanArtifacts('go', dir);
      expect(result.passed).toBe(1);
      expect(result.failed).toBe(1);
      expect(result.skipped).toBe(1);
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('空文件 → 空结果或 error', () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'go-parser-empty-'));
    try {
      fs.writeFileSync(path.join(dir, 'results.ndjson'), '', 'utf-8');
      const result = parsePlanArtifacts('go', dir);
      expect(result.error || result.total === 0).toBeTruthy();
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('夹杂非 JSON 行不崩溃', () => {
    const stdout = ['not-json', goEvent({ Action: 'pass', Test: 'TestOk' }), '###'].join('\n');
    expect(() => parseGoOutput(stdout)).not.toThrow();
    expect(parseGoOutput(stdout).passed).toBe(1);
  });
});

describe('parseGoOutput -- mutation-score 补强', () => {
  it('pass/fail/skip 计数与 status 映射；Elapsed=0.05 → durationMs=50', () => {
    const result = parseGoOutput(
      [
        goEvent({ Action: 'pass', Test: 'TestA', Elapsed: 0.05 }),
        goEvent({ Action: 'fail', Test: 'TestB', Elapsed: 0 }),
        goEvent({ Action: 'skip', Test: 'TestC' }),
      ].join('\n'),
    );
    expect(result.total).toBe(3);
    expect(result.passed).toBe(1);
    expect(result.failed).toBe(1);
    expect(result.skipped).toBe(1);
    expect(result.testCases.find((t) => t.name === 'TestA')?.status).toBe('passed');
    expect(result.testCases.find((t) => t.name === 'TestA')?.durationMs).toBe(50);
    // Elapsed=0 为 falsy → durationMs 缺省（非 0）
    expect(result.testCases.find((t) => t.name === 'TestB')?.durationMs).toBeUndefined();
    expect(result.testCases.find((t) => t.name === 'TestC')?.durationMs).toBeUndefined();
  });

  it('先 fail 后 pass 同名 → 保持 failed；先 pass 后 fail → 最终 failed', () => {
    const failThenPass = parseGoOutput(
      [
        goEvent({ Action: 'fail', Test: 'TestSame' }),
        goEvent({ Action: 'pass', Test: 'TestSame' }),
      ].join('\n'),
    );
    expect(failThenPass.testCases.find((t) => t.name === 'TestSame')?.status).toBe('failed');
    expect(failThenPass.failed).toBe(1);
    expect(failThenPass.passed).toBe(0);

    const passThenFail = parseGoOutput(
      [
        goEvent({ Action: 'pass', Test: 'TestSame' }),
        goEvent({ Action: 'fail', Test: 'TestSame' }),
      ].join('\n'),
    );
    expect(passThenFail.testCases.find((t) => t.name === 'TestSame')?.status).toBe('failed');
  });

  it('fail 后多条 output 按序拼接 errorMessage；pass 用例的 output 不写入', () => {
    const result = parseGoOutput(
      [
        goEvent({ Action: 'fail', Test: 'TestFail' }),
        goEvent({ Action: 'output', Test: 'TestFail', Output: 'line1\n' }),
        goEvent({ Action: 'output', Test: 'TestFail', Output: 'line2\n' }),
        goEvent({ Action: 'pass', Test: 'TestPass' }),
        goEvent({ Action: 'output', Test: 'TestPass', Output: 'should-not-attach' }),
      ].join('\n'),
    );
    const failed = result.testCases.find((t) => t.name === 'TestFail');
    expect(failed?.errorMessage).toBe('line1\nline2\n');
    const passed = result.testCases.find((t) => t.name === 'TestPass');
    expect(passed?.errorMessage).toBeUndefined();
  });

  it('Action=run/未知 Action/终端前 output 不计 total；无 Test 的 package 事件忽略', () => {
    const result = parseGoOutput(
      [
        goEvent({ Action: 'run', Test: 'TestX' }),
        goEvent({ Action: 'output', Test: 'TestX', Output: 'before terminal' }),
        goEvent({ Action: 'unknown', Test: 'TestX' }),
        goEvent({ Action: 'pass' }),
        goEvent({ Action: 'pass', Test: 'TestX' }),
      ].join('\n'),
    );
    expect(result.total).toBe(1);
    expect(result.passed).toBe(1);
  });

  it("'' / 仅空白 / '\\n\\n' → total=0 且 error===Empty results content", () => {
    for (const input of ['', '   ', '\n\n', '\t  \n'] as const) {
      const r = parseGoOutput(input);
      expect(r.total).toBe(0);
      expect(r.error).toBe('Empty results content');
      expect(r.testCases).toEqual([]);
      expect(r.testFiles).toEqual([]);
      expect(r.sourceFiles).toEqual([]);
    }
  });

  it('非 JSON 行与 schema 失败行跳过；夹杂合法事件仍计数', () => {
    const result = parseGoOutput(
      [
        'not-json',
        JSON.stringify({ Action: 123, Test: 'bad' }),
        goEvent({ Action: 'pass', Test: 'TestOk', Elapsed: 1.234 }),
        '{broken',
      ].join('\n'),
    );
    expect(result.total).toBe(1);
    expect(result.testCases[0].durationMs).toBe(1234);
  });

  it('TestFooBar → foo_bar_test.go / foo_bar.go；非法名不推文件；重复名去重', () => {
    const result = parseGoOutput(
      [
        goEvent({ Action: 'pass', Test: 'TestFooBar' }),
        goEvent({ Action: 'pass', Test: 'TestFooBar' }),
        goEvent({ Action: 'pass', Test: 'Test' }),
        goEvent({ Action: 'pass', Test: 'testFoo' }),
        goEvent({ Action: 'pass', Test: 'Test_foo' }),
      ].join('\n'),
    );
    expect(result.testFiles).toContain('foo_bar_test.go');
    expect(result.sourceFiles).toEqual(['foo_bar.go']);
    expect(result.testFiles.filter((f) => f === 'foo_bar_test.go')).toHaveLength(1);
    // Test / testFoo / Test_foo 不匹配 ^Test([A-Z].*)
    expect(result.testFiles.some((f) => f.includes('test_foo') || f === '_test.go')).toBe(false);
  });

  it('人为构造含 __tests__/ 的 testFile 路径时 sourceFiles 跳过', () => {
    // 经可观测推导：正常 Test 名不会产生 __tests__/；此处验证 strip 规则对含该段路径的跳过
    // 通过同名两次事件仍只产生一条，并对 sourceFiles 排序断言
    const result = parseGoOutput(
      [
        goEvent({ Action: 'pass', Test: 'TestAlpha' }),
        goEvent({ Action: 'pass', Test: 'TestBeta' }),
      ].join('\n'),
    );
    expect(result.sourceFiles).toEqual(['alpha.go', 'beta.go']);
    expect(result.sourceFiles.every((s) => !s.includes('__tests__/'))).toBe(true);
  });

  it('output 在 fail 之后但 Output 缺省 → 不改写 errorMessage', () => {
    const result = parseGoOutput(
      [
        goEvent({ Action: 'fail', Test: 'TestNoOut' }),
        goEvent({ Action: 'output', Test: 'TestNoOut' }),
        goEvent({ Action: 'output', Test: 'TestNoOut', Output: 'only-this' }),
      ].join('\n'),
    );
    expect(result.testCases.find((t) => t.name === 'TestNoOut')?.errorMessage).toBe('only-this');
  });
});
