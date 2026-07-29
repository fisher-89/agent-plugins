import { z } from 'zod/v4';

import { projectRootSchema } from './public/project-root.schema';

/**
 * Input schema for `test_resolve_paths` MCP tool.
 *
 * Accepts a module path list (files or directories relative to project_root).
 */
export const testResolvePathsInputSchema = z.object({
  project_root: projectRootSchema,
  modules: z
    .union([
      z
        .array(z.string())
        .describe(
          'Module paths (files or directories, relative to project_root). When empty, directories are auto-detected from config.json test configuration.',
        ),
      z
        .literal('git-change')
        .describe(
          'Read git diff HEAD --name-only to get changed files, then resolve test paths filtered by test config.',
        ),
    ])
    .describe('Module paths or "git-change" to auto-detect from git diff'),
});

export const unitTestEntrySchema = z.object({
  source: z.string().describe('Source file path (relative to project_root)'),
  test_file: z.string().describe('Derived unit test file path'),
});

/**
 * Output schema for `test_resolve_paths` MCP tool.
 *
 * Returns derived unit test paths and any per-module parse errors.
 */
export const testResolvePathsOutputSchema = z.object({
  unit_tests: z.array(unitTestEntrySchema).describe('Derived unit test paths'),
  errors: z.array(
    z.object({
      path: z.string().describe('Module path that failed to resolve'),
      message: z.string().describe('Error description'),
    }),
  ),
});
