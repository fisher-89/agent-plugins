import { z } from 'zod/v4';

export const configContextInputSchema = {
  context: z
    .string()
    .optional()
    .describe('New context value to write. Omit to read current context'),
  project_root: z.string().optional().describe('Project root directory (defaults to cwd)'),
};

export const configContextOutputSchema = z.object({
  context: z.string().describe('The current context value'),
  written: z
    .boolean()
    .optional()
    .describe('Whether the context was written (present when writing)'),
});
