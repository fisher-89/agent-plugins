/**
 * Tests for lib/test-report -- sub-report and summary report generation.
 *
 * Covers:
 * - generateSummaryReport: aggregation, conclusion, problems
 * - generateSummaryReport: coverage weighted average, threshold pass/fail
 * - generateSummaryReport: null dimensions, edge cases
 * - generateSubReport: basic structure, coverage block
 * - generateSubReport: exit_code passthrough, empty test_cases
 * - generateSummaryReport: coverage threshold pass/fail logic
 * - generateSummaryReport: weighted average by source_files.length
 * - Idempotency: same input produces same output
 *
 * @see openspec/changes/cli-unit-test-execute/test-design.md
 */

import * as fs from 'fs';
import * as os from 'os';
import * as path from 'path';

import { describe, it, expect } from 'vite-plus/test';

import type { UnitTestSubReport } from '../schemas/unit-test-output.schema';
import { generateSubReport, generateSummaryReport } from './test-report';
import type { ExecutionResult } from './test-runner';

// ===========================================================================
// Helpers
// ===========================================================================

function createTempDir(): { root: string; cleanup: () => void } {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'test-report-'));
  return {
    root: tmpDir,
    cleanup: () => fs.rmSync(tmpDir, { recursive: true, force: true }),
  };
}

function makeExecutionResult(overrides: Partial<ExecutionResult> = {}): ExecutionResult {
  return {
    framework: 'vitest',
    exitCode: 0,
    stdout: '',
    stderr: '',
    testCases: [
      { name: 'test1', status: 'passed', durationMs: 100 },
      { name: 'test2', status: 'failed', durationMs: 200, errorMessage: 'Error: fail' },
    ],
    coverage: null,
    durationMs: 500,
    testFiles: ['src/foo.test.ts'],
    sourceFiles: ['src/foo.ts'],
    ...overrides,
  };
}

// ===========================================================================
// generateSummaryReport -- aggregation
// ===========================================================================

describe('generateSummaryReport -- aggregation', () => {
  it('should aggregate totals from multiple sub-reports', () => {
    const dir = createTempDir();
    try {
      // Create two sub-reports manually
      const sub1 = generateSubReport(
        'vitest',
        makeExecutionResult({
          framework: 'vitest',
          testCases: [
            { name: 't1', status: 'passed', durationMs: 100 },
            { name: 't2', status: 'passed', durationMs: 200 },
          ],
          durationMs: 300,
          sourceFiles: ['src/foo.ts'],
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      const sub2 = generateSubReport(
        'jest',
        makeExecutionResult({
          framework: 'jest',
          testCases: [{ name: 't3', status: 'failed', durationMs: 150 }],
          durationMs: 200,
          sourceFiles: ['src/bar.ts'],
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      const summary = generateSummaryReport(
        [sub1, sub2],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      expect(summary.total).toBe(3);
      expect(summary.passed).toBe(2);
      expect(summary.failed).toBe(1);
      expect(summary.skipped).toBe(0);
      expect(summary.duration_seconds).toBe(1); // 500ms rounds to 1s
    } finally {
      dir.cleanup();
    }
  });

  it('should set conclusion to "fail" when tests fail', () => {
    const dir = createTempDir();
    try {
      const sub = generateSubReport(
        'vitest',
        makeExecutionResult({
          testCases: [{ name: 't1', status: 'failed', errorMessage: 'fail' }],
          sourceFiles: ['src/foo.ts'],
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      const summary = generateSummaryReport(
        [sub],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.conclusion).toBe('fail');
    } finally {
      dir.cleanup();
    }
  });

  it('should set conclusion to "pass" when all tests pass', () => {
    const dir = createTempDir();
    try {
      const sub = generateSubReport(
        'vitest',
        makeExecutionResult({
          testCases: [{ name: 't1', status: 'passed' }],
          exitCode: 0,
          sourceFiles: ['src/foo.ts'],
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      const summary = generateSummaryReport(
        [sub],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.conclusion).toBe('pass');
    } finally {
      dir.cleanup();
    }
  });

  it('should include problems for failed tests', () => {
    const dir = createTempDir();
    try {
      const sub = generateSubReport(
        'vitest',
        makeExecutionResult({
          testCases: [
            { name: 't1', status: 'failed', errorMessage: 'AssertionError: expected true' },
          ],
          sourceFiles: ['src/foo.ts'],
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      const summary = generateSummaryReport(
        [sub],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.problems.length).toBeGreaterThan(0);
      expect(summary.problems[0].type).toBe('test_failure');
    } finally {
      dir.cleanup();
    }
  });

  it('should handle empty sub-reports list gracefully', () => {
    const dir = createTempDir();
    try {
      const summary = generateSummaryReport(
        [],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.total).toBe(0);
      expect(summary.conclusion).toBe('pass');
    } finally {
      dir.cleanup();
    }
  });
});

// ===========================================================================
// generateSummaryReport -- coverage
// ===========================================================================

describe('generateSummaryReport -- coverage', () => {
  it('should set coverage=null when no sub-reports have coverage', () => {
    const dir = createTempDir();
    try {
      const sub = generateSubReport(
        'vitest',
        makeExecutionResult({
          coverage: null,
          sourceFiles: ['src/foo.ts'],
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      const summary = generateSummaryReport(
        [sub],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).toBeNull();
    } finally {
      dir.cleanup();
    }
  });

  it('should compute weighted average coverage across frameworks', () => {
    const dir = createTempDir();
    try {
      // Sub-report with coverage data
      const subWithCoverage = generateSubReport(
        'vitest',
        makeExecutionResult({
          coverage: {
            lines: 90,
            branches: 85,
            functions: 95,
            fileCoverage: null,
          },
          sourceFiles: ['src/foo.ts', 'src/bar.ts'],
        }),
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );

      // Need to write coverage report manually since generateSubReport
      // uses result.coverage which comes from parseCoverageFromFile
      // Let's create a sub-report directly
      const subReport = {
        ...subWithCoverage,
        coverage: {
          pass: true,
          measured: { lines: 90, branches: 85, functions: 95 },
          thresholds: { lines: 80, branches: 70, functions: 75 },
          by_framework: {
            vitest: {
              measured: { lines: 90, branches: 85, functions: 95 },
              source_files: ['src/foo.ts', 'src/bar.ts'],
            },
          },
        },
      };

      const summary = generateSummaryReport(
        [subReport],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      expect(summary.coverage!.pass).toBe(true);
      expect(summary.coverage!.measured.lines).toBe(90);
    } finally {
      dir.cleanup();
    }
  });
});

// ===========================================================================
// generateSubReport
// ===========================================================================

describe('generateSubReport', () => {
  it('should create a sub-report with correct fields', () => {
    const dir = createTempDir();
    try {
      const result = makeExecutionResult({
        testCases: [{ name: 'test1', status: 'passed', durationMs: 100 }],
        sourceFiles: ['src/foo.ts'],
        testFiles: ['src/foo.test.ts'],
      });

      const report = generateSubReport(
        'vitest',
        result,
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(report.framework).toBe('vitest');
      expect(report.exit_code).toBe(0);
      expect(report.summary.total).toBe(1);
      expect(report.summary.passed).toBe(1);
      expect(report.test_cases).toHaveLength(1);
      expect(report.test_files).toContain('src/foo.test.ts');
      expect(report.source_files).toContain('src/foo.ts');
    } finally {
      dir.cleanup();
    }
  });

  it('should write the report file to disk', () => {
    const dir = createTempDir();
    try {
      const result = makeExecutionResult();
      const reportsDir = path.join(dir.root, 'reports', 'unit-test');

      generateSubReport('vitest', result, dir.root, reportsDir);

      const filePath = path.join(reportsDir, 'vitest.json');
      expect(fs.existsSync(filePath)).toBe(true);

      const content = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
      expect(content.framework).toBe('vitest');
    } finally {
      dir.cleanup();
    }
  });

  it('should include coverage=null when no coverage data', () => {
    const dir = createTempDir();
    try {
      const result = makeExecutionResult({ coverage: null });
      const report = generateSubReport(
        'vitest',
        result,
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(report.coverage).toBeNull();
    } finally {
      dir.cleanup();
    }
  });

  it('exit_code 非零时正确传递', () => {
    const dir = createTempDir();
    try {
      const result = makeExecutionResult({
        exitCode: 1,
        testCases: [{ name: 'test1', status: 'failed', durationMs: 100, errorMessage: 'Error' }],
        sourceFiles: ['src/foo.ts'],
        testFiles: ['src/foo.test.ts'],
      });

      const report = generateSubReport(
        'vitest',
        result,
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(report.exit_code).toBe(1);
      expect(report.summary.failed).toBe(1);
    } finally {
      dir.cleanup();
    }
  });

  it('0 个 test_cases 时 summary.total=0', () => {
    const dir = createTempDir();
    try {
      const result = makeExecutionResult({
        testCases: [],
        sourceFiles: ['src/foo.ts'],
        testFiles: [],
      });

      const report = generateSubReport(
        'vitest',
        result,
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(report.summary.total).toBe(0);
      expect(report.summary.passed).toBe(0);
      expect(report.summary.failed).toBe(0);
      expect(report.summary.skipped).toBe(0);
      expect(report.test_cases).toEqual([]);
    } finally {
      dir.cleanup();
    }
  });

  it('大量 test_cases（>1000）时报告正确生成', () => {
    const dir = createTempDir();
    try {
      const testCases = Array.from({ length: 1001 }, (_, i) => ({
        name: `test${i}`,
        status: 'passed' as const,
        durationMs: i,
      }));
      const result = makeExecutionResult({
        testCases,
        sourceFiles: ['src/foo.ts'],
        testFiles: ['src/foo.test.ts'],
      });

      const report = generateSubReport(
        'vitest',
        result,
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(report.summary.total).toBe(1001);
      expect(report.summary.passed).toBe(1001);
      expect(report.test_cases).toHaveLength(1001);
    } finally {
      dir.cleanup();
    }
  });
});

// Helper to create a minimal valid UnitTestSubReport
function createSubReport(overrides: Partial<UnitTestSubReport> = {}): UnitTestSubReport {
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
    ...overrides,
  };
}

// ===========================================================================
// generateSummaryReport -- coverage threshold
// ===========================================================================

describe('generateSummaryReport -- coverage threshold (AC-9)', () => {
  it('coverage.measured >= coverage.thresholds 时 pass=true', () => {
    const dir = createTempDir();
    try {
      const subReport = createSubReport({
        coverage: {
          pass: true,
          measured: { lines: 90, branches: 85, functions: 95 },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: 90, branches: 85, functions: 95 },
              source_files: ['src/foo.ts'],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [subReport],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      expect(summary.coverage!.pass).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('任一维度 measured < thresholds 时 pass=false', () => {
    const dir = createTempDir();
    try {
      const subReport = createSubReport({
        coverage: {
          pass: false,
          measured: { lines: 70, branches: 85, functions: 95 },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: 70, branches: 85, functions: 95 },
              source_files: ['src/foo.ts'],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [subReport],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      expect(summary.coverage!.pass).toBe(false);
    } finally {
      dir.cleanup();
    }
  });

  it('measured 某维度为 null 时跳过该维度判定', () => {
    const dir = createTempDir();
    try {
      const subReport = createSubReport({
        coverage: {
          pass: true,
          measured: { lines: 90, branches: null, functions: null },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: 90, branches: null, functions: null },
              source_files: ['src/foo.ts'],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [subReport],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      expect(summary.coverage!.pass).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('所有 measured 维度均为 null 时 pass=true（跳过所有维度判定）', () => {
    const dir = createTempDir();
    try {
      const subReport = createSubReport({
        coverage: {
          pass: true,
          measured: { lines: null, branches: null, functions: null },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: null, branches: null, functions: null },
              source_files: ['src/foo.ts'],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [subReport],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      expect(summary.coverage!.pass).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('measured 某维度恰好等于 threshold（如 80.0 == 80.0）时 pass=true', () => {
    const dir = createTempDir();
    try {
      const subReport = createSubReport({
        coverage: {
          pass: true,
          measured: { lines: 80, branches: 80, functions: 80 },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: 80, branches: 80, functions: 80 },
              source_files: ['src/foo.ts'],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [subReport],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      expect(summary.coverage!.pass).toBe(true);
    } finally {
      dir.cleanup();
    }
  });

  it('thresholds 对象缺失或 undefined 时不崩溃', () => {
    const dir = createTempDir();
    try {
      // 创建一个包含 coverage 但缺少 thresholds 的子报告
      const subReport = createSubReport({
        coverage: {
          pass: true,
          measured: { lines: 90, branches: 85, functions: 95 },
          // @ts-expect-error - intentionally omitting thresholds to test robustness
          thresholds: undefined,
          by_framework: {
            vitest: {
              measured: { lines: 90, branches: 85, functions: 95 },
              source_files: ['src/foo.ts'],
            },
          },
        },
      });

      // 不应抛出异常
      expect(() => {
        const summary = generateSummaryReport(
          [subReport],
          dir.root,
          path.join(dir.root, 'reports', 'unit-test'),
        );
        // 即使 thresholds 为 undefined，仍能正常生成报告
        expect(summary.coverage).not.toBeNull();
      }).not.toThrow();
    } finally {
      dir.cleanup();
    }
  });
});

// ===========================================================================
// generateSummaryReport -- coverage weighted average
// ===========================================================================

describe('generateSummaryReport -- coverage weighted average', () => {
  it('加权平均各维度按 source_files.length 加权', () => {
    const dir = createTempDir();
    try {
      const sub1 = createSubReport({
        framework: 'vitest',
        coverage: {
          pass: true,
          measured: { lines: 90, branches: 85, functions: 95 },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: 90, branches: 85, functions: 95 },
              source_files: ['src/foo.ts'],
            },
          },
        },
      });
      const sub2 = createSubReport({
        framework: 'vite-plus',
        coverage: {
          pass: true,
          measured: { lines: 80, branches: 75, functions: 85 },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            'vite-plus': {
              measured: { lines: 80, branches: 75, functions: 85 },
              source_files: ['src/bar.ts'],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [sub1, sub2],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      // 加权平均: (90*1 + 80*1) / (1+1) = 85
      expect(summary.coverage!.measured.lines).toBe(85);
    } finally {
      dir.cleanup();
    }
  });

  it('某框架某维度为 null 时不参与该维度加权平均', () => {
    const dir = createTempDir();
    try {
      const sub1 = createSubReport({
        framework: 'vitest',
        coverage: {
          pass: true,
          measured: { lines: 90, branches: null, functions: 95 },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: 90, branches: null, functions: 95 },
              source_files: ['src/foo.ts'],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [sub1],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      expect(summary.coverage!.measured.branches).toBeNull();
    } finally {
      dir.cleanup();
    }
  });

  it('所有框架某维度均为 null 时总体该维度为 null', () => {
    const dir = createTempDir();
    try {
      const sub1 = createSubReport({
        framework: 'vitest',
        coverage: {
          pass: true,
          measured: { lines: null, branches: null, functions: null },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: null, branches: null, functions: null },
              source_files: ['src/foo.ts'],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [sub1],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      expect(summary.coverage!.measured.lines).toBeNull();
    } finally {
      dir.cleanup();
    }
  });

  it('所有框架 source_files 等长时退化为简单平均', () => {
    const dir = createTempDir();
    try {
      const sub1 = createSubReport({
        framework: 'vitest',
        coverage: {
          pass: true,
          measured: { lines: 90, branches: 85, functions: 95 },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: 90, branches: 85, functions: 95 },
              source_files: ['src/foo.ts', 'src/bar.ts'],
            },
          },
        },
      });
      const sub2 = createSubReport({
        framework: 'vite-plus',
        coverage: {
          pass: true,
          measured: { lines: 80, branches: 75, functions: 85 },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            'vite-plus': {
              measured: { lines: 80, branches: 75, functions: 85 },
              source_files: ['src/baz.ts', 'src/qux.ts'],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [sub1, sub2],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).not.toBeNull();
      // 等权平均: (90 + 80) / 2 = 85
      expect(summary.coverage!.measured.lines).toBe(85);
    } finally {
      dir.cleanup();
    }
  });

  it('某框架 source_files 为空数组时不参与加权', () => {
    const dir = createTempDir();
    try {
      const sub1 = createSubReport({
        framework: 'vitest',
        source_files: [], // top-level source_files empty
        coverage: {
          pass: true,
          measured: { lines: 90, branches: 85, functions: 95 },
          thresholds: { lines: 80, branches: 80, functions: 80 },
          by_framework: {
            vitest: {
              measured: { lines: 90, branches: 85, functions: 95 },
              source_files: [],
            },
          },
        },
      });

      const summary = generateSummaryReport(
        [sub1],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      // source_files 为空时该框架不参与加权
      // coverage 仍存在但 measured 各维度为 null（因无有效权重）
      expect(summary.coverage).not.toBeNull();
      expect(summary.coverage!.measured.lines).toBeNull();
    } finally {
      dir.cleanup();
    }
  });
});

// ===========================================================================
// generateSummaryReport -- conclusion edge cases
// ===========================================================================

describe('generateSummaryReport -- conclusion edge cases', () => {
  it('所有子报告测试均失败时 conclusion=fail', () => {
    const dir = createTempDir();
    try {
      const subReport = createSubReport({
        summary: { total: 2, passed: 0, failed: 2, skipped: 0 },
        test_cases: [
          { name: 't1', status: 'failed', errorMessage: 'Error 1' },
          { name: 't2', status: 'failed', errorMessage: 'Error 2' },
        ],
      });

      const summary = generateSummaryReport(
        [subReport],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.conclusion).toBe('fail');
      expect(summary.problems.length).toBeGreaterThan(0);
    } finally {
      dir.cleanup();
    }
  });

  it('没有子报告时 conclusion=pass（未执行不视为失败）', () => {
    const dir = createTempDir();
    try {
      const summary = generateSummaryReport(
        [],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.conclusion).toBe('pass');
      expect(summary.total).toBe(0);
      expect(summary.problems).toEqual([]);
    } finally {
      dir.cleanup();
    }
  });

  it('汇总报告不包含 test_cases 和 test_files 明细字段（AC-7）', () => {
    const dir = createTempDir();
    try {
      const subReport = createSubReport();
      const summary = generateSummaryReport(
        [subReport],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary).not.toHaveProperty('test_cases');
      expect(summary).not.toHaveProperty('test_files');
    } finally {
      dir.cleanup();
    }
  });

  it('子报告 coverage 全为 null 时汇总报告 coverage=null', () => {
    const dir = createTempDir();
    try {
      const subReport = createSubReport({ coverage: null });
      const summary = generateSummaryReport(
        [subReport],
        dir.root,
        path.join(dir.root, 'reports', 'unit-test'),
      );
      expect(summary.coverage).toBeNull();
    } finally {
      dir.cleanup();
    }
  });
});

// ===========================================================================
// Idempotency
// ===========================================================================

describe('generateSummaryReport -- 幂等性', () => {
  it('相同子报告列表重复调用两次应产生相同汇总报告对象（字段值一致）', () => {
    const dir = createTempDir();
    try {
      const subReport = createSubReport();
      const reportsDir = path.join(dir.root, 'reports', 'unit-test');

      const first = generateSummaryReport([subReport], dir.root, reportsDir);
      const second = generateSummaryReport([subReport], dir.root, reportsDir);

      expect(first.total).toBe(second.total);
      expect(first.passed).toBe(second.passed);
      expect(first.failed).toBe(second.failed);
      expect(first.skipped).toBe(second.skipped);
      expect(first.conclusion).toBe(second.conclusion);
    } finally {
      dir.cleanup();
    }
  });
});

describe('generateSubReport -- 幂等性', () => {
  it('相同输入重复调用两次应产生相同子报告对象（字段值一致）', () => {
    const dir = createTempDir();
    try {
      const result = makeExecutionResult({
        testCases: [{ name: 'test1', status: 'passed', durationMs: 100 }],
        sourceFiles: ['src/foo.ts'],
        testFiles: ['src/foo.test.ts'],
      });
      const reportsDir = path.join(dir.root, 'reports', 'unit-test');

      const first = generateSubReport('vitest', result, dir.root, reportsDir);
      const second = generateSubReport('vitest', result, dir.root, reportsDir);

      expect(first.framework).toBe(second.framework);
      expect(first.summary.total).toBe(second.summary.total);
      expect(first.summary.passed).toBe(second.summary.passed);
      expect(first.test_cases).toEqual(second.test_cases);
      expect(first.test_files).toEqual(second.test_files);
      expect(first.source_files).toEqual(second.source_files);
    } finally {
      dir.cleanup();
    }
  });
});
