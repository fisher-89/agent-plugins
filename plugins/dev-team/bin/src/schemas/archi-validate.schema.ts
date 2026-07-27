import { z } from 'zod/v4';

export const archiValidateInputSchema = z.object({
  source: z
    .string()
    .optional()
    .describe('DSL text to validate (omit to validate current model files)'),
});

export const archiValidateOutputSchema = z.object({
  valid: z.boolean().describe('Whether the DSL is valid'),
  error: z.string().optional().describe('Top-level error message'),
  errors: z.array(z.string()).optional().describe('List of validation errors'),
  warnings: z.array(z.unknown()).optional().describe('List of validation warnings'),
});
