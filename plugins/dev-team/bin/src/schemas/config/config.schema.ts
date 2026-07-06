import { z } from 'zod/v4';

/** 行覆盖率阈值 */
const TEST_COVERAGE_LINE_DEFAULT = 80;
/** 分支覆盖率阈值 */
const TEST_COVERAGE_BRANCH_DEFAULT = 70;
/** 函数覆盖率阈值 */
const TEST_COVERAGE_FUNCTION_DEFAULT = 75;

/** 变异测试得分阈值 */
const TEST_MUTATION_SCORE_DEFAULT = 80;

const testFrameworkSchema = z
  .enum(['jest', 'vitest', 'vite-plus', 'bun', 'rust', 'node-test', 'go', 'pytest'])
  .describe('测试框架');

const testCoverageSchema = z
  .object({
    lines: z
      .number()
      .min(0)
      .max(100)
      .optional()
      .prefault(TEST_COVERAGE_LINE_DEFAULT)
      .describe(`行覆盖率阈值（百分比，默认 ${TEST_COVERAGE_LINE_DEFAULT}）`),
    branches: z
      .number()
      .min(0)
      .max(100)
      .optional()
      .prefault(TEST_COVERAGE_BRANCH_DEFAULT)
      .describe(`分支覆盖率阈值（百分比，默认 ${TEST_COVERAGE_BRANCH_DEFAULT}）`),
    functions: z
      .number()
      .min(0)
      .max(100)
      .optional()
      .prefault(TEST_COVERAGE_FUNCTION_DEFAULT)
      .describe(`函数覆盖率阈值（百分比，默认 ${TEST_COVERAGE_FUNCTION_DEFAULT}）`),
  })
  .prefault({})
  .describe('测试覆盖率');

const mutationScoreSchema = z
  .number()
  .min(0)
  .max(100)
  .optional()
  .describe('变异测试得分阈值（百分比）');

const mutationConfigSchema = z
  .object({
    score: mutationScoreSchema
      .prefault(TEST_MUTATION_SCORE_DEFAULT)
      .describe(`变异测试得分阈值（百分比，默认 ${TEST_MUTATION_SCORE_DEFAULT}）`),
  })
  .prefault({})
  .describe('变异测试配置');

const mutationOverrideSchema = z
  .object({ score: mutationScoreSchema })
  .describe('变异测试覆盖配置');

/**
 * Zod schema for openspec/config.json.
 *
 * Uses `.passthrough()` to allow extra unknown fields (e.g. tool-specific keys).
 * This ensures backward compatibility with tools that write custom fields.
 *
 * @see design.md (D2 — zod/v4, D3 — .passthrough())
 */
const writeProtectionFileSchema = z
  .object({
    glob: z
      .string()
      .nonempty()
      .describe('文件路径 glob 模式（如 "openspec/changes/**/eval.json"）'),
    reason: z
      .string()
      .optional()
      .describe('自定义拒绝原因，支持 %s（文件路径）和 %t（工具名称）占位符'),
  })
  .describe('单条写入保护规则');

const writeProtectionSchema = z
  .object({
    files: z.array(writeProtectionFileSchema).optional().describe('需要保护的文件 glob 模式列表'),
  })
  .describe('写入保护配置');

export const configSchema = z.object({
  $schema: z.string().optional().describe('schema规则文件'),
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
      mutation: mutationConfigSchema.optional(),
      overrides: z
        .array(
          z.object({
            file: z.string().nonempty().describe('文件路径，支持glob规则（如 "src/**/*.test.ts"）'),
            framework: testFrameworkSchema.optional(),
            coverage: testCoverageSchema.optional(),
            mutation: mutationOverrideSchema.optional(),
          }),
        )
        .optional(),
    })
    .optional()
    .prefault({})
    .describe('测试相关配置'),
  write_protection: writeProtectionSchema
    .optional()
    .describe('写入保护配置，定义受保护的文件路径模式'),
});

/** TypeScript type inferred from configSchema — replaces `Record<string, unknown>`. */
export type OpenSpecConfig = z.output<typeof configSchema>;

export type OpenSpecConfigInput = z.input<typeof configSchema>;

export type TestFrameworks = NonNullable<OpenSpecConfig['test']['framework']>;
