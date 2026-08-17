import { z } from 'zod/v4';

import { projectRootSchema } from './public/project-root.schema';

export const specListInputSchema = z.object({
  project_root: projectRootSchema,
});

export const specListOutputSchema = z.object({
  project_root: z.string().describe('项目根目录'),
  specs: z.array(
    z.object({
      name: z.string().describe('Capability directory name (kebab-case)'),
      path: z.string().describe('Absolute path of the spec.md file'),
      description: z.string().describe('Description extracted from the `## <name>` heading'),
    }),
  ),
});
