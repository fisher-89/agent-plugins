import { z } from 'zod/v4';

/**
 * Input schema for `test_detect_frameworks` MCP tool.
 *
 * Accepts an optional `files` array of file paths. When omitted, the tool
 * auto-scans the project for files matching configured glob patterns.
 */
export const testDetectFrameworksInputSchema = {
  files: z
    .array(z.string())
    .optional()
    .describe('File paths to detect framework for (omit for auto-scan)'),
  project_root: z
    .string()
    .optional()
    .nullable()
    .describe('Project root directory (defaults to cwd)'),
};

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
        .string()
        .describe('Framework name (e.g. "vitest", "jest", "rust") or "unknown" if no match'),
    }),
  ),
  frameworks: z.array(z.string()).describe('Unique framework names detected across all files'),
  plan: z
    .array(
      z.object({
        directory: z
          .string()
          .describe('Working directory for command execution (relative to project root)'),
        framework: z.string().describe('Framework name'),
        coverage_cmd: z.string().describe('Coverage command (includes test execution)'),
        coverage_format: z.enum(['istanbul', 'llvm-cov']).describe('Coverage output format'),
        coverage_output: z.string().describe('Coverage output file path (relative to directory)'),
        coverage_artifacts: z
          .array(z.string())
          .optional()
          .describe('Glob patterns for coverage artifacts to move to unified location'),
        coverage_cleanup: z
          .array(z.string())
          .optional()
          .describe('Directory/file names to clean up after successful move'),
      }),
    )
    .describe('Execution plan: one entry per configured framework mapping'),
});
