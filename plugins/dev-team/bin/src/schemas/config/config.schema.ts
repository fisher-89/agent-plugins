import { z } from 'zod/v4';

/** 行覆盖率阈值 */
const TEST_COVERAGE_LINE_DEFAULT = 80;
/** 分支覆盖率阈值 */
const TEST_COVERAGE_BRANCH_DEFAULT = 70;
/** 函数覆盖率阈值 */
const TEST_COVERAGE_FUNCTION_DEFAULT = 75;

export const testFrameworkSchema = z
  .enum(['jest', 'vitest', 'vite-plus', 'bun', 'rust'])
  .describe('测试框架');

const testCoverageSchema = z
  .object({
    lines: z
      .number()
      .optional()
      .prefault(TEST_COVERAGE_LINE_DEFAULT)
      .describe(`行覆盖率阈值（百分比，默认 ${TEST_COVERAGE_LINE_DEFAULT}）`),
    branches: z
      .number()
      .optional()
      .prefault(TEST_COVERAGE_BRANCH_DEFAULT)
      .describe(`分支覆盖率阈值（百分比，默认 ${TEST_COVERAGE_BRANCH_DEFAULT}）`),
    functions: z
      .number()
      .optional()
      .prefault(TEST_COVERAGE_FUNCTION_DEFAULT)
      .describe(`函数覆盖率阈值（百分比，默认 ${TEST_COVERAGE_FUNCTION_DEFAULT}）`),
  })
  .prefault({})
  .describe('测试覆盖率');

/**
 * Zod schema for openspec/config.json.
 *
 * Uses `.passthrough()` to allow extra unknown fields (e.g. tool-specific keys).
 * This ensures backward compatibility with tools that write custom fields.
 *
 * @see design.md (D2 — zod/v4, D3 — .passthrough())
 */
export const configSchema = z.object({
  schema: z
    .literal('spec-driven')
    .optional()
    .prefault('spec-driven')
    .describe('@deprecated 模式标识，固定为 spec-driven'),
  context: z.string().optional().describe('项目上下文描述，用于向 AI 提供业务背景信息'),
  rules: z
    .object({
      proposal: z.array(z.string()).optional().describe('提案阶段的自定义规则列表'),
      tasks: z.array(z.string()).optional().describe('任务阶段的自定义规则列表'),
    })
    .optional()
    .describe('自定义规则配置，按工作流阶段分组'),
  static_analysis: z.string().optional().describe('静态分析工具配置（如 ESLint、Prettier）'),
  test: z
    .object({
      framework: testFrameworkSchema.optional(),
      coverage: testCoverageSchema.optional(),
      overrides: z
        .array(
          z.object({
            file: z.string().nonempty().describe('文件路径，支持glob规则（如 "src/**/*.test.ts"）'),
            framework: testFrameworkSchema.optional(),
            coverage: testCoverageSchema.optional(),
          }),
        )
        .optional(),
    })
    .optional()
    .prefault({})
    .describe('测试相关配置'),
});

/** TypeScript type inferred from configSchema — replaces `Record<string, unknown>`. */
export type OpenSpecConfig = z.output<typeof configSchema>;

export type OpenSpecConfigInput = z.input<typeof configSchema>;

export type TestFrameworks = NonNullable<OpenSpecConfig['test']['framework']>;