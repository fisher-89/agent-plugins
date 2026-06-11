import { z } from 'zod/v4';

export const archiValidateInputSchema = {
  source: z
    .string()
    .optional()
    .describe('DSL text to validate (omit to validate current model files)'),
  project_root: z.string().optional().describe('Project root directory (defaults to cwd)'),
};

export const archiValidateOutputSchema = z.object({
  valid: z.boolean().describe('Whether the DSL is valid'),
  error: z.string().optional().describe('Top-level error message'),
  errors: z.array(z.string()).optional().describe('List of validation errors'),
  warnings: z.array(z.unknown()).optional().describe('List of validation warnings'),
});
