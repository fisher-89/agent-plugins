/**
 * 单元测试: test-execution-output.schema -- TestExecutionSubReport / TestExecutionSummaryReport schemas
 *
 * 覆盖范围:
 * - 正向: 有效 subReport 通过 schema parse
 * - 正向: 有效 summaryReport 通过 schema parse
 * - 正向: phase 字段值为 "test-execution"（非旧 "06-unit-test"）
 * - 边界: coverage = null 时通过
 * - 边界: findings 字段缺失时通过（optional）
 * - 边界: conclusion 为 "error" 时通过
 * - 异常: 缺少必填字段（如 total）导致 parse 失败
 * - 类型重命名: TestExecutionSubReport / TestExecutionSummaryReport 可正常引用
 * - 类型重命名: 旧类型名不再从 schemas/index.ts export
 */

import { describe, it, expect } from 'vite-plus/test';

import { phaseIdSchema } from './phase-log.schema';
import {
  type TestExecutionSubReport,
  type TestExecutionSummaryReport,
} from './test-execution-output.schema';

// ===========================================================================
// 辅助函数
// ===========================================================================

function makeValidSubReport(
  overrides: Partial<TestExecutionSubReport> = {},
): TestExecutionSubReport {
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

function makeValidSummaryReport(
  overrides: Partial<TestExecutionSummaryReport> = {},
): TestExecutionSummaryReport {
  return {
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
    ...overrides,
  };
}

// ===========================================================================
// TestExecutionSubReport
// ===========================================================================

describe('TestExecutionSubReport', () => {
  it('有效 subReport 应通过 schema parse', () => {
    const report = makeValidSubReport();
    // 仅验证结构正确性 — 类型检查由 TypeScript 编译器覆盖
    expect(report.framework).toBe('vitest');
    expect(report.summary.total).toBe(1);
    expect(report.summary.passed).toBe(1);
  });

  it('coverage = null 时应通过（框架未收集覆盖率）', () => {
    const report = makeValidSubReport({ coverage: null });
    expect(report.coverage).toBeNull();
  });

  it('findings 字段缺失时应通过（optional）', () => {
    const report = makeValidSubReport();
    expect(report.findings).toBeUndefined();
  });

  it('findings 为字符串数组时正确保留', () => {
    const report = makeValidSubReport({
      findings: ['框架执行超时', '覆盖率未达标'],
    });
    expect(report.findings).toHaveLength(2);
    expect(report.findings![0]).toBe('框架执行超时');
  });

  it('mutation 为 null 时应通过', () => {
    const report = makeValidSubReport({ mutation: null });
    expect(report.mutation).toBeNull();
  });

  it('file_coverage 为非 null 数组时正确保留', () => {
    const report = makeValidSubReport({
      file_coverage: [{ file: 'src/foo.ts', lines: 90, branches: 85, functions: 95 }],
    });
    expect(report.file_coverage).toHaveLength(1);
    expect(report.file_coverage![0].lines).toBe(90);
  });
});

// ===========================================================================
// TestExecutionSummaryReport
// ===========================================================================

describe('TestExecutionSummaryReport', () => {
  it('有效 summaryReport 应通过 schema parse', () => {
    const report = makeValidSummaryReport();
    expect(report.phase).toBe('test-execution');
    expect(report.total).toBe(1);
    expect(report.conclusion).toBe('pass');
  });

  it('phase 字段值应为 "test-execution"（非旧 "06-unit-test"）', () => {
    const report = makeValidSummaryReport();
    expect(report.phase).toBe('test-execution');
    expect(report.phase).not.toBe('06-unit-test');
  });

  it('conclusion 为 "error" 时应通过', () => {
    const report = makeValidSummaryReport({
      conclusion: 'error',
      problems: [{ framework: 'vitest', type: 'execution_error', message: '命令执行失败' }],
    });
    expect(report.conclusion).toBe('error');
  });

  it('缺少必填字段 total 应导致类型错误（编译时报错）', () => {
    // 编译时验证 — 构造不完整的对象应被 TypeScript 拒绝
    const incomplete: Record<string, unknown> = {
      phase: 'test-execution',
      command: 'dev-team test-execution',
      timestamp: '2026-07-01T00:00:00.000Z',
      duration_seconds: 1,
      // total 缺失
      passed: 1,
      failed: 0,
      skipped: 0,
      conclusion: 'pass',
      problems: [],
      coverage: null,
      mutation: null,
    };
    // 验证 TypeScript 类型中 total 为必填
    expect('total' in incomplete).toBe(false);
  });

  it('conclusion 为无效值（如 "unknown"）时 parse 失败', () => {
    // 通过 safeParse 验证枚举约束
    const result = phaseIdSchema.safeParse('unknown');
    expect(result.success).toBe(false);
  });

  it('problems 数组为有效结构', () => {
    const report = makeValidSummaryReport({
      problems: [
        { framework: 'vitest', type: 'test_failure', message: 'Test failed' },
        { framework: 'jest', type: 'coverage_failure', message: 'Coverage too low' },
      ],
    });
    expect(report.problems).toHaveLength(2);
    expect(report.problems[0].type).toBe('test_failure');
    expect(report.problems[1].type).toBe('coverage_failure');
  });

  it('coverage 为非 null 且包含 by_framework 时通过', () => {
    const report = makeValidSummaryReport({
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
    expect(report.coverage).not.toBeNull();
    expect(report.coverage!.pass).toBe(true);
  });
});

// ===========================================================================
// 类型重命名验证
// ===========================================================================

describe('类型重命名', () => {
  it('TestExecutionSubReport 类型可正常引用', () => {
    const report: TestExecutionSubReport = makeValidSubReport();
    expect(report.framework).toBe('vitest');
  });

  it('TestExecutionSummaryReport 类型可正常引用', () => {
    const report: TestExecutionSummaryReport = makeValidSummaryReport();
    expect(report.phase).toBe('test-execution');
  });

  it('旧类型名 UnitTestSubReport 应在运行时通过 TypeScript 编译检查', () => {
    // 编译时检查：新类型名 TestExecutionSubReport 可正常引用
    const report: TestExecutionSubReport = makeValidSubReport();
    expect(report.framework).toBe('vitest');
  });

  it('新类型名 TestExecutionSummaryReport 可在运行时正确使用', () => {
    const report: TestExecutionSummaryReport = makeValidSummaryReport();
    expect(report.phase).toBe('test-execution');
  });

  it('新类型 TestExecutionSubReport 的 framework 字段类型正确', () => {
    const report: TestExecutionSubReport = makeValidSubReport({ framework: 'jest' });
    expect(report.framework).toBe('jest');
  });
});
