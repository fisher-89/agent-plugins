import { z } from 'zod/v4';

export const changeListInputSchema = {
  project_root: z
    .string()
    .optional()
    .nullable()
    .describe('Project root directory (defaults to cwd)'),
};

const changeEntrySchema = z.object({
  name: z.string().describe('Change directory name (kebab-case)'),
  artifacts: z
    .array(z.string())
    .describe('Present artifact filenames (e.g. proposal.md, design.md, tasks.md, eval.json)'),
  tasks: z
    .object({
      total: z.number().describe('Total task count from tasks.md'),
      done: z.number().describe('Completed task count'),
    })
    .nullable()
    .describe('Task progress from tasks.md, null if tasks.md does not exist'),
  latest_phase: z
    .object({
      phase: z.string().describe('Phase identifier of the latest eval entry'),
      verdict: z.string().describe('Verdict of the latest eval entry (pass/fail)'),
      stale: z.boolean().optional().describe('Whether the latest entry is stale'),
    })
    .nullable()
    .describe('Latest eval.json entry summary, null if eval.json does not exist or is empty'),
});

export const changeListOutputSchema = z.object({
  changes: z.array(changeEntrySchema).describe('List of active (non-archived) changes'),
  count: z.number().describe('Number of active changes'),
});
