import { z } from 'zod/v4';

export const evalLogInputSchema = {
  change: z.string().describe('Change name (corresponds to openspec/changes/<name>)'),
  phase: z.string().describe('Phase identifier (e.g. 01-requirements)'),
  verdict: z.enum(['pass', 'fail']).describe('Evaluation verdict'),
  report: z.string().max(500).describe('Evaluation report text (max 500 chars)'),
  items: z
    .string()
    .describe(
      'Checklist evaluation items as JSON array string. Each item: {"item":"...","pass":true|false,"evidence":"...","notes":"..."}',
    ),
  attempt: z.number().int().optional().describe('Attempt number (auto-calculated if omitted)'),
  backtrack_to: z.string().optional().describe('Backtrack target phase identifier'),
  skipped: z
    .boolean()
    .optional()
    .describe('Mark entry as skipped (no-op phase, requires verdict pass)'),
  findings: z.string().optional().describe('Diagnostic findings text from decision tree analysis'),
};

export const evalLogOutputSchema = z.object({
  written: z.boolean().describe('Whether the entry was written successfully'),
  phase: z.string().describe('The phase identifier that was logged'),
  attempt: z.number().int().describe('The attempt number for this phase'),
});
