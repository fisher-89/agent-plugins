import { z } from 'zod/v4';

export const archiWriteInputSchema = z.object({
  path: z.string().describe('Target file path within models/ directory (required)'),
  source: z.string().describe('DSL text to write (required)'),
});

export const archiWriteOutputSchema = z.object({
  success: z.boolean().describe('Whether the write succeeded'),
  path: z.string().optional().describe('Absolute path of the written file'),
  error: z.string().optional().describe('Error message if write failed'),
  validation: z.unknown().optional().describe('Validation result details if validation failed'),
});
