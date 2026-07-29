import { z } from 'zod/v4';

import { projectRootSchema } from './public/project-root.schema';

export const archiCheckInputSchema = z.object({
  project_root: projectRootSchema,
  staged: z.boolean().optional().describe('Check git staged files'),
  files: z.string().optional().describe('Comma-separated file list to check'),
});

export const archiCheckOutputSchema = z.object({
  violations: z.array(z.unknown()).describe('List of cross-reference violations found'),
  warnings: z.array(z.string()).describe('List of warnings'),
  matched: z.array(z.unknown()).describe('Files matched to model elements'),
  unmatched_files: z.array(z.string()).describe('Files not matching any model element'),
  status: z
    .enum(['clean', 'violations_found', 'no_changes', 'skipped'])
    .describe('Overall check status'),
});
