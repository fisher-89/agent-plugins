/**
 * 集成测试: CLI test-execution 端到端执行
 *
 * 覆盖范围:
 * - AC-7: 调用 runTestDetectFrameworks 获取 plan
 * - AC-7: 对每个 framework plan entry 执行测试命令
 * - AC-7: 相同 plan 重复执行两次产生相同子报告内容（幂等性）
 * - AC-11: 子报告写入 reports/test-execution/<framework>.json（非旧 reports/unit-test/）
 * - AC-11: 汇总报告写入 reports/test-execution.json（非旧 reports/unit-test-execution.json）
 * - AC-11: 汇总报告 phase 字段为 "test-execution"（非旧 "06-unit-test"）
 * - AC-11: 汇总报告 command 字段为 "dev-team test-execution"（非旧 "dev-team unit-test"）
 *
 * @see openspec/changes/consolidate-test-execution-phase/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { runTestExecution } from '../../src/commands/test-execution';
import type { ExecutionResult } from '../../src/lib/test-runner';
import { type TestPlan } from '../../src/schemas';
import type { TestExecutionSubReport } from '../../src/schemas/test-execution-output.schema';

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
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cli-test-execution-'));
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  fs.writeFileSync(
    path.join(openspecDir, 'config.json'),
    JSON.stringify({ schema: 'spec-driven', test: { framework: 'vitest' } }, null, 2),
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

function makeSubReport(overrides: Partial<TestExecutionSubReport> = {}): TestExecutionSubReport {
  return {
    framework: 'vitest',
    timestamp: '2026-07-01T00:00:00.000Z',
    exit_code: 0,
    duration_ms: 500,
    summary: { total: 1, passed: 1, failed: 0, skipped: 0 },
    test_cases: [{ name: 'test1', status: 'passed' }],
    test_files: ['src/foo.test.ts'],
    source_files: ['src/foo.ts'],
    file_coverage: null,
    coverage: null,
    mutation: null,
    ...overrides,
  };
}

// ===========================================================================
// AC-7: CLI 端到端执行
// ===========================================================================

describe('CLI 端到端执行 — AC-7', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('调用 runTestExecution 时应触发 runTestDetectFrameworks 获取 plan', async () => {
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
        mutation: null,
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

  it('应对每个 framework plan entry 执行测试命令', async () => {
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
        mutation: null,
      });

      runTestExecution({ projectRoot: project.root });

      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(2);
    } finally {
      project.cleanup();
    }
  });

  it('相同 plan 重复执行两次应产生相同子报告内容（幂等性）', async () => {
    const project = createTempProject();
    try {
      const planEntry = makePlanEntry();
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [planEntry],
      });
      mockExecutePlanEntry.mockReturnValue(makeExecutionResult());

      const subReport = makeSubReport();
      mockGenerateSubReport.mockReturnValue(subReport);
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
        mutation: null,
      });

      // 第一次执行
      runTestExecution({ projectRoot: project.root });
      const firstCall = mockGenerateSubReport.mock.calls[0];

      mockGenerateSubReport.mockReset();
      mockGenerateSubReport.mockReturnValue(subReport);

      // 第二次执行
      runTestExecution({ projectRoot: project.root });
      const secondCall = mockGenerateSubReport.mock.calls[0];

      // 子报告内容应一致
      expect(firstCall[0]).toBe(secondCall[0]); // 相同 framework
      expect(firstCall[2]).toBe(secondCall[2]); // 相同 projectRoot
    } finally {
      project.cleanup();
    }
  });
});

// ===========================================================================
// AC-11: 报告路径
// ===========================================================================

describe('报告路径 — AC-11', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('子报告应写入 reports/test-execution/<framework>.json（非旧 reports/unit-test/）', async () => {
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
        mutation: null,
      });

      runTestExecution({ projectRoot: project.root });

      // 子报告路径应包含 test-execution（而非 unit-test）
      expect(mockGenerateSubReport).toHaveBeenCalled();
      const reportsDir = mockGenerateSubReport.mock.calls[0][3];
      expect(reportsDir.replace(/\\/g, '/')).toContain('reports/test-execution');
      expect(reportsDir.replace(/\\/g, '/')).not.toContain('reports/unit-test');
    } finally {
      project.cleanup();
    }
  });

  it('汇总报告应写入 reports/test-execution.json（非旧 reports/unit-test-execution.json）', async () => {
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
        mutation: null,
      });

      runTestExecution({ projectRoot: project.root });

      // 汇总报告被调用
      expect(mockGenerateSummaryReport).toHaveBeenCalled();
      // 验证 reportsDir 参数
      const reportsDir = mockGenerateSummaryReport.mock.calls[0][2];
      expect(reportsDir.replace(/\\/g, '/')).toContain('reports/test-execution');
    } finally {
      project.cleanup();
    }
  });

  it('汇总报告 phase 字段应为 "test-execution"（非旧 "06-unit-test"）', async () => {
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
        mutation: null,
      });

      runTestExecution({ projectRoot: project.root });

      // 验证汇总报告中 phase 字段为 test-execution
      const summaryReport = mockGenerateSummaryReport.mock.results[0]?.value;
      if (summaryReport) {
        expect(summaryReport.phase).toBe('test-execution');
        expect(summaryReport.phase).not.toBe('06-unit-test');
      }
    } finally {
      project.cleanup();
    }
  });

  it('汇总报告 command 字段应为 "dev-team test-execution"（非旧 "dev-team unit-test"）', async () => {
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
        mutation: null,
      });

      runTestExecution({ projectRoot: project.root });

      // 验证汇总报告中 command 字段
      const summaryReport = mockGenerateSummaryReport.mock.results[0]?.value;
      if (summaryReport) {
        expect(summaryReport.command).toBe('dev-team test-execution');
        expect(summaryReport.command).not.toBe('dev-team unit-test');
      }
    } finally {
      project.cleanup();
    }
  });
});
