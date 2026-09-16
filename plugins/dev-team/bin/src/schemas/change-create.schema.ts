import { z } from 'zod/v4';

import { projectRootSchema } from './public/project-root.schema';
import { workflowTypeSchema } from './workflow.schema';

/**
 * kebab-case: lowercase letters/digits with single hyphens.
 * Allowed to start with a digit (e.g. `123-fix`).
 */
export const kebabCasePattern = /^[a-z0-9][a-z0-9-]*$/;

export const changeCreateInputSchema = z.object({
  name: z
    .string()
    .min(1)
    .max(128)
    .regex(
      kebabCasePattern,
      'name 必须为 kebab-case（小写字母/数字，可用 `-` 连接，最长 128 字符）',
    ),
  workflow_type: workflowTypeSchema,
  project_root: projectRootSchema,
});

export type ChangeCreateInput = z.input<typeof changeCreateInputSchema>;

export const changeCreateOutputSchema = z.object({
  name: z.string().describe('Created change directory name (kebab-case)'),
  path: z.string().describe('Absolute path of the created change directory'),
});
