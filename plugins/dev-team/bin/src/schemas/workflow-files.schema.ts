import { z } from 'zod/v4';

import { projectRootSchema } from './public/project-root.schema';

/**
 * Input schema for `workflow_files` MCP tool (read-only query).
 */
export const workflowFilesInputSchema = z.object({
  change: z.string().min(1).describe('Target change name (openspec/changes/<change>)'),
  project_root: projectRootSchema,
});

/**
 * Output schema for `workflow_files`: the net state of the change file
 * inventory (`workflow.json.files`). The `source` audit map is structurally
 * excluded — it is audit-only and consumers MUST NOT read it.
 */
export const workflowFilesOutputSchema = z.object({
  written: z.array(z.string()).describe('当前净写入路径列表（相对项目根 POSIX 风格）'),
  deleted: z.array(z.string()).describe('当前净删除路径列表（相对项目根 POSIX 风格）'),
});
