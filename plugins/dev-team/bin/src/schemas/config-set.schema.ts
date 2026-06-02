import { z } from 'zod/v4';

export const configSetInputSchema = {
  key: z
    .string()
    .min(1)
    .describe('Key path to write, supports dot-separated nested paths (e.g. "test_scripts.unit")'),
  value: z.unknown().describe('The value to write at the key path'),
  project_root: z
    .string()
    .optional()
    .nullable()
    .describe('Project root directory (defaults to cwd)'),
};

export const configSetOutputSchema = z.object({
  key: z.string().describe('The key path that was written'),
  written: z.boolean().describe('Whether the value was written successfully'),
});
