import { z } from 'zod/v4';

import { phaseLogSchema } from './phase-log.schema';

/**
 * Value space of `workflow.json`'s `workflow_type` field — the single source
 * shared by `change-create.schema.ts` (writer) and `workflowFileSchema` (reader).
 */
export const workflowTypeSchema = z
  .enum(['requirement', 'bug-fix', 'refactor', 'test-only'])
  .describe('PGE workflow type');

/** Shape of the evaluation history stored in `workflow.json.eval`. */
export const workflowEvalSchema = z.array(phaseLogSchema);

/** Shape of the change file inventory net state stored in `workflow.json.files`. */
export const workflowFilesSchema = z.object({
  written: z
    .array(z.string())
    .describe('净写入路径（相对项目根 POSIX 风格；目录删除记录目录路径）'),
  deleted: z.array(z.string()).describe('净删除路径（语义同 written）'),
  source: z
    .record(z.string(), z.string())
    .optional()
    .describe('旁挂来源映射：路径 → 事件 agent_type，仅审计用途，消费方 MUST NOT 读取'),
});

/**
 * Shape of `openspec/changes/<name>/workflow.json`, created by `change_create`
 * and required by `phase_next` / `backtrack` / `phase_log`.
 *
 * - `workflow_type` is required and restricted to `workflowTypeSchema`;
 * - `created` is optional and, when present, MUST be `YYYY-MM-DD`;
 * - `eval` (the change's evaluation history) is optional;
 * - `files` (the change file inventory net state) is optional here; it is
 *   initialized by `change_create` and consumers hard-error when missing
 *   (legacy change created before the inventory mechanism — rebuild required);
 * - unknown keys are allowed and preserved as-is on write.
 */
export const workflowFileSchema = z.looseObject({
  workflow_type: workflowTypeSchema,
  created: z
    .string()
    .regex(/^\d{4}-\d{2}-\d{2}$/)
    .optional()
    .describe('change_create 写入 YYYY-MM-DD；缺失不视为非法'),
  eval: workflowEvalSchema.optional().describe('评估历史条目数组'),
  files: workflowFilesSchema
    .optional()
    .describe('文件清单净状态；change_create 初始化，消费方对缺失硬报错'),
});

export type WorkflowFile = z.infer<typeof workflowFileSchema>;
