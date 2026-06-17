/**
 * 测试 testResolvePathsInputSchema / testResolvePathsOutputSchema -- Zod 契约验证。
 *
 * 覆盖范围:
 * - AC-10: modules 空数组被 input schema 拒绝
 * - AC-11: input/output schema 与 MCP 注册一致
 *
 * @see openspec/changes/add-test-path-resolver-api/test-design.md
 * @see openspec/changes/add-test-path-resolver-api/specs/test-path-resolver/spec.md
 */

import { describe, it, expect } from 'vite-plus/test';

import {
  testResolvePathsInputSchema,
  testResolvePathsOutputSchema,
} from './test-resolve-paths.schema';

// ===========================================================================
// testResolvePathsInputSchema -- 正向
// ===========================================================================

describe('testResolvePathsInputSchema -- 正向', () => {
  it('完整有效输入（含 modules、integration_scenarios、extension、project_root）应通过验证', () => {
    const input = {
      modules: ['src/config.ts'],
      integration_scenarios: ['api-flow'],
      extension: 'ts',
      project_root: '/abs/project',
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('仅 modules 最小必填输入应通过验证', () => {
    const input = {
      modules: ['src/a.ts'],
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('extension 带前导 "." 的输入应通过 schema（规范化在 command 层）', () => {
    const input = {
      modules: ['src/a.ts'],
      extension: '.ts',
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('project_root 为 null 时应通过验证（nullable）', () => {
    const input = {
      modules: ['src/a.ts'],
      project_root: null,
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// testResolvePathsInputSchema -- 异常 (AC-10)
// ===========================================================================

describe('testResolvePathsInputSchema -- 异常', () => {
  it('modules: [] 应被 input schema 拒绝 (AC-10)', () => {
    const input = {
      modules: [],
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('缺少 modules 字段时应拒绝', () => {
    const input = {
      integration_scenarios: ['api-flow'],
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('modules 为非数组类型时应拒绝', () => {
    const input = {
      modules: 'src/a.ts',
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// testResolvePathsOutputSchema -- 正向 (AC-11)
// ===========================================================================

describe('testResolvePathsOutputSchema -- 正向', () => {
  it('完整输出对象（含 unit_tests、integration_tests、errors）应通过验证 (AC-11)', () => {
    const output = {
      unit_tests: [{ source: 'src/config.ts', test_file: 'src/config.test.ts' }],
      integration_tests: [
        { scenario: 'api-flow', test_file: '__tests__/api-flow/api-flow.test.ts' },
      ],
      errors: [{ path: 'README.md', message: 'Not a testable source file' }],
    };
    const result = testResolvePathsOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('三个数组均为空 [] 时应通过验证', () => {
    const output = {
      unit_tests: [],
      integration_tests: [],
      errors: [],
    };
    const result = testResolvePathsOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// testResolvePathsOutputSchema -- 异常
// ===========================================================================

describe('testResolvePathsOutputSchema -- 异常', () => {
  it('缺少 unit_tests 字段时应拒绝', () => {
    const output = {
      integration_tests: [],
      errors: [],
    };
    const result = testResolvePathsOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('缺少 integration_tests 字段时应拒绝', () => {
    const output = {
      unit_tests: [],
      errors: [],
    };
    const result = testResolvePathsOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('缺少 errors 字段时应拒绝', () => {
    const output = {
      unit_tests: [],
      integration_tests: [],
    };
    const result = testResolvePathsOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('unit_tests 条目缺少 source 时应拒绝', () => {
    const output = {
      unit_tests: [{ test_file: 'src/config.test.ts' }],
      integration_tests: [],
      errors: [],
    };
    const result = testResolvePathsOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('unit_tests 条目缺少 test_file 时应拒绝', () => {
    const output = {
      unit_tests: [{ source: 'src/config.ts' }],
      integration_tests: [],
      errors: [],
    };
    const result = testResolvePathsOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('errors 条目 message 为 null 时应拒绝', () => {
    const output = {
      unit_tests: [],
      integration_tests: [],
      errors: [{ path: 'README.md', message: null }],
    };
    const result = testResolvePathsOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// testResolvePathsOutputSchema -- 边界
// ===========================================================================

describe('testResolvePathsOutputSchema -- 边界', () => {
  it('integration_tests 含多条 scenario 条目时应通过验证', () => {
    const output = {
      unit_tests: [],
      integration_tests: [
        { scenario: 'alpha', test_file: '__tests__/alpha/alpha.test.ts' },
        { scenario: 'beta', test_file: '__tests__/beta/beta.test.ts' },
        { scenario: 'gamma', test_file: '__tests__/gamma/gamma.test.ts' },
      ],
      errors: [],
    };
    const result = testResolvePathsOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.integration_tests).toHaveLength(3);
    }
  });
});

// ===========================================================================
// add-integration-root-param — testResolvePathsInputSchema.integration_root
// @see openspec/changes/add-integration-root-param/test-design.md
// ===========================================================================

describe('testResolvePathsInputSchema -- integration_root 可选字段', () => {
  it('含 integration_root: "plugins/dev-team/bin" 的完整有效输入应通过验证 (AC-7)', () => {
    const input = {
      modules: ['src/config.ts'],
      integration_scenarios: ['api-flow'],
      extension: 'ts',
      integration_root: 'plugins/dev-team/bin',
      project_root: '/abs/project',
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data).toHaveProperty('integration_root', 'plugins/dev-team/bin');
    }
  });

  it('不含 integration_root 的最小必填输入仍应通过（向后兼容）', () => {
    const input = {
      modules: ['src/a.ts'],
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('integration_root: "" 空字符串应通过 schema（规范化在 command 层）', () => {
    const input = {
      modules: ['src/a.ts'],
      integration_root: '',
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('integration_root: "." 应通过 schema', () => {
    const input = {
      modules: ['src/a.ts'],
      integration_root: '.',
    };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });
});
