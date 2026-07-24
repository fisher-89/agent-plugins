/**
 * 集成测试: mutation 报告格式 / suite 阈值无全局级联
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, expect, it } from 'vite-plus/test';

import { generateSubReport, generateSummaryReport } from '../../src/lib/test-report';
import type { ExecutionResult } from '../../src/lib/test-runner';
import type { SourceFileEntry, TestExecutionSubReport } from '../../src/schemas';

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

function writeConfig(root: string, config: Record<string, unknown>): void {
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(path.join(openspecDir, 'config.json'), JSON.stringify(config, null, 2), 'utf-8');
}

function sfe(
  file: string,
  overrides: Partial<NonNullable<SourceFileEntry['coverage']>> = {},
): SourceFileEntry {
  return {
    file,
    coverage: {
      lines: 100,
      branches: 100,
      functions: 100,
      total_lines: 10,
      covered_lines: 10,
      total_branches: 10,
      covered_branches: 10,
      total_functions: 10,
      covered_functions: 10,
      ...overrides,
    },
  };
}

function makeExecutionResult(overrides: Partial<ExecutionResult> = {}): ExecutionResult {
  return {
    framework: 'vitest',
    exitCode: 0,
    testCases: [{ name: 't1', status: 'passed', durationMs: 100 }],
    coverage: null,
    durationMs: 500,
    testFiles: ['src/foo.test.ts'],
    sourceFiles: ['src/foo.ts'],
    ...overrides,
  };
}

function makeMutationMeasured(
  overrides: Partial<{
    killed: number;
    survived: number;
    timeout: number;
    noCoverage: number;
    compileError: number;
    runtimeError: number;
    ignored: number;
    total: number;
    detected: number;
    undetected: number;
  }> = {},
) {
  return {
    killed: 8,
    survived: 2,
    timeout: 0,
    noCoverage: 0,
    compileError: 0,
    runtimeError: 0,
    ignored: 0,
    total: 10,
    detected: 8,
    undetected: 2,
    ...overrides,
  };
}

function createSubReport(overrides: Partial<TestExecutionSubReport> = {}): TestExecutionSubReport {
  return {
    framework: 'vitest',
    directory: 'src',
    timestamp: new Date().toISOString(),
    exit_code: 0,
    duration_ms: 100,
    summary: { total: 1, passed: 1, failed: 0, skipped: 0 },
    error_cases: [],
    test_files: ['src/foo.test.ts'],
    source_files: [sfe('src/foo.ts')],
    coverage: null,
    mutation: null,
    ...overrides,
  };
}

// ===========================================================================
// 正向: 子报告 mutation 块结构
// ===========================================================================

describe('mutation 报告格式 -- 子报告', () => {
  it('子报告 mutation 块结构：pass/score/threshold/measured', () => {
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
          },
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
        '.',
      );
      expect(sub.mutation).not.toBeNull();
      expect(typeof sub.mutation!.pass).toBe('boolean');
      expect(typeof sub.mutation!.score).toBe('number');
      expect(typeof sub.mutation!.threshold).toBe('number');
      expect(sub.mutation!.measured).toBeDefined();
    } finally {
      dir.cleanup();
    }
  });
});

// ===========================================================================
// 正向: 汇总报告 mutation 块结构
// ===========================================================================

describe('mutation 报告格式 -- 汇总报告', () => {
  it('汇总报告 mutation 块结构：pass/score/threshold/measured', () => {
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
          },
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
        '.',
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
          },
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
        '.',
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
        '.',
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

// ===========================================================================
// 场景: 多 suite 阈值无全局级联 (AC-5)
// ===========================================================================

describe('多 suite 阈值无全局级联', () => {
  it('suite A/B 自定义阈值分别出现在汇总分组中', () => {
    const dir = createTempDir();
    try {
      writeConfig(dir.root, {
        schema: 'spec-driven',
        tests: [
          {
            root: 'pkg-a',
            framework: 'vitest',
            // 源码树入报告分组需显式 includes（省略时仅 default_glob）
            includes: ['**/*.{ts,tsx}'],
            coverage: { lines: 90, branches: 90, functions: 90 },
            mutation: { score: 90 },
          },
          {
            root: 'pkg-b',
            framework: 'vite-plus',
            includes: ['**/*.{ts,tsx}'],
            coverage: { lines: 60, branches: 60, functions: 60 },
            mutation: { score: 50 },
          },
        ],
      });

      const subA = createSubReport({
        framework: 'vitest',
        directory: 'pkg-a',
        source_files: [sfe('pkg-a/a.ts')],
        coverage: {
          pass: true,
          measured: { lines: 100, branches: 100, functions: 100 },
          thresholds: { lines: 90, branches: 90, functions: 90 },
        },
        mutation: {
          pass: true,
          score: 95,
          threshold: 90,
          measured: makeMutationMeasured({ killed: 9, survived: 1, detected: 9, undetected: 1 }),
        },
      });
      const subB = createSubReport({
        framework: 'vite-plus',
        directory: 'pkg-b',
        source_files: [sfe('pkg-b/b.ts')],
        coverage: {
          pass: true,
          measured: { lines: 100, branches: 100, functions: 100 },
          thresholds: { lines: 60, branches: 60, functions: 60 },
        },
        mutation: {
          pass: true,
          score: 80,
          threshold: 50,
          measured: makeMutationMeasured(),
        },
      });

      const summary = generateSummaryReport(
        [subA, subB],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      const covA = summary.coverage?.overrides?.find((o) => o.glob === 'pkg-a');
      const covB = summary.coverage?.overrides?.find((o) => o.glob === 'pkg-b');
      expect(covA?.thresholds).toEqual({ lines: 90, branches: 90, functions: 90 });
      expect(covB?.thresholds).toEqual({ lines: 60, branches: 60, functions: 60 });

      const mutA = summary.mutation?.overrides?.find((o) => o.glob === 'pkg-a');
      const mutB = summary.mutation?.overrides?.find((o) => o.glob === 'pkg-b');
      expect(mutA?.threshold).toBe(90);
      expect(mutB?.threshold).toBe(50);
    } finally {
      dir.cleanup();
    }
  });

  it('省略 suite 阈值时使用 schema 默认，且不依赖任何顶层全局块', () => {
    const dir = createTempDir();
    try {
      // 省略 includes → suite scope 回落框架 default_glob；用 *.test.ts 验证分组仍可用默认阈值
      writeConfig(dir.root, {
        schema: 'spec-driven',
        tests: [{ root: 'src', framework: 'vitest' }],
      });

      const result = makeExecutionResult({
        coverage: { lines: 90, branches: 90, functions: 90, fileCoverage: null },
        sourceFiles: ['src/foo.ts'],
        mutation: {
          pass: true,
          score: 80,
          threshold: 70,
          measured: makeMutationMeasured(),
        },
      });
      const sub = generateSubReport(
        'vitest',
        result,
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
        'src',
      );
      expect(sub.coverage?.thresholds).toEqual({ lines: 80, branches: 70, functions: 75 });

      // 源文件不匹配 default_glob → 无 suite 分组
      const summarySource = generateSummaryReport(
        [sub],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summarySource.mutation?.overrides?.find((o) => o.glob === 'src')).toBeUndefined();

      const subTest = createSubReport({
        framework: 'vitest',
        directory: 'src',
        source_files: [sfe('src/foo.test.ts')],
        mutation: {
          pass: true,
          score: 80,
          threshold: 70,
          measured: makeMutationMeasured(),
        },
      });
      const summary = generateSummaryReport(
        [subTest],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      const mutGroup = summary.mutation?.overrides?.find((o) => o.glob === 'src');
      expect(mutGroup?.threshold).toBe(70);
    } finally {
      dir.cleanup();
    }
  });

  it('suite 阈值为 -1 时配置解析失败，报告流程不采用非法值', () => {
    const dir = createTempDir();
    try {
      writeConfig(dir.root, {
        schema: 'spec-driven',
        tests: [
          {
            root: 'src',
            framework: 'vitest',
            coverage: { lines: -1, branches: -1, functions: -1 },
            mutation: { score: -1 },
          },
        ],
      });

      const result = makeExecutionResult({
        coverage: { lines: 90, branches: 90, functions: 90, fileCoverage: null },
        sourceFiles: ['src/foo.ts'],
      });
      const sub = generateSubReport(
        'vitest',
        result,
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
        'src',
      );
      // invalid config → readConfig 回落 schema 默认，不采用 -1
      expect(sub.coverage?.thresholds.lines).toBe(80);
      expect(sub.coverage?.thresholds.lines).not.toBe(-1);
    } finally {
      dir.cleanup();
    }
  });

  it('suite 阈值为 0 / 100 时汇总分组使用对应边界值', () => {
    const dir = createTempDir();
    try {
      writeConfig(dir.root, {
        schema: 'spec-driven',
        tests: [
          {
            root: 'pkg-zero',
            framework: 'vitest',
            includes: ['**/*.{ts,tsx}'],
            coverage: { lines: 0, branches: 0, functions: 0 },
            mutation: { score: 0 },
          },
          {
            root: 'pkg-full',
            framework: 'vite-plus',
            includes: ['**/*.{ts,tsx}'],
            coverage: { lines: 100, branches: 100, functions: 100 },
            mutation: { score: 100 },
          },
        ],
      });

      const subZero = createSubReport({
        framework: 'vitest',
        directory: 'pkg-zero',
        source_files: [
          sfe('pkg-zero/a.ts', { lines: 0, branches: 0, functions: 0, covered_lines: 0 }),
        ],
        coverage: {
          pass: true,
          measured: { lines: 0, branches: 0, functions: 0 },
          thresholds: { lines: 0, branches: 0, functions: 0 },
        },
        mutation: {
          pass: true,
          score: 50,
          threshold: 0,
          measured: makeMutationMeasured({
            killed: 1,
            survived: 1,
            total: 2,
            detected: 1,
            undetected: 1,
          }),
        },
      });
      const subFull = createSubReport({
        framework: 'vite-plus',
        directory: 'pkg-full',
        source_files: [sfe('pkg-full/b.ts')],
        coverage: {
          pass: true,
          measured: { lines: 100, branches: 100, functions: 100 },
          thresholds: { lines: 100, branches: 100, functions: 100 },
        },
        mutation: {
          pass: true,
          score: 100,
          threshold: 100,
          measured: makeMutationMeasured({
            killed: 10,
            survived: 0,
            detected: 10,
            undetected: 0,
          }),
        },
      });

      const summary = generateSummaryReport(
        [subZero, subFull],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      expect(
        summary.coverage?.overrides?.find((o) => o.glob === 'pkg-zero')?.thresholds.lines,
      ).toBe(0);
      expect(
        summary.coverage?.overrides?.find((o) => o.glob === 'pkg-full')?.thresholds.lines,
      ).toBe(100);
      expect(summary.mutation?.overrides?.find((o) => o.glob === 'pkg-zero')?.threshold).toBe(0);
      expect(summary.mutation?.overrides?.find((o) => o.glob === 'pkg-full')?.threshold).toBe(100);
    } finally {
      dir.cleanup();
    }
  });

  it('仅残留旧 test.coverage 键时报告不采用该值作为正式阈值来源', () => {
    const dir = createTempDir();
    try {
      writeConfig(dir.root, {
        schema: 'spec-driven',
        // 旧全局键：pasthrough 保留但报告层不得级联读取
        test: {
          coverage: { lines: 99, branches: 99, functions: 99 },
          mutation: { score: 99 },
        },
        // 省略 includes：分组仅匹配 default_glob；用 *.test.ts 验证阈值仍来自 suite/schema
        tests: [{ root: 'src', framework: 'vitest' }],
      });

      const result = makeExecutionResult({
        coverage: { lines: 90, branches: 90, functions: 90, fileCoverage: null },
        sourceFiles: ['src/foo.ts'],
        mutation: {
          pass: true,
          score: 80,
          threshold: 70,
          measured: makeMutationMeasured(),
        },
      });
      const sub = generateSubReport(
        'vitest',
        result,
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
        'src',
      );
      expect(sub.coverage?.thresholds).toEqual({ lines: 80, branches: 70, functions: 75 });
      expect(sub.coverage?.thresholds.lines).not.toBe(99);

      const subTest = createSubReport({
        framework: 'vitest',
        directory: 'src',
        source_files: [sfe('src/foo.test.ts')],
        mutation: {
          pass: true,
          score: 80,
          threshold: 70,
          measured: makeMutationMeasured(),
        },
      });
      const summary = generateSummaryReport(
        [subTest],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      const mutGroup = summary.mutation?.overrides?.find((o) => o.glob === 'src');
      expect(mutGroup?.threshold).toBe(70);
      expect(mutGroup?.threshold).not.toBe(99);
    } finally {
      dir.cleanup();
    }
  });
});
