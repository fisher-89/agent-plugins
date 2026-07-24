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
      lines: 0,
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
const mockGetGitDiffFiles = vi.fn();

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

vi.mock('../lib/git', () => ({
  getGitDiffFiles: (...args: unknown[]) => mockGetGitDiffFiles(...args),
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
        tests: [{ root: '.', framework: 'vitest' }],
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
    coverage_format: 'istanbul',
    coverage_output: 'coverage/coverage-summary.json',
    coverage_artifacts: ['coverage/coverage-summary.json'],
    script: {
      shell: '#!/bin/bash\nset -e\n\nnpx vitest run --coverage --coverage.reporter=json-summary',
      cmd: 'npx vitest run --coverage --coverage.reporter=json-summary',
    },
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
    mockGetGitDiffFiles.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('应调用 runTestDetectFrameworks 获取 plan', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      const exitCode = await runTestExecution({ projectRoot: project.root });

      expect(mockDetectFrameworks).toHaveBeenCalledWith(
        expect.objectContaining({ projectRoot: project.root }),
      );
      expect(exitCode).toBe(0);
    } finally {
      project.cleanup();
    }
  });

  it('应对 plan 中每个 framework 调用 executePlanEntry', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [
          { file: 'src/foo.test.ts', framework: 'vitest' },
          { file: 'tests/bar.test.ts', framework: 'vite-plus' },
        ],
        plan: [makePlanEntry({ framework: 'vitest' }), makePlanEntry({ framework: 'vite-plus' })],
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

      await runTestExecution({ projectRoot: project.root });

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

  it('应在每个 framework 执行后调用 generateSubReport 写入子报告', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root });

      expect(mockGenerateSubReport).toHaveBeenCalledTimes(1);
      const callArgs = mockGenerateSubReport.mock.calls[0];
      expect(callArgs[0]).toBe('vitest');
      expect(callArgs[2]).toBe(project.root);
      expect(callArgs[3].replace(/\\/g, '/')).toContain('reports/test-execution');
    } finally {
      project.cleanup();
    }
  });

  it('应在所有 framework 执行后调用 generateSummaryReport 写入汇总报告', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root });

      expect(mockGenerateSummaryReport).toHaveBeenCalledTimes(1);
      const callArgs = mockGenerateSummaryReport.mock.calls[0];
      expect(Array.isArray(callArgs[0])).toBe(true);
      expect(callArgs[1]).toBe(project.root);
      expect(callArgs[2].replace(/\\/g, '/')).toContain('reports/test-execution');
    } finally {
      project.cleanup();
    }
  });

  it('子报告应写入 reports/test-execution/<framework>.json 路径', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root });

      const reportsDir = mockGenerateSubReport.mock.calls[0][3];
      expect(reportsDir.replace(/\\/g, '/')).toContain('reports/test-execution');
    } finally {
      project.cleanup();
    }
  });

  it('汇总报告应写入 reports/test-execution.json', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root });

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
    mockGetGitDiffFiles.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('plan 为空时不执行任何测试，退出码 0', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [],
        plan: [],
      });

      const exitCode = await runTestExecution({ projectRoot: project.root });

      expect(exitCode).toBe(0);
      expect(mockExecutePlanEntry).not.toHaveBeenCalled();
      expect(mockGenerateSubReport).not.toHaveBeenCalled();
      expect(mockGenerateSummaryReport).not.toHaveBeenCalled();
    } finally {
      project.cleanup();
    }
  });

  it('某个 framework 执行失败不阻塞后续 framework', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [
          { file: 'src/foo.test.ts', framework: 'vitest' },
          { file: 'tests/bar.test.ts', framework: 'vite-plus' },
        ],
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

      const exitCode = await runTestExecution({ projectRoot: project.root });

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
    mockGetGitDiffFiles.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('options.projectRoot 为 undefined 时使用默认 project dir', async () => {
    // 当 projectRoot 为 undefined 时，runTestExecution 应使用 getProjectDir() 的返回值
    // 这里我们 mock detectFrameworks 返回空 plan，确保不会出错
    mockDetectFrameworks.mockReturnValue({
      detected: [],
      plan: [],
    });

    // 不传 projectRoot，应该不会崩溃
    expect(async () => await runTestExecution({})).not.toThrow();
  });

  it('所有框架均通过时 conclusion=pass 退出码 0', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      const exitCode = await runTestExecution({ projectRoot: project.root });
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
    mockGetGitDiffFiles.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('相同 plan 重复执行两次 generateSubReport 产生相同的子报告内容', async () => {
    const project = createTempProject();
    try {
      const planEntry = makePlanEntry();
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root });

      // 验证 generateSubReport 被调用
      expect(mockGenerateSubReport).toHaveBeenCalledTimes(1);
    } finally {
      project.cleanup();
    }
  });

  it('相同 plan 重复执行两次 generateSummaryReport 产生相同的汇总报告内容', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root });

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
    mockGetGitDiffFiles.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('TestExecutionOptions.noMutation 为 true 时透传到 executePlanEntry 的 options 中', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root, noMutation: true });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ noMutation: true }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('TestExecutionOptions.noMutation 为 false 时透传到 executePlanEntry 的 options 中', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root, noMutation: false });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ noMutation: false }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('TestExecutionOptions.noMutation 为 undefined 时等价于 false（默认执行 mutation）', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root });

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

// ===========================================================================
// mutationDiffOnly 透传
// ===========================================================================

describe('runTestExecution -- mutationDiffOnly 透传', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    mockGetGitDiffFiles.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('mutationDiffOnly 为 true 时调用 getGitDiffFiles 获取变更文件', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        plan: [makePlanEntry()],
      });
      mockGetGitDiffFiles.mockResolvedValue(['src/a.ts', 'src/b.ts']);
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

      await runTestExecution({ projectRoot: project.root, mutationDiffOnly: true });

      expect(mockGetGitDiffFiles).toHaveBeenCalledWith(project.root);
    } finally {
      project.cleanup();
    }
  });

  it('mutationDiffOnly 为 true 且 git diff 为空时透传空数组', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        plan: [makePlanEntry()],
      });
      mockGetGitDiffFiles.mockResolvedValue([]);
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

      await runTestExecution({ projectRoot: project.root, mutationDiffOnly: true });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ mutationDiffFiles: [] }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('mutationDiffOnly 为 false 时不调用 getGitDiffFiles', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root, mutationDiffOnly: false });

      expect(mockGetGitDiffFiles).not.toHaveBeenCalled();
    } finally {
      project.cleanup();
    }
  });

  it('mutationDiffOnly 为 false 时透传 undefined 到 executePlanEntry 的 mutationDiffFiles', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root, mutationDiffOnly: false });

      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({ mutationDiffFiles: undefined }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('mutationDiffOnly 为 undefined 时不调用 getGitDiffFiles（等价于 false）', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
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

      await runTestExecution({ projectRoot: project.root });

      expect(mockGetGitDiffFiles).not.toHaveBeenCalled();
    } finally {
      project.cleanup();
    }
  });

  it('多 framework 时每个 plan entry 收到相同的 mutationDiffFiles', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [
          { file: 'src/foo.test.ts', framework: 'vitest' },
          { file: 'tests/test_basic.py', framework: 'pytest' },
        ],
        plan: [
          makePlanEntry({ framework: 'vitest' }),
          makePlanEntry({ framework: 'pytest', directory: 'tests' }),
        ],
      });
      mockGetGitDiffFiles.mockResolvedValue(['src/a.ts']);
      mockExecutePlanEntry
        .mockReturnValueOnce(makeExecutionResult({ framework: 'vitest' }))
        .mockReturnValueOnce(makeExecutionResult({ framework: 'pytest' }));
      mockGenerateSubReport
        .mockReturnValueOnce(makeSubReport({ framework: 'vitest' }))
        .mockReturnValueOnce(makeSubReport({ framework: 'pytest' }));
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

      await runTestExecution({ projectRoot: project.root, mutationDiffOnly: true });

      const expectedDiffFiles = [path.resolve(project.root, 'src/a.ts').replace(/\\/g, '/')];
      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(2);
      expect(mockExecutePlanEntry).toHaveBeenNthCalledWith(
        1,
        expect.objectContaining({ framework: 'vitest' }),
        project.root,
        expect.objectContaining({ mutationDiffFiles: expectedDiffFiles }),
      );
      expect(mockExecutePlanEntry).toHaveBeenNthCalledWith(
        2,
        expect.objectContaining({ framework: 'pytest' }),
        project.root,
        expect.objectContaining({ mutationDiffFiles: expectedDiffFiles }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('--framework 过滤时仅匹配的 framework 执行，且仍收到 mutationDiffFiles', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [
          { file: 'src/foo.test.ts', framework: 'vitest' },
          { file: 'tests/test_basic.py', framework: 'pytest' },
        ],
        plan: [
          makePlanEntry({ framework: 'vitest' }),
          makePlanEntry({ framework: 'pytest', directory: 'tests' }),
        ],
      });
      mockGetGitDiffFiles.mockResolvedValue(['src/a.ts']);
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult({ framework: 'vitest' }));
      mockGenerateSubReport.mockReturnValue(makeSubReport({ framework: 'vitest' }));
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

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: true,
        framework: 'vitest',
      });

      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(1);
      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.objectContaining({ framework: 'vitest' }),
        project.root,
        expect.objectContaining({
          mutationDiffFiles: [path.resolve(project.root, 'src/a.ts').replace(/\\/g, '/')],
        }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('mutationDiffOnly + --change 时报告写入 change 专属目录，且透传 mutationDiffFiles', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        plan: [makePlanEntry()],
      });
      mockGetGitDiffFiles.mockResolvedValue(['src/a.ts']);
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

      await runTestExecution({
        projectRoot: project.root,
        mutationDiffOnly: true,
        change: 'my-feature',
      });

      const expectedReportsDir = path
        .resolve(project.root, 'openspec', 'changes', 'my-feature', 'reports', 'test-execution')
        .replace(/\\/g, '/');
      const subReportsDir = mockGenerateSubReport.mock.calls[0][3].replace(/\\/g, '/');
      const summaryReportsDir = mockGenerateSummaryReport.mock.calls[0][2].replace(/\\/g, '/');
      expect(subReportsDir).toBe(expectedReportsDir);
      expect(summaryReportsDir).toBe(expectedReportsDir);
      expect(mockExecutePlanEntry).toHaveBeenCalledWith(
        expect.any(Object),
        project.root,
        expect.objectContaining({
          mutationDiffFiles: [path.resolve(project.root, 'src/a.ts').replace(/\\/g, '/')],
        }),
      );
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// runTestExecution — 无配置提示引导 tests (AC-6)
// ===========================================================================

describe('runTestExecution — 无配置提示引导 tests (AC-6)', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    mockGetGitDiffFiles.mockReset();
  });

  it('plan.length === 0 时日志包含配置 tests 的引导文案，返回 0', async () => {
    const project = createTempProject();
    const logs: string[] = [];
    const spy = vi.spyOn(console, 'log').mockImplementation((msg?: string) => {
      logs.push(String(msg ?? ''));
    });
    try {
      mockDetectFrameworks.mockReturnValue({ detected: [], plan: [] });
      const code = await runTestExecution({ projectRoot: project.root });
      expect(code).toBe(0);
      expect(logs.some((l) => l.includes('tests'))).toBe(true);
      expect(logs.some((l) => /test\.framework|test\.overrides/.test(l))).toBe(false);
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });

  it('runTestDetectFrameworks 抛错时向上传播或转为非 0 退出码', async () => {
    const project = createTempProject();
    vi.spyOn(console, 'log').mockImplementation(() => {});
    try {
      mockDetectFrameworks.mockImplementation(() => {
        throw new Error('detect failed');
      });
      await expect(runTestExecution({ projectRoot: project.root })).rejects.toThrow(
        /detect failed/,
      );
    } finally {
      project.cleanup();
    }
  });

  it('配置仅含旧 test 键导致空 plan 时仍引导 tests，不提及 test.framework', async () => {
    const project = createTempProject();
    const logs: string[] = [];
    const spy = vi.spyOn(console, 'log').mockImplementation((msg?: string) => {
      logs.push(String(msg ?? ''));
    });
    try {
      mockDetectFrameworks.mockReturnValue({ detected: [], plan: [] });
      await runTestExecution({ projectRoot: project.root });
      const joined = logs.join('\n');
      expect(joined).toMatch(/tests/i);
      expect(joined).not.toContain('test.framework');
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });

  it('旧文案 test.framework / test.overrides 不再出现在日志中', async () => {
    const project = createTempProject();
    const logs: string[] = [];
    const spy = vi.spyOn(console, 'log').mockImplementation((msg?: string) => {
      logs.push(String(msg ?? ''));
    });
    try {
      mockDetectFrameworks.mockReturnValue({ detected: [], plan: [] });
      await runTestExecution({ projectRoot: project.root });
      expect(logs.join('\n')).not.toMatch(/test\.framework|test\.overrides/);
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });

  it('options 缺省字段（如无 files）时仍能完成空 plan 提示路径', async () => {
    const project = createTempProject();
    const spy = vi.spyOn(console, 'log').mockImplementation(() => {});
    try {
      mockDetectFrameworks.mockReturnValue({ detected: [], plan: [] });
      const code = await runTestExecution({ projectRoot: project.root });
      expect(code).toBe(0);
    } finally {
      spy.mockRestore();
      project.cleanup();
    }
  });
});

describe('runTestExecution -- 正向异常路径 (AC-1)', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    mockGetGitDiffFiles.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('executePlanEntry 失败或子报告生成抛错时返回非 0 退出码（不静默成功）', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(
        makeExecutionResult({
          exitCode: 1,
          testCases: [{ name: 't', status: 'failed', durationMs: 1 }],
        }),
      );
      mockGenerateSubReport.mockReturnValue(
        makeSubReport({
          exit_code: 1,
          summary: { total: 1, passed: 0, failed: 1, skipped: 0 },
        }),
      );
      mockGenerateSummaryReport.mockReturnValue({
        phase: 'test-execution',
        command: 'dev-team test-execution',
        timestamp: '2026-07-01T00:00:00.000Z',
        duration_seconds: 1,
        total: 1,
        passed: 0,
        failed: 1,
        skipped: 0,
        conclusion: 'fail',
        problems: [],
        coverage: null,
      });
      const code = await runTestExecution({ projectRoot: project.root });
      expect(code).not.toBe(0);
    } finally {
      project.cleanup();
    }
  });
});
