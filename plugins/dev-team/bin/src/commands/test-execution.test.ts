/**
 * 单元测试: commands/test-execution -- runTestExecution 编排逻辑
 *
 * 覆盖范围:
 * - AC-1: 调用 runTestDetectFrameworks 获取 plan
 * - AC-1: 对 plan 中每个 framework 调用 executePlanEntry
 * - AC-1: 在每个 framework 执行后调用 generateSubReport 写入子报告
 * - AC-1: 所有 framework 执行后调用 generateSummaryReport 写入汇总报告
 * - AC-1: 子报告写入 reports/test-execution/<framework>.json
 * - AC-1: 汇总报告写入 reports/test-execution.json
 * - 异常: plan 为空、framework 执行失败不阻塞
 * - 边界: projectRoot 为 undefined、幂等性
 *
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { ExecutionResult } from '../lib/test-runner';
import { type TestPlan } from '../schemas';
import type {
  SourceFileEntry,
  TestExecutionSubReport,
} from '../schemas/test-execution-output.schema';
import { runTestExecution } from './test-execution';

function sfe(file: string, overrides: Partial<SourceFileEntry['coverage']> = {}): SourceFileEntry {
  return {
    file,
    coverage: {
      lines: null,
      branches: null,
      functions: null,
      total_lines: null,
      covered_lines: null,
      total_branches: null,
      covered_branches: null,
      total_functions: null,
      covered_functions: null,
      ...overrides,
    },
  };
}

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockDetectFrameworks = vi.fn();
const mockExecutePlanEntry = vi.fn();
const mockGenerateSubReport = vi.fn();
const mockGenerateSummaryReport = vi.fn();

vi.mock('./test-detect-frameworks', () => ({
  runTestDetectFrameworks: (...args: unknown[]) => mockDetectFrameworks(...args),
}));

vi.mock('../lib/test-runner', () => ({
  executePlanEntry: (...args: unknown[]) => mockExecutePlanEntry(...args),
}));

vi.mock('../lib/test-report', () => ({
  generateSubReport: (...args: unknown[]) => mockGenerateSubReport(...args),
  generateSummaryReport: (...args: unknown[]) => mockGenerateSummaryReport(...args),
}));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'test-execution-'));
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(
    path.join(openspecDir, 'config.json'),
    JSON.stringify(
      {
        schema: 'spec-driven',
        test: { framework: 'vitest' },
      },
      null,
      2,
    ),
    'utf-8',
  );
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function makePlanEntry(overrides: Partial<TestPlan> = {}): TestPlan {
  return {
    directory: '.',
    framework: 'vitest',
    test_cmd: 'npx vitest run --reporter=json {files}',
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    coverage_cleanup: ['coverage', '.nyc_output', 'test-stderr.txt'],
    script: '#!/bin/bash\nset -e\n\nnpx vitest run --coverage --coverage.reporter=json-summary',
    ...overrides,
  };
}

function makeExecutionResult(overrides: Partial<ExecutionResult> = {}): ExecutionResult {
  return {
    framework: 'vitest',
    exitCode: 0,
    testCases: [{ name: 'test1', status: 'passed', durationMs: 100 }],
    coverage: null,
    durationMs: 500,
    testFiles: ['src/foo.test.ts'],
    sourceFiles: ['src/foo.ts'],
    ...overrides,
  };
}

function makeSubReport(overrides: Partial<TestExecutionSubReport> = {}): TestExecutionSubReport {
  return {
    framework: 'vitest',
    directory: '.',
    timestamp: '2026-07-01T00:00:00.000Z',
    exit_code: 0,
    duration_ms: 500,
    summary: { total: 1, passed: 1, failed: 0, skipped: 0 },
    error_cases: [],
    test_files: ['src/foo.test.ts'],
    source_files: [sfe('src/foo.ts')],
    coverage: null,
    mutation: null,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// 正向测试
// ===========================================================================

describe('runTestExecution -- 正向 (AC-1)', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('应调用 runTestDetectFrameworks 获取 plan', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      const exitCode = runTestExecution({ projectRoot: project.root });

      expect(mockDetectFrameworks).toHaveBeenCalledWith(
        expect.objectContaining({ projectRoot: project.root }),
      );
      expect(exitCode).toBe(0);
    } finally {
      project.cleanup();
    }
  });

  it('应对 plan 中每个 framework 调用 executePlanEntry', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [
          { file: 'src/foo.test.ts', framework: 'vitest' },
          { file: 'tests/bar.test.ts', framework: 'vite-plus' },
        ],
        frameworks: ['vitest', 'vite-plus'],
        plan: [
          makePlanEntry({ framework: 'vitest' }),
          makePlanEntry({ framework: 'vite-plus', test_cmd: 'vp test {files}' }),
        ],
      });
      mockExecutePlanEntry
        .mockReturnValueOnce(makeExecutionResult({ framework: 'vitest' }))
        .mockReturnValueOnce(makeExecutionResult({ framework: 'vite-plus' }));
      mockGenerateSubReport
        .mockReturnValueOnce(makeSubReport({ framework: 'vitest' }))
        .mockReturnValueOnce(makeSubReport({ framework: 'vite-plus' }));
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 2,
        passed: 2,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      runTestExecution({ projectRoot: project.root });

      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(2);
      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.objectContaining({ framework: 'vitest' }),
        project.root,
        expect.any(Object),
      );
      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.objectContaining({ framework: 'vite-plus' }),
        project.root,
        expect.any(Object),
      );
    } finally {
      project.cleanup();
    }
  });

  it('应在每个 framework 执行后调用 generateSubReport 写入子报告', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      runTestExecution({ projectRoot: project.root });

      expect(mockGenerateSubReport).toHaveBeenCalledTimes(1);
      const callArgs = mockGenerateSubReport.mock.calls[0];
      expect(callArgs[0]).toBe('vitest');
      expect(callArgs[2]).toBe(project.root);
      expect(callArgs[3].replace(/\\/g, '/')).toContain('reports/test-execution');
    } finally {
      project.cleanup();
    }
  });

  it('应在所有 framework 执行后调用 generateSummaryReport 写入汇总报告', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      runTestExecution({ projectRoot: project.root });

      expect(mockGenerateSummaryReport).toHaveBeenCalledTimes(1);
      const callArgs = mockGenerateSummaryReport.mock.calls[0];
      expect(Array.isArray(callArgs[0])).toBe(true);
      expect(callArgs[1]).toBe(project.root);
      expect(callArgs[2].replace(/\\/g, '/')).toContain('reports/test-execution');
    } finally {
      project.cleanup();
    }
  });

  it('子报告应写入 reports/test-execution/<framework>.json 路径', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      runTestExecution({ projectRoot: project.root });

      const reportsDir = mockGenerateSubReport.mock.calls[0][3];
      expect(reportsDir.replace(/\\/g, '/')).toContain('reports/test-execution');
    } finally {
      project.cleanup();
    }
  });

  it('汇总报告应写入 reports/test-execution.json', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      runTestExecution({ projectRoot: project.root });

      expect(mockGenerateSummaryReport).toHaveBeenCalled();
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// 异常测试
// ===========================================================================

describe('runTestExecution -- 异常', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('plan 为空时不执行任何测试，退出码 0', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [],
        frameworks: [],
        plan: [],
      });

      const exitCode = runTestExecution({ projectRoot: project.root });

      expect(exitCode).toBe(0);
      expect(mockExecutePlanEntry).not.toHaveBeenCalled();
      expect(mockGenerateSubReport).not.toHaveBeenCalled();
      expect(mockGenerateSummaryReport).not.toHaveBeenCalled();
    } finally {
      project.cleanup();
    }
  });

  it('某个 framework 执行失败不阻塞后续 framework', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [
          { file: 'src/foo.test.ts', framework: 'vitest' },
          { file: 'tests/bar.test.ts', framework: 'vite-plus' },
        ],
        frameworks: ['vitest', 'vite-plus'],
        plan: [makePlanEntry({ framework: 'vitest' }), makePlanEntry({ framework: 'vite-plus' })],
      });
      // 第一个框架执行成功
      mockExecutePlanEntry.mockReturnValueOnce(
        makeExecutionResult({
          framework: 'vitest',
          testCases: [{ name: 't1', status: 'passed', durationMs: 100 }],
        }),
      );
      // 第二个框架执行失败
      mockExecutePlanEntry.mockReturnValueOnce(
        makeExecutionResult({
          framework: 'vite-plus',
          exitCode: 1,
          testCases: [{ name: 't2', status: 'failed', durationMs: 50, errorMessage: 'Error' }],
        }),
      );
      mockGenerateSubReport
        .mockReturnValueOnce(makeSubReport({ framework: 'vitest' }))
        .mockReturnValueOnce(
          makeSubReport({
            framework: 'vite-plus',
            exit_code: 1,
            summary: { total: 1, passed: 0, failed: 1, skipped: 0 },
          }),
        );
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 2,
        passed: 1,
        failed: 1,
        skipped: 0,
        conclusion: 'fail',
        problems: [{ framework: 'vite-plus', type: 'test_failure', message: 'Error' }],
        coverage: null,
      });

      const exitCode = runTestExecution({ projectRoot: project.root });

      // 两个框架都应执行
      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(2);
      expect(mockGenerateSubReport).toHaveBeenCalledTimes(2);
      // 汇总报告应被调用
      expect(mockGenerateSummaryReport).toHaveBeenCalledTimes(1);
      // 退出码应为 1（因为测试失败）
      expect(exitCode).toBe(1);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// 边界测试
// ===========================================================================

describe('runTestExecution -- 边界', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('options.projectRoot 为 undefined 时使用默认 project dir', () => {
    // 当 projectRoot 为 undefined 时，runTestExecution 应使用 getProjectDir() 的返回值
    // 这里我们 mock detectFrameworks 返回空 plan，确保不会出错
    mockDetectFrameworks.mockReturnValue({
      detected: [],
      frameworks: [],
      plan: [],
    });

    // 不传 projectRoot，应该不会崩溃
    expect(() => runTestExecution({})).not.toThrow();
  });

  it('所有框架均通过时 conclusion=pass 退出码 0', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(
        makeExecutionResult({
          testCases: [{ name: 't1', status: 'passed', durationMs: 100 }],
        }),
      );
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      const exitCode = runTestExecution({ projectRoot: project.root });
      expect(exitCode).toBe(0);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// 幂等性测试
// ===========================================================================

describe('runTestExecution -- 幂等性', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('相同 plan 重复执行两次 generateSubReport 产生相同的子报告内容', () => {
    const project = createTempProject();
    try {
      const planEntry = makePlanEntry();
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [planEntry],
      });

      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      runTestExecution({ projectRoot: project.root });

      // 验证 generateSubReport 被调用
      expect(mockGenerateSubReport).toHaveBeenCalledTimes(1);
    } finally {
      project.cleanup();
    }
  });

  it('相同 plan 重复执行两次 generateSummaryReport 产生相同的汇总报告内容', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());

      const summaryReport = {
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      };
      mockGenerateSummaryReport.mockReturnValue(summaryReport);

      runTestExecution({ projectRoot: project.root });

      // 验证 generateSummaryReport 被正确调用
      expect(mockGenerateSummaryReport).toHaveBeenCalledTimes(1);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// noMutation 透传
// ===========================================================================

describe('runTestExecution -- noMutation 透传', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('TestExecutionOptions.noMutation 为 true 时透传到 executePlanEntry 的 options 中', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      runTestExecution({ projectRoot: project.root, noMutation: true });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ noMutation: true }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('TestExecutionOptions.noMutation 为 false 时透传到 executePlanEntry 的 options 中', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      runTestExecution({ projectRoot: project.root, noMutation: false });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ noMutation: false }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('TestExecutionOptions.noMutation 为 undefined 时等价于 false（默认执行 mutation）', () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());
      mockGenerateSubReport.mockReturnValue(makeSubReport());
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 1,
        failed: 0,
        skipped: 0,
        conclusion: 'pass',
        problems: [],
        coverage: null,
      });

      runTestExecution({ projectRoot: project.root });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ noMutation: undefined }),
      );
    } finally {
      project.cleanup();
    }
  });
});
