import { z } from 'zod/v4';

export const configUnsetInputSchema = {
  key: z
    .string()
    .min(1)
    .describe('Key path to delete, supports dot-separated nested paths (e.g. "test_scripts.e2e")'),
  project_root: z
    .string()
    .optional()
    .nullable()
    .describe('Project root directory (defaults to cwd)'),
};

export const configUnsetOutputSchema = z.object({
  key: z.string().describe('The key path that was requested for deletion'),
  removed: z
    .boolean()
    .describe('Whether the key was actually removed (false if key did not exist)'),
});
