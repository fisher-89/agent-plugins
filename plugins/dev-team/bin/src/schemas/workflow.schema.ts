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

/**
 * Shape of `openspec/changes/<name>/workflow.json`, created by `change_create`
 * and required by `phase_next` / `backtrack` / `phase_log`.
 *
 * - `workflow_type` is required and restricted to `workflowTypeSchema`;
 * - `created` is optional and, when present, MUST be `YYYY-MM-DD`;
 * - `eval` (the change's evaluation history) is optional;
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
});

export type WorkflowFile = z.infer<typeof workflowFileSchema>;
