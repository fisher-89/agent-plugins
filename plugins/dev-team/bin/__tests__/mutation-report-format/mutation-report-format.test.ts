/**
 * 集成测试: mutation 报告格式
 *
 * 覆盖范围:
 * - 子报告 mutation 块结构：pass/score/threshold/measured/by_framework
 * - 汇总报告 mutation 块结构：pass/score/threshold/measured/by_framework/overrides
 * - 变异得分低于阈值时报告 pass=false，conclusion=fail
 * - 不支持框架时子报告和汇总报告 mutation 均为 null
 *
 * @see openspec/changes/unit-test-mutation-testing/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { generateSubReport, generateSummaryReport } from '../../src/lib/test-report';
import type { ExecutionResult } from '../../src/lib/test-runner';

// ===========================================================================
// Helpers
// ===========================================================================

interface TempDir {
  root: string;
  cleanup: () => void;
}

function createTempDir(): TempDir {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mutation-report-'));
  return { root: tmpDir, cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }) };
}

function makeExecutionResult(overrides: Partial<ExecutionResult> = {}): ExecutionResult {
  return {
    framework: 'vitest',
    exitCode: 0,
    stdout: '',
    stderr: '',
    testCases: [{ name: 't1', status: 'passed', durationMs: 100 }],
    coverage: null,
    durationMs: 500,
    testFiles: ['src/foo.test.ts'],
    sourceFiles: ['src/foo.ts'],
    ...overrides,
  };
}

// ===========================================================================
// 正向: 子报告 mutation 块结构
// ===========================================================================

describe('mutation 报告格式 -- 子报告', () => {
  it('子报告 mutation 块结构：pass/score/threshold/measured/by_framework', () => {
    const dir = createTempDir();
    try {
      const sub = generateSubReport(
        'vitest',
        makeExecutionResult({
          mutation: {
            pass: true,
            score: 85.5,
            threshold: 80,
            measured: {
              killed: 10,
              survived: 1,
              timeout: 1,
              noCoverage: 0,
              compileError: 0,
              runtimeError: 0,
              ignored: 0,
              total: 12,
              detected: 11,
              undetected: 1,
            },
            by_framework: {
              vitest: {
                score: 85.5,
                measured: {
                  killed: 10,
                  survived: 1,
                  timeout: 1,
                  noCoverage: 0,
                  compileError: 0,
                  runtimeError: 0,
                  ignored: 0,
                  total: 12,
                  detected: 11,
                  undetected: 1,
                },
                source_files: ['src/foo.ts'],
              },
            },
          },
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(sub.mutation).not.toBeNull();
      expect(typeof sub.mutation!.pass).toBe('boolean');
      expect(typeof sub.mutation!.score).toBe('number');
      expect(typeof sub.mutation!.threshold).toBe('number');
      expect(sub.mutation!.measured).toBeDefined();
      expect(sub.mutation!.by_framework).toBeDefined();
      expect(sub.mutation!.by_framework.vitest).toBeDefined();
      expect(sub.mutation!.by_framework.vitest.score).toBe(85.5);
    } finally {
      dir.cleanup();
    }
  });
});

// ===========================================================================
// 正向: 汇总报告 mutation 块结构
// ===========================================================================

describe('mutation 报告格式 -- 汇总报告', () => {
  it('汇总报告 mutation 块结构：pass/score/threshold/measured/by_framework', () => {
    const dir = createTempDir();
    try {
      const sub = generateSubReport(
        'vitest',
        makeExecutionResult({
          mutation: {
            pass: true,
            score: 85.5,
            threshold: 80,
            measured: {
              killed: 10,
              survived: 1,
              timeout: 1,
              noCoverage: 0,
              compileError: 0,
              runtimeError: 0,
              ignored: 0,
              total: 12,
              detected: 11,
              undetected: 1,
            },
            by_framework: {
              vitest: {
                score: 85.5,
                measured: {
                  killed: 10,
                  survived: 1,
                  timeout: 1,
                  noCoverage: 0,
                  compileError: 0,
                  runtimeError: 0,
                  ignored: 0,
                  total: 12,
                  detected: 11,
                  undetected: 1,
                },
                source_files: ['src/foo.ts'],
              },
            },
          },
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      const summary = generateSummaryReport(
        [sub],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.mutation).not.toBeNull();
      expect(typeof summary.mutation!.pass).toBe('boolean');
      expect(typeof summary.mutation!.score).toBe('number');
      expect(typeof summary.mutation!.threshold).toBe('number');
      expect(summary.mutation!.measured).toBeDefined();
      expect(summary.mutation!.by_framework).toBeDefined();
    } finally {
      dir.cleanup();
    }
  });
});

// ===========================================================================
// 异常: 变异得分低于阈值
// ===========================================================================

describe('mutation 报告格式 -- 得分低于阈值', () => {
  it('变异得分低于阈值时报告 pass=false，conclusion=fail', () => {
    const dir = createTempDir();
    try {
      const sub = generateSubReport(
        'vitest',
        makeExecutionResult({
          mutation: {
            pass: false,
            score: 50,
            threshold: 80,
            measured: {
              killed: 5,
              survived: 5,
              timeout: 0,
              noCoverage: 0,
              compileError: 0,
              runtimeError: 0,
              ignored: 0,
              total: 10,
              detected: 5,
              undetected: 5,
            },
            by_framework: {
              vitest: {
                score: 50,
                measured: {
                  killed: 5,
                  survived: 5,
                  timeout: 0,
                  noCoverage: 0,
                  compileError: 0,
                  runtimeError: 0,
                  ignored: 0,
                  total: 10,
                  detected: 5,
                  undetected: 5,
                },
                source_files: ['src/foo.ts'],
              },
            },
          },
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      const summary = generateSummaryReport(
        [sub],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(sub.mutation!.pass).toBe(false);
      expect(summary.mutation!.pass).toBe(false);
      expect(summary.conclusion).toBe('fail');
    } finally {
      dir.cleanup();
    }
  });
});

// ===========================================================================
// 边界: 不支持框架
// ===========================================================================

describe('mutation 报告格式 -- 不支持框架', () => {
  it('不支持框架时子报告和汇总报告 mutation 均为 null', () => {
    const dir = createTempDir();
    try {
      const result = makeExecutionResult({
        framework: 'bun',
        mutation: null,
      });

      const sub = generateSubReport(
        'bun',
        result,
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(sub.mutation).toBeNull();

      const summary = generateSummaryReport(
        [sub],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.mutation).toBeNull();
    } finally {
      dir.cleanup();
    }
  });
});
