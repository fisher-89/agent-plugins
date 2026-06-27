/**
 * 测试 testResolvePathsInputSchema / testResolvePathsOutputSchema -- Zod 契约验证。
 *
 * 覆盖范围:
 * - AC-1: modules: [] 空数组通过 schema（原 .min(1) 拒绝 → 现 union 类型允许）
 * - AC-4: modules 非法类型（数字/布尔/对象）被拒绝
 * - AC-11: input/output schema 与 MCP 注册一致
 *
 * @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
 * @see openspec/changes/add-test-path-resolver-api/test-design.md
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
// testResolvePathsInputSchema -- union 类型 (AC-1, AC-3, AC-4)
// @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
// ===========================================================================

describe('testResolvePathsInputSchema -- union 类型', () => {
  it('modules: [] 空数组应通过 schema 验证（原 .min(1) 拒绝 → 现允许）(AC-1)', () => {
    const input = { modules: [] };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('modules: "git-change" 字符串字面量应通过验证 (AC-1)', () => {
    const input = { modules: 'git-change' };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('modules: ["src/a.ts"] 常规数组仍应通过验证（向后兼容）(AC-3)', () => {
    const input = { modules: ['src/a.ts'] };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// testResolvePathsInputSchema -- 边界
// @see openspec/changes/test-resolve-paths-config-dirs/test-design.md
// ===========================================================================

describe('testResolvePathsInputSchema -- 边界', () => {
  it('modules: [""] 空字符串数组应通过 schema（路径校验在 command 层）', () => {
    const input = { modules: [''] };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('modules: "GIT-CHANGE" 大小写不匹配的字面量应被拒绝', () => {
    const input = { modules: 'GIT-CHANGE' };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// testResolvePathsInputSchema -- 异常 (AC-10)
// ===========================================================================

describe('testResolvePathsInputSchema -- 异常', () => {
  it('modules: 123 非法数字类型应被拒绝 (AC-4)', () => {
    const input = { modules: 123 };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('modules: true 布尔类型应被拒绝 (AC-4)', () => {
    const input = { modules: true };
    const result = testResolvePathsInputSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('modules: { key: "val" } 对象类型应被拒绝 (AC-4)', () => {
    const input = { modules: { key: 'val' } };
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
