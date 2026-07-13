/**
 * 集成测试: non-zero-exit-code
 *
 * 覆盖范围:
 * - AC-11: 测试命令非零退出时记录失败和退出码
 * - AC-11: 非零退出不阻塞同项目其他 framework 执行
 * - AC-11: 所有 framework 执行完成后退出码反映汇总结果
 *
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { beforeEach, describe, it, expect, vi } from 'vite-plus/test';

import { runTestExecution } from '../../src/commands/test-execution';
import type { ExecutionResult } from '../../src/lib/test-runner';
import type { SourceFileEntry, TestExecutionSubReport, TestPlan } from '../../src/schemas';

function sf(file: string): SourceFileEntry {
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
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'non-zero-exit-'));
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
    testCases: [{ name: 'test1', status: 'passed', durationMs: 100 }],
    coverage: null,
    durationMs: 500,
    testFiles: ['src/foo.test.ts'],
    sourceFiles: ['src/foo.ts'],
    ...overrides,
  };
}

function makeSubReport(
  framework = 'vitest',
  overrides: Record<string, unknown> = {},
): TestExecutionSubReport {
  return {
    framework,
    directory: '.',
    timestamp: '2026-07-01T00:00:00.000Z',
    exit_code: 0,
    duration_ms: 500,
    summary: { total: 1, passed: 1, failed: 0, skipped: 0 },
    error_cases: [],
    test_files: ['src/foo.test.ts'],
    source_files: [sf('src/foo.ts')],
    coverage: null,
    mutation: null,
    ...overrides,
  };
}

// ===========================================================================
// AC-11: 非零退出码
// ===========================================================================

describe('non-zero-exit-code (AC-11)', () => {
  beforeEach(() => {
    mockDetectFrameworks.mockReset();
    mockExecutePlanEntry.mockReset();
    mockGenerateSubReport.mockReset();
    mockGenerateSummaryReport.mockReset();
  });

  it('测试命令非零退出时记录失败和退出码', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(
        makeExecutionResult({
          exitCode: 1,
          testCases: [
            {
              name: 'failing_test',
              status: 'failed',
              durationMs: 50,
              errorMessage: 'AssertionError',
            },
          ],
        }),
      );
      mockGenerateSubReport.mockReturnValue(
        makeSubReport('vitest', {
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
        problems: [{ framework: 'vitest', type: 'test_failure', message: 'Test failed' }],
        coverage: null,
      });

      const exitCode = await runTestExecution({ projectRoot: project.root });

      expect(exitCode).toBe(1);
      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(1);
    } finally {
      project.cleanup();
    }
  });

  it('非零退出不阻塞同项目其他 framework 执行', async () => {
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
      // vitest 失败
      mockExecutePlanEntry.mockReturnValueOnce(
        makeExecutionResult({
          framework: 'vitest',
          exitCode: 1,
          testCases: [{ name: 'fail', status: 'failed', errorMessage: 'Error' }],
        }),
      );
      // vite-plus 成功
      mockExecutePlanEntry.mockReturnValueOnce(
        makeExecutionResult({
          framework: 'vite-plus',
          testCases: [{ name: 'pass', status: 'passed' }],
        }),
      );
      mockGenerateSubReport
        .mockReturnValueOnce(
          makeSubReport('vitest', {
            exit_code: 1,
            summary: { total: 1, passed: 0, failed: 1, skipped: 0 },
          }),
        )
        .mockReturnValueOnce(makeSubReport('vite-plus'));
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
        problems: [{ framework: 'vitest', type: 'test_failure', message: 'Error' }],
        coverage: null,
      });

      const exitCode = await runTestExecution({ projectRoot: project.root });

      // 两个框架都应被执行
      expect(mockExecutePlanEntry).toHaveBeenCalledTimes(2);
      expect(mockGenerateSubReport).toHaveBeenCalledTimes(2);
      // 最终退出码为 1
      expect(exitCode).toBe(1);
    } finally {
      project.cleanup();
    }
  });

  it('所有 framework 执行完成后退出码反映汇总结果', async () => {
    const project = createTempProject();
    try {
      mockDetectFrameworks.mockReturnValue({
        detected: [{ file: 'src/foo.test.ts', framework: 'vitest' }],
        frameworks: ['vitest'],
        plan: [makePlanEntry()],
      });
      mockExecutePlanEntry.mockReturnValue(
        makeExecutionResult({
          testCases: [{ name: 't1', status: 'passed' }],
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

      // 所有测试通过，退出码为 0
      expect(exitCode).toBe(0);
      expect(mockGenerateSummaryReport).toHaveBeenCalledTimes(1);
    } finally {
      project.cleanup();
    }
  });
});
