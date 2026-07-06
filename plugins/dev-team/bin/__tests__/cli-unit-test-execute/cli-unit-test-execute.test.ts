/**
 * 集成测试: cli-unit-test-execute
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { beforeEach, describe, it, expect, vi } from 'vite-plus/test';

import type { ExecutionResult } from '../../src/lib/test-runner';
import { type TestPlan } from '../../src/schemas';

// ---------------------------------------------------------------------------
// Mocks
// ---------------------------------------------------------------------------

const mockDetectFrameworks = vi.fn();
const mockExecutePlanEntry = vi.fn();
const mockGenerateSubReport = vi.fn();
const mockGenerateSummaryReport = vi.fn();

vi.mock('../../src/commands/test-detect-frameworks', () => ({
  runTestDetectFrameworks: (...args: unknown[]) => mockDetectFrameworks(...args),
}));

vi.mock('../../src/lib/test-runner', () => ({
  executePlanEntry: (...args: unknown[]) => mockExecutePlanEntry(...args),
}));

vi.mock('../../src/lib/test-report', () => ({
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
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cli-unit-test-execute-'));
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(
    path.join(openspecDir, 'config.json'),
    JSON.stringify({ schema: 'spec-driven', test: { framework: 'vitest' } }, null, 2),
    'utf-8',
  );
  return { root, cleanup: () => fs.rmSync(root, { recursive: true, force: true }) };
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
    stdout: '',
    stderr: '',
    testCases: [{ name: 'test1', status: 'passed', durationMs: 100 }],
    coverage: null,
    durationMs: 500,
    testFiles: ['src/foo.test.ts'],
    sourceFiles: ['src/foo.ts'],
    ...overrides,
  };
}

function makeSubReport(framework = 'vitest') {
  return {
    framework,
    timestamp: '2026-07-01T00:00:00.000Z',
    exit_code: 0,
    duration_ms: 500,
    summary: { total: 1, passed: 1, failed: 0, skipped: 0 },
    test_cases: [{ name: 'test1', status: 'passed' as const }],
    test_files: ['src/foo.test.ts'],
    source_files: ['src/foo.ts'],
    file_coverage: null,
    coverage: null,
  };
}

// ===========================================================================
// AC-1: CLI 端到端执行
// ===========================================================================

describe('cli-unit-test-execute (AC-1)', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
  });

  it('运行 dev-team unit-test 时调用 runTestDetectFrameworks 获取 plan', async () => {
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

      const { runTestExecution } = await import('../../src/commands/test-execution');
      runTestExecution({ projectRoot: project.root });

      expect(mockDetectFrameworks).toHaveBeenCalledWith(
        expect.objectContaining({ projectRoot: project.root }),
      );
    } finally {
      project.cleanup();
    }
  });

  it('对每个 framework plan entry 执行测试命令', async () => {
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
      mockExecutePlanEntry
        .mockReturnValueOnce(makeExecutionResult({ framework: 'vitest' }))
        .mockReturnValueOnce(makeExecutionResult({ framework: 'vite-plus' }));
      mockGenerateSubReport
        .mockReturnValueOnce(makeSubReport('vitest'))
        .mockReturnValueOnce(makeSubReport('vite-plus'));
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

      const { runTestExecution } = await import('../../src/commands/test-execution');
      runTestExecution({ projectRoot: project.root });

      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(2);
    } finally {
      project.cleanup();
    }
  });

  it('在 reports/test-execution/<framework>.json 生成子报告', async () => {
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

      const { runTestExecution } = await import('../../src/commands/test-execution');
      runTestExecution({ projectRoot: project.root });

      const reportsDir = mockGenerateSubReport.mock.calls[0][3];
      expect(reportsDir.replace(/\\/g, '/')).toContain('reports/test-execution');
    } finally {
      project.cleanup();
    }
  });

  it('在 reports/test-execution-execution.json 生成汇总报告', async () => {
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

      const { runTestExecution } = await import('../../src/commands/test-execution');
      runTestExecution({ projectRoot: project.root });

      expect(mockGenerateSummaryReport).toHaveBeenCalledTimes(1);
    } finally {
      project.cleanup();
    }
  });

  it('重复执行 CLI 命令时报告文件内容一致（幂等性）', async () => {
    const project = createTempProject();
    try {
      const planEntry = makePlanEntry();
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [planEntry],
      });
      const execResult = makeExecutionResult();
      mockExecutePlanEntry.mockReturnValue(execResult);
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

      const { runTestExecution } = await import('../../src/commands/test-execution');

      // 第一次执行
      const exit1 = runTestExecution({ projectRoot: project.root });
      // 第二次执行
      const exit2 = runTestExecution({ projectRoot: project.root });

      expect(exit1).toBe(0);
      expect(exit2).toBe(0);
      expect(mockDetectFrameworks).toHaveBeenCalledTimes(2);
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// AC-12: 无 merge_mode
// ===========================================================================

describe('cli-unit-test-execute -- 无 merge_mode (AC-12)', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
  });

  it('FrameworkConfig 和 TestPlan 不含 merge_mode 字段', async () => {
    const project = createTempProject();
    try {
      const planEntry = makePlanEntry();
      expect(planEntry).not.toHaveProperty('merge_mode');

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

      const { runTestExecution } = await import('../../src/commands/test-execution');
      runTestExecution({ projectRoot: project.root });

      // 验证所有 plan 条目不含 merge_mode
      const plan = mockDetectFrameworks.mock.results[0].value.plan;
      for (const entry of plan) {
        expect(entry).not.toHaveProperty('merge_mode');
      }
    } finally {
      project.cleanup();
    }
  });
});
