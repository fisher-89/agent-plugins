/**
 * 测试 testGetFrameworkConfigOutputSchema -- 输出 schema 的向后兼容性和新字段验证。
 *
 * 覆盖范围:
 * - AC-5: coverage_artifacts 和 coverage_cleanup 为可选字段（向后兼容）
 * - AC-5: 新字段存在时通过验证
 * - 边界: 空数组、undefined、超长字符串元素
 * - 异常: null、非法类型、含非法元素
 *
 * @see openspec/changes/unified-coverage-artifacts/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import { testGetFrameworkConfigOutputSchema } from './test-get-framework-config.schema';

// ===========================================================================
// AC-5: 向后兼容 -- 无新字段仍通过验证
// ===========================================================================

describe('testGetFrameworkConfigOutputSchema -- 向后兼容 (AC-5)', () => {
  it('输出对象无 coverage_artifacts 和 coverage_cleanup 时 schema 验证通过', () => {
    const output = {
      framework: 'vitest',
      test_cmd: 'npx vitest run --reporter=verbose',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('输出对象包含 coverage_artifacts 和 coverage_cleanup 时通过验证', () => {
    const output = {
      framework: 'vitest',
      test_cmd: 'npx vitest run --reporter=verbose',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/**'],
      coverage_cleanup: ['coverage', '.nyc_output'],
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.coverage_artifacts).toEqual(['coverage/**']);
      expect(result.data.coverage_cleanup).toEqual(['coverage', '.nyc_output']);
    }
  });

  it('新字段显式设置为 undefined 时 schema 验证通过', () => {
    const output = {
      framework: 'rust',
      test_cmd: 'cargo test',
      coverage_cmd: 'cargo llvm-cov --all --coverage',
      coverage_format: 'llvm-cov',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: undefined,
      coverage_cleanup: undefined,
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('新字段仅为可选，不应破坏原有必填字段的校验', () => {
    const output = {
      // 缺少必填字段 framework
      test_cmd: 'npx vitest run',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// 边界测试: 新字段的边界情况
// ===========================================================================

describe('testGetFrameworkConfigOutputSchema -- 新字段边界测试', () => {
  it('coverage_artifacts 为空数组时通过验证', () => {
    const output = {
      framework: 'vitest',
      test_cmd: 'npx vitest run --reporter=verbose',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: [],
      coverage_cleanup: ['coverage'],
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('coverage_cleanup 为单元素数组时通过验证', () => {
    const output = {
      framework: 'bun',
      test_cmd: 'bun test',
      coverage_cmd: 'bun test --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/**'],
      coverage_cleanup: ['coverage'],
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('coverage_artifacts 含超长字符串元素（2000 字符）时通过验证', () => {
    const output = {
      framework: 'vitest',
      test_cmd: 'npx vitest run --reporter=verbose',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['x'.repeat(2000)],
      coverage_cleanup: ['coverage'],
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// 异常测试: 新字段的非法类型和值
// ===========================================================================

describe('testGetFrameworkConfigOutputSchema -- 新字段异常测试', () => {
  it('coverage_artifacts 为 null 时 schema 拒绝', () => {
    const output = {
      framework: 'vitest',
      test_cmd: 'npx vitest run --reporter=verbose',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: null,
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('coverage_artifacts 为字符串（非数组）时 schema 拒绝', () => {
    const output = {
      framework: 'vitest',
      test_cmd: 'npx vitest run --reporter=verbose',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: 'coverage/**',
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('coverage_cleanup 含 null 元素时 schema 拒绝', () => {
    const output = {
      framework: 'vitest',
      test_cmd: 'npx vitest run --reporter=verbose',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/**'],
      coverage_cleanup: ['coverage', null],
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('coverage_artifacts 含数字元素时 schema 拒绝', () => {
    const output = {
      framework: 'vitest',
      test_cmd: 'npx vitest run --reporter=verbose',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/**', 123],
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('coverage_cleanup 为对象时 schema 拒绝', () => {
    const output = {
      framework: 'vitest',
      test_cmd: 'npx vitest run --reporter=verbose',
      coverage_cmd: 'npx vitest run --coverage',
      coverage_format: 'istanbul',
      coverage_output: 'coverage/coverage-summary.json',
      coverage_artifacts: ['coverage/**'],
      coverage_cleanup: {},
    };
    const result = testGetFrameworkConfigOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });
});
