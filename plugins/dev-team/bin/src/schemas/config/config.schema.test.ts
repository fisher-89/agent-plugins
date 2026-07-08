/**
 * 单元测试: config.schema.ts — write_protection 配置项的 Zod schema 验证
 *
 * @see openspec/changes/write-protection-config/test-design.md
 *
 * 补充说明: 存量字段 (test.framework, test.coverage, test.mutation) 的边界测试
 * 用于提升 config.schema.ts 的 branches/function 覆盖率。
 */

import { describe, it, expect } from 'vite-plus/test';

import { configSchema } from '../config/config.schema';

// ---------------------------------------------------------------------------
// writeProtectionSchema — 有效配置验证 (AC-1)
// ---------------------------------------------------------------------------

describe('writeProtectionSchema — 有效配置验证 (AC-1)', () => {
  it('包含完整 write_protection.files 配置时验证通过', () => {
    const input = {
      write_protection: {
        files: [{ glob: 'openspec/data/*.json', reason: '受保护 %s' }],
      },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      const protection = result.data.write_protection;
      expect(protection?.files).toHaveLength(1);
      expect(protection?.files?.[0].glob).toBe('openspec/data/*.json');
      expect(protection?.files?.[0].reason).toBe('受保护 %s');
    }
  });

  it('write_protection 完全省略时验证通过 (.passthrough())', () => {
    const input = { schema: 'spec-driven' };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('write_protection 为 {} 空对象时验证通过（files 可选）', () => {
    const input = { write_protection: {} };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('files 为空数组 [] 时验证通过', () => {
    const input = { write_protection: { files: [] } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('reason 省略时验证通过', () => {
    const input = {
      write_protection: { files: [{ glob: 'openspec/data/*.json' }] },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('多个 files 条目均有效时验证通过', () => {
    const input = {
      write_protection: {
        files: [
          { glob: 'openspec/data/*.json', reason: '保护 1' },
          { glob: 'openspec/config.json', reason: '保护 2' },
          { glob: '**/secrets.env' },
        ],
      },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.write_protection?.files).toHaveLength(3);
    }
  });

  it('额外未知字段不导致验证失败 (.passthrough() 兼容)', () => {
    const input = {
      write_protection: {
        files: [{ glob: '*.json', reason: 'test' }],
        unknown_field: 'should be allowed',
      },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('顶层额外未知字段不导致验证失败', () => {
    const input = {
      schema: 'spec-driven',
      custom_tool_config: { some: 'value' },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// writeProtectionSchema — 无效配置拒绝 (AC-1)
// ---------------------------------------------------------------------------

describe('writeProtectionSchema — 无效配置拒绝 (AC-1)', () => {
  it('files 条目缺少必填字段 glob 时验证失败', () => {
    const input = {
      write_protection: { files: [{ reason: '缺少 glob' }] },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('glob 为空字符串时验证失败', () => {
    const input = {
      write_protection: { files: [{ glob: '' }] },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('files 为字符串而非数组时验证失败', () => {
    const input = {
      write_protection: { files: 'not-an-array' },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('write_protection 为字符串而非对象时验证失败', () => {
    const input = { write_protection: 'not-an-object' };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('reason 为数字而非字符串时验证失败', () => {
    const input = {
      write_protection: { files: [{ glob: '*.json', reason: 42 }] },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('files 中的无效条目不应被静默忽略', () => {
    const input = {
      write_protection: { files: [null] },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('files 条目中的额外字段被静默剥离（not strict）', () => {
    const input = {
      write_protection: { files: [{ glob: '*.json', reason: 'ok', extra_field: 'nope' }] },
    };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      // 验证 extra_field 被剥离
      const fileEntry = result.data.write_protection!.files![0];
      expect(fileEntry).not.toHaveProperty('extra_field');
    }
  });

  it('嵌套层级过深的对象不应导致异常', () => {
    // 使用递归构造深度嵌套对象，避免类型断言
    function deepObject(depth: number): Record<string, unknown> {
      if (depth <= 0) return {};
      return { next: deepObject(depth - 1) };
    }
    const input: Record<string, unknown> = {
      write_protection: {
        files: [{ glob: '*.json', reason: 'test' }],
      },
      deep: deepObject(100),
    };
    // 超深嵌套字段不应导致 stack overflow 或 crash
    expect(() => configSchema.safeParse(input)).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// 存量 schema — test.framework 枚举值验证（补充存量覆盖）
// ---------------------------------------------------------------------------

describe('存量 schema — test.framework 验证', () => {
  const validFrameworks = [
    'jest',
    'vitest',
    'vite-plus',
    'bun',
    'rust',
    'node-test',
    'go',
    'pytest',
  ] as const;

  for (const framework of validFrameworks) {
    it(`test.framework 为 "${framework}" 时验证通过`, () => {
      const input = { test: { framework } };
      const result = configSchema.safeParse(input);
      expect(result.success).toBe(true);
    });
  }

  it('test.framework 为无效枚举值时验证失败', () => {
    const input = { test: { framework: 'unknown-framework' } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('test 对象完全省略时验证通过（prefault 为空对象）', () => {
    const input = {};
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 存量 schema — test.coverage 边界（补充存量覆盖）
// ---------------------------------------------------------------------------

describe('存量 schema — test.coverage 边界', () => {
  it('lines/branches/functions 均为 0 时验证通过（下限边界）', () => {
    const input = { test: { coverage: { lines: 0, branches: 0, functions: 0 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('lines/branches/functions 均为 100 时验证通过（上限边界）', () => {
    const input = { test: { coverage: { lines: 100, branches: 100, functions: 100 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('lines 为负数时验证失败', () => {
    const input = { test: { coverage: { lines: -1 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('lines 超过 100 时验证失败', () => {
    const input = { test: { coverage: { lines: 101 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('branches 为负数时验证失败', () => {
    const input = { test: { coverage: { branches: -5 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('branches 超过 100 时验证失败', () => {
    const input = { test: { coverage: { branches: 200 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('functions 超过 100 时验证失败', () => {
    const input = { test: { coverage: { functions: 150 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('coverage 未配置时使用 prefault 默认值', () => {
    const input = { test: {} };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.test?.coverage?.lines).toBe(80);
      expect(result.data.test?.coverage?.branches).toBe(70);
      expect(result.data.test?.coverage?.functions).toBe(75);
    }
  });
});

// ---------------------------------------------------------------------------
// 存量 schema — test.mutation 边界（补充存量覆盖）
// ---------------------------------------------------------------------------

describe('存量 schema — test.mutation 边界', () => {
  it('score 为 0 时验证通过（下限边界）', () => {
    const input = { test: { mutation: { score: 0 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('score 为 100 时验证通过（上限边界）', () => {
    const input = { test: { mutation: { score: 100 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('score 为负数时验证失败', () => {
    const input = { test: { mutation: { score: -10 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('score 超过 100 时验证失败', () => {
    const input = { test: { mutation: { score: 200 } } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it('mutation 未配置时使用 prefault 默认值', () => {
    const input = { test: {} };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.test?.mutation?.score).toBe(70);
    }
  });
});

// ---------------------------------------------------------------------------
// 存量 schema — 其他字段基础验证（补充存量覆盖）
// ---------------------------------------------------------------------------

describe('存量 schema — 其他字段基础验证', () => {
  it('context 为字符串时验证通过', () => {
    const input = { context: '这是一个项目上下文描述' };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.context).toBe('这是一个项目上下文描述');
    }
  });

  it('static_analysis 为字符串时验证通过', () => {
    const input = { static_analysis: 'pnpm run check' };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.static_analysis).toBe('pnpm run check');
    }
  });

  it('rules 包含 proposal 和 tasks 规则列表时验证通过', () => {
    const input = { rules: { proposal: ['rule1'], tasks: ['rule2'] } };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.rules?.proposal).toEqual(['rule1']);
      expect(result.data.rules?.tasks).toEqual(['rule2']);
    }
  });

  it('schema 为 "spec-driven" 时验证通过', () => {
    const input = { schema: 'spec-driven' };
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it('schema 省略时 prefault 为 "spec-driven"', () => {
    const input = {};
    const result = configSchema.safeParse(input);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.schema).toBe('spec-driven');
    }
  });
});
