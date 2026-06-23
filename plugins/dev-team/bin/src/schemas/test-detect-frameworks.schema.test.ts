/**
 * 测试 testDetectFrameworksOutputSchema -- 输出 schema 中 plan 字段的 Zod 验证。
 *
 * 覆盖范围:
 * - AC-1: plan 数组字段 Zod schema 验证
 * - AC-4: plan 条目新增 coverage_artifacts 和 coverage_cleanup 字段验证
 * - AC-5: 向后兼容 -- 无新字段仍通过验证
 * - 正向: 完整 plan 对象、多条目、空数组、新字段通过验证
 * - 异常: 缺失必填字段、类型错误、非法枚举值、新字段非法值被拒绝
 * - 边界: 大小写敏感、多余字段 passthrough、完整输出对象验证
 *
 * @see openspec/changes/per-directory-test-execution/test-design.md
 * @see openspec/changes/unified-coverage-artifacts/test-design.md
 */

import { describe, it, expect } from 'vite-plus/test';

import { testDetectFrameworksOutputSchema } from './test-detect-frameworks.schema';

// ===========================================================================
// 正向测试: 完整 plan 对象验证
// ===========================================================================

describe('testDetectFrameworksOutputSchema -- plan 正向测试 (AC-1)', () => {
  it('应验证包含完整 plan 数组的输出对象通过 schema 验证', () => {
    const output = {
      detected: [{ file: 'src/test.ts', framework: 'vitest' }],
      frameworks: ['vitest'],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('应验证包含多个 plan 条目的数组通过验证', () => {
    const output = {
      detected: [
        { file: 'plugins/dev-team/bin/src/test.ts', framework: 'vite-plus' },
        { file: 'src/app.test.ts', framework: 'vitest' },
      ],
      frameworks: ['vite-plus', 'vitest'],
      plan: [
        {
          directory: 'plugins/dev-team/bin',
          framework: 'vite-plus',
          coverage_cmd: 'vp test --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('应验证空 plan 数组 [] 通过验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.plan).toEqual([]);
    }
  });

  it('应验证包含多余未知字段的 plan 条目通过 passthrough 验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
          extra_field: 'should be allowed via passthrough',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// 异常测试: plan 字段缺失/类型错误
// ===========================================================================

describe('testDetectFrameworksOutputSchema -- plan 异常测试', () => {
  it('应在 plan 条目缺少 directory 字段时拒绝', () => {
    // `directory` 是必填字符串字段
    const output: Record<string, unknown> = {
      detected: [],
      frameworks: [],
      plan: [
        {
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('应在 plan 条目缺少 framework 字段时拒绝', () => {
    const output: Record<string, unknown> = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('应在 plan 条目缺少 coverage_cmd 字段时拒绝', () => {
    const output: Record<string, unknown> = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('应在 plan 条目的 directory 为 null 时拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: null,
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('应在 coverage_format 为非法枚举值（非 istanbul/llvm-cov）时拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'cobertura',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('应在 plan 为非数组类型（如字符串）时拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: 'not-an-array',
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// 边界测试: 枚举大小写敏感、passthrough、完整输出对象
// ===========================================================================

describe('testDetectFrameworksOutputSchema -- plan 边界测试', () => {
  it('应区分 coverage_format 大小写：istanbul 通过，Istanbul 拒绝', () => {
    const validOutput = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    expect(testDetectFrameworksOutputSchema.safeParse(validOutput).success).toBe(true);

    const invalidOutput = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'Istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    expect(testDetectFrameworksOutputSchema.safeParse(invalidOutput).success).toBe(false);
  });

  it('应验证完整输出对象（detected + frameworks + plan）通过验证', () => {
    const output = {
      detected: [
        { file: '/project/src/a.test.ts', framework: 'vitest' },
        { file: '/project/src/b.test.ts', framework: 'vitest' },
      ],
      frameworks: ['vitest'],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('应在 plan 条目 directory 为空字符串时通过验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    // directory 是 string 类型，空字符串也是合法的 string
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// AC-4: plan 条目 coverage_artifacts 和 coverage_cleanup 正向测试
// ===========================================================================

describe('testDetectFrameworksOutputSchema -- plan 新增字段正向测试 (AC-4)', () => {
  it('包含 coverage_artifacts 和 coverage_cleanup 的 plan 条目通过验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**'],
          coverage_cleanup: ['coverage', '.nyc_output'],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('新字段值为非空字符串数组时通过 schema 验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'rust',
          coverage_cmd: 'cargo llvm-cov --all --coverage',
          coverage_format: 'llvm-cov',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**', 'target/llvm-cov/**'],
          coverage_cleanup: ['coverage', 'target/llvm-cov'],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.plan[0].coverage_artifacts).toHaveLength(2);
    }
  });

  it('coverage_artifacts 和 coverage_cleanup 均为空数组时通过验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: [],
          coverage_cleanup: [],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('多个 plan 条目各自包含新的字段时通过验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**'],
          coverage_cleanup: ['coverage', '.nyc_output'],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
        {
          directory: 'tests',
          framework: 'rust',
          coverage_cmd: 'cargo llvm-cov --all --coverage',
          coverage_format: 'llvm-cov',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**', 'target/llvm-cov/**'],
          coverage_cleanup: ['coverage', 'target/llvm-cov'],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// AC-5: 向后兼容 -- 无新字段仍通过验证
// ===========================================================================

describe('testDetectFrameworksOutputSchema -- 向后兼容 (AC-5)', () => {
  it('plan 条目中无 coverage_artifacts 和 coverage_cleanup 时 schema 验证通过', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('plan 条目包含新旧全部字段通过验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**'],
          coverage_cleanup: ['coverage'],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('完整输出（detected + frameworks + plan 含新字段）通过验证', () => {
    const output = {
      detected: [{ file: '/project/src/a.test.ts', framework: 'vitest' }],
      frameworks: ['vitest'],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**'],
          coverage_cleanup: ['coverage', '.nyc_output'],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('plan 条目含多余未知字段 + 新字段仍通过 passthrough 验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**'],
          coverage_cleanup: ['coverage'],
          script: '#!/bin/bash\nset -e\n\necho test',
          extra_unknown_field: 'should be allowed',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// 新字段异常测试
// ===========================================================================

describe('testDetectFrameworksOutputSchema -- 新字段异常测试', () => {
  it('coverage_artifacts 为 null 时 schema 拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: null,
          coverage_cleanup: ['coverage'],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('coverage_cleanup 为字符串（非数组）时 schema 拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**'],
          coverage_cleanup: 'coverage',
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('coverage_artifacts 含数字元素时 schema 拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**', 123],
          coverage_cleanup: ['coverage'],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('coverage_cleanup 含 null 元素时 schema 拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'src',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          coverage_artifacts: ['coverage/**'],
          coverage_cleanup: ['coverage', null],
          script: '#!/bin/bash\nset -e\n\necho test',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// plan script -- 正向测试 (AC-7)
// ===========================================================================
// TODO: 所有 script schema 测试待 PlanEntry 新增 script 字段后启用

describe('testDetectFrameworksOutputSchema -- plan script 正向测试 (AC-7)', () => {
  it('plan 条目包含 script 字符串字段时通过 schema 验证 (AC-7)', () => {
    const output = {
      detected: [{ file: 'src/test.ts', framework: 'vitest' }],
      frameworks: ['vitest'],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\nnpx vitest run --coverage',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.plan[0].script).toBe('#!/bin/bash\nset -e\n\nnpx vitest run --coverage');
    }
  });

  it('script 为多行 bash 脚本（含 \\n）时通过验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: 'plugins/dev-team/bin',
          framework: 'vite-plus',
          coverage_cmd: 'vp test --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: [
            '#!/bin/bash',
            'set -e',
            '',
            'cd plugins/dev-team/bin',
            'vp test --coverage',
            'rm -rf coverage',
            '',
          ].join('\n'),
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('多个 plan 条目各自包含 script 字段时通过验证', () => {
    const output = {
      detected: [],
      frameworks: ['vitest', 'rust'],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\nnpx vitest run --coverage',
        },
        {
          directory: '.',
          framework: 'rust',
          coverage_cmd: 'cargo llvm-cov --all --coverage',
          coverage_format: 'llvm-cov',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\ncargo llvm-cov --all --coverage',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// plan script -- 异常测试 (AC-8)
// ===========================================================================

describe('testDetectFrameworksOutputSchema -- plan script 异常测试 (AC-8)', () => {
  it('plan 条目缺少 script 字段时 schema 拒绝 (AC-8)', () => {
    const output: Record<string, unknown> = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('plan 条目 script 为 null 时 schema 拒绝 (AC-8)', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: null,
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('plan 条目 script 为数字时 schema 拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: 42,
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });

  it('plan 条目 script 为数组时 schema 拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: ['line1', 'line2'],
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(false);
  });
});

// ===========================================================================
// plan script -- 边界测试
// ===========================================================================

describe('testDetectFrameworksOutputSchema -- plan script 边界测试', () => {
  it('script 为空字符串 "" 时通过 schema 验证（string 类型允许空值）', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('script 为超长字符串（10000 字符）时通过验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\n' + '# ' + 'x'.repeat(9989) + '\necho done',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });

  it('plan 条目含多余未知字段 + script 时通过 passthrough 验证', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [
        {
          directory: '.',
          framework: 'vitest',
          coverage_cmd: 'npx vitest run --coverage',
          coverage_format: 'istanbul',
          coverage_output: 'coverage/coverage-summary.json',
          script: '#!/bin/bash\nset -e\n\necho test',
          extra_field: 'should be allowed',
        },
      ],
    };
    const result = testDetectFrameworksOutputSchema.safeParse(output);
    expect(result.success).toBe(true);
  });
});

// ===========================================================================
// add-node-go-pytest-frameworks: plan coverage_format 新枚举值 (AC-5)
// @see openspec/changes/add-node-go-pytest-frameworks/test-design.md
// ===========================================================================

describe('testDetectFrameworksOutputSchema — plan coverage_format (AC-5)', () => {
  const planBase = {
    directory: '.',
    framework: 'vitest',
    coverage_cmd: 'npx vitest run --coverage',
    coverage_output: 'coverage/coverage-summary.json',
    script: '#!/bin/bash\nset -e\n\necho test',
  };

  it('plan 条目 coverage_format 为 node-test / go-cover / coverage-py 时通过验证', () => {
    for (const fmt of ['node-test', 'go-cover', 'coverage-py']) {
      const output = {
        detected: [],
        frameworks: [],
        plan: [{ ...planBase, coverage_format: fmt }],
      };
      expect(testDetectFrameworksOutputSchema.safeParse(output).success).toBe(true);
    }
  });

  it('空字符串 coverage_format 被拒绝', () => {
    const output = {
      detected: [],
      frameworks: [],
      plan: [{ ...planBase, coverage_format: '' }],
    };
    expect(testDetectFrameworksOutputSchema.safeParse(output).success).toBe(false);
  });
});
