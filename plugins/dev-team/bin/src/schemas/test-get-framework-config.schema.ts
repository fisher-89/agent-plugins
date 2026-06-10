import { z } from 'zod/v4';

import { testFrameworkSchema } from './config/config.schema';

/**
 * Input schema for `test_get_framework_config` MCP tool.
 *
 * Accepts a framework name to look up in the hardcoded command registry.
 */
export const testGetFrameworkConfigInputSchema = {
  framework: testFrameworkSchema,
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
  framework: testFrameworkSchema,
  test_cmd: z.string().describe('Test command to execute'),
  coverage_cmd: z.string().describe('Coverage command to execute'),
  coverage_format: z.enum(['istanbul', 'llvm-cov']).describe('Coverage output format identifier'),
  coverage_output: z.string().describe('Coverage output file path (relative to project root)'),
  coverage_artifacts: z
    .array(z.string())
    .optional()
    .describe('Glob patterns for coverage artifacts to move to unified location'),
  coverage_cleanup: z
    .array(z.string())
    .optional()
    .describe('Directory/file names to clean up after successful move'),
});
