import { z } from 'zod/v4';

/** 测试框架 */
const TEST_FRAMEWORKS = ['jest', 'vitest', 'vite-plus', 'bun', 'rust'];
/** 行覆盖率阈值 */
const TEST_COVERAGE_LINE_DEFAULT = 80;
/** 分支覆盖率阈值 */
const TEST_COVERAGE_BRANCH_DEFAULT = 70;
/** 函数覆盖率阈值 */
const TEST_COVERAGE_FUNCTION_DEFAULT = 75;

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
    .describe('@deperacated 模式标识，固定为 spec-driven'),
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
      frameworks: z
        .union([
          z.enum(TEST_FRAMEWORKS).describe('测试框架'),
          z
            .array(
              z.object({
                glob: z.string().nonempty().describe('文件匹配模式，如 "src/**/*.test.ts"'),
                framework: z.enum(TEST_FRAMEWORKS).describe('对应的测试框架名称'),
              }),
            )
            .describe('按文件模式指定不同测试框架的映射列表'),
        ])
        .optional()
        .describe('测试框架配置，支持单一框架或按文件模式映射多个框架'),
      coverage: z
        .object({
          thresholds: z
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
            .optional()
            .prefault({})
            .describe('全局覆盖率阈值配置'),
          overrides: z
            .array(
              z.object({
                glob: z.string().describe('文件匹配模式，用于指定覆盖范围'),
                thresholds: z.object({
                  lines: z.number().optional().describe('行覆盖率阈值（百分比）'),
                  branches: z.number().optional().describe('分支覆盖率阈值（百分比）'),
                  functions: z.number().optional().describe('函数覆盖率阈值（百分比）'),
                }),
              }),
            )
            .optional()
            .describe('按文件模式覆盖默认阈值的规则列表'),
        })
        .optional()
        .prefault({})
        .describe('测试覆盖率配置，包含全局阈值和文件级覆盖规则'),
    })
    .optional()
    .prefault({})
    .describe('测试相关配置'),
});

/** TypeScript type inferred from configSchema — replaces `Record<string, unknown>`. */
export type OpenSpecConfig = z.infer<typeof configSchema>;
