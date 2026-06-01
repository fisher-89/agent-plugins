import { z } from 'zod/v4';

export const archiWriteInputSchema = {
  path: z.string().describe('Target file path within models/ directory (required)'),
  source: z.string().describe('DSL text to write (required)'),
  project_root: z.string().optional().describe('Project root directory (defaults to cwd)'),
};

export const archiWriteOutputSchema = z.object({
  success: z.boolean().describe('Whether the write succeeded'),
  path: z.string().optional().describe('Absolute path of the written file'),
  error: z.string().optional().describe('Error message if write failed'),
  validation: z.unknown().optional().describe('Validation result details if validation failed'),
});
