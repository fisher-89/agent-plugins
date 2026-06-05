import { z } from 'zod/v4';

/**
 * Input schema for `test_get_framework_config` MCP tool.
 *
 * Accepts a framework name to look up in the hardcoded command registry.
 */
export const testGetFrameworkConfigInputSchema = {
  framework: z
    .string()
    .min(1)
    .describe('Framework name to look up (e.g. "vitest", "jest", "rust")'),
  project_root: z
    .string()
    .optional()
    .nullable()
    .describe('Project root directory (defaults to cwd)'),
};

/**
 * Output schema for `test_get_framework_config` MCP tool.
 *
 * Returns the test and coverage command configuration for a known framework.
 */
export const testGetFrameworkConfigOutputSchema = z.object({
  framework: z.string().describe('Framework name'),
  test_cmd: z.string().describe('Test command to execute'),
  coverage_cmd: z.string().describe('Coverage command to execute'),
  coverage_format: z.enum(['istanbul', 'llvm-cov']).describe('Coverage output format identifier'),
  coverage_output: z.string().describe('Coverage output file path (relative to project root)'),
});
