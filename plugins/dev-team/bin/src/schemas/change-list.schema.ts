import { z } from 'zod/v4';

import { projectRootSchema } from './public/project-root.schema';

export const changeListInputSchema = z.object({
  project_root: projectRootSchema,
});

const changeEntrySchema = z.object({
  name: z.string().describe('Change directory name (kebab-case)'),
  artifacts: z
    .array(z.string())
    .describe(
      'Present artifact filenames (e.g. proposal.md, design.md, tasks.md); evaluation history is not an artifact — it lives in workflow.json',
    ),
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
    .describe(
      'Latest evaluation entry summary from workflow.json eval; null when there is no evaluation entry, or when workflow.json is missing / malformed',
    ),
  workflow_done: z
    .boolean()
    .describe(
      '所有 phase 是否已完成（非 stale 的 pass/skipped）；workflow.json 缺失或格式非法时为 false',
    ),
});

export const changeListOutputSchema = z.object({
  project_root: z.string().describe('项目根目录'),
  changes: z.array(changeEntrySchema).describe('List of active (non-archived) changes'),
  count: z.number().describe('Number of active changes'),
});
