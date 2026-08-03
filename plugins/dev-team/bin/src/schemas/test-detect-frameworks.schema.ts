import { z } from 'zod/v4';

import { testFrameworkSchema } from './config/config.schema';
import { projectRootSchema } from './public/project-root.schema';

const testPlanSchema = z.object({
  cwd: z.string().describe('Working directory for command execution (relative to project root)'),
  root: z
    .string()
    .describe(
      'Suite test-tree root (relative to project root). File membership boundary; CLI path filter when {files} is empty is derived as root relative to cwd',
    ),
  framework: testFrameworkSchema.describe('Framework name'),
  coverage_format: z
    .enum(['istanbul', 'llvm-cov', 'node-test', 'go-cover', 'coverage-py', 'lcov'])
    .describe('Coverage output format'),
  coverage_output: z
    .string()
    .describe('Coverage output file name relative to the plan report directory (reportDir)'),
  mutation_config: z
    .object({
      score: z.number().describe('Mutation score threshold for this plan entry'),
    })
    .nullable()
    .optional()
    .describe('Mutation override configuration, null when no override applies'),
  mutation_score: z
    .number()
    .nullable()
    .optional()
    .describe('Global mutation score threshold from config'),
  mutation_script: z
    .object({
      shell: z
        .string()
        .describe('POSIX mutation command template (placeholders like {config} unresolved)'),
      cmd: z
        .string()
        .describe('Windows cmd.exe mutation command template (placeholders unresolved)'),
    })
    .nullable()
    .describe(
      'Platform-specific mutation scripts from registry; null when the framework has no mutation_execution',
    ),
  script: z
    .object({
      shell: z
        .string()
        .describe('POSIX shell (bash) execution script for Unix/macOS and Windows Git Bash'),
      cmd: z.string().describe('Windows cmd.exe execution script for Windows without Git Bash'),
    })
    .describe('Platform-specific execution scripts: shell for POSIX, cmd for Windows cmd.exe'),
});

/**
 * Input schema for `test_detect_frameworks` MCP tool.
 *
 * Accepts an optional `files` array of file paths. When omitted, the tool
 * auto-scans the project for files matching configured glob patterns.
 */
export const testDetectFrameworksInputSchema = z.object({
  project_root: projectRootSchema,
  files: z
    .array(z.string())
    .optional()
    .describe('File paths to detect framework for (omit for auto-scan)'),
});

/**
 * Output schema for `test_detect_frameworks` MCP tool.
 *
 * Returns per-file framework detection results and a deduplicated list of
 * unique framework names found.
 */
export const testDetectFrameworksOutputSchema = z.object({
  detected: z.array(
    z.object({
      file: z.string().describe('File path'),
      framework: z
        .union([testFrameworkSchema, z.literal('unknown')])
        .describe('Framework name (e.g. "vitest", "jest", "rust") or "unknown" if no match'),
    }),
  ),
  plan: z
    .array(testPlanSchema)
    .describe('Execution plan: one entry per configured framework mapping'),
});

export type TestDetectFrameworksResult = z.infer<typeof testDetectFrameworksOutputSchema>;
export type TestPlan = z.infer<typeof testPlanSchema>;
