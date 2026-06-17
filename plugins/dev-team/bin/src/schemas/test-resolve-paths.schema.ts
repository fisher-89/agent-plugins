import { z } from 'zod/v4';

/**
 * Input schema for `test_resolve_paths` MCP tool.
 *
 * Accepts a module path list (files or directories relative to project_root),
 * optional integration test scenarios, and optional extension override.
 */
export const testResolvePathsInputSchema = z.object({
  modules: z
    .array(z.string())
    .min(1)
    .describe('Module paths (files or directories, relative to project_root)'),
  integration_scenarios: z.array(z.string()).optional().describe('Integration test scenario names'),
  extension: z
    .string()
    .optional()
    .describe('Integration test file extension (e.g. "ts", "py"; leading dot optional)'),
  integration_root: z
    .string()
    .optional()
    .describe('Parent directory of __tests__/ (relative to project_root)'),
  project_root: z
    .string()
    .optional()
    .nullable()
    .describe('Project root directory (defaults to cwd)'),
});

/**
 * Output schema for `test_resolve_paths` MCP tool.
 *
 * Returns derived unit/integration test paths and any per-module parse errors.
 */
export const testResolvePathsOutputSchema = z.object({
  unit_tests: z.array(
    z.object({
      source: z.string().describe('Source file path (relative to project_root)'),
      test_file: z.string().describe('Derived unit test file path'),
    }),
  ),
  integration_tests: z.array(
    z.object({
      scenario: z.string().describe('Integration test scenario name'),
      test_file: z.string().describe('Derived integration test file path'),
    }),
  ),
  errors: z.array(
    z.object({
      path: z.string().describe('Module path that failed to resolve'),
      message: z.string().describe('Error description'),
    }),
  ),
});
