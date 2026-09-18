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

/** Running phase state (`workflow.json.active_phase`), written by `phase_start`. */
export const activePhaseSchema = z.object({
  phase: z.string().describe('运行中的 phase 标识符'),
  attempt: z.number().int().describe('本次尝试序号（与 eval 同 phase 条目数 + 1 对齐）'),
  start_at: z.iso.datetime().describe('本次尝试的开始时刻（ISO 8601）'),
});

/** One archived interrupted attempt (`workflow.json.interrupted[]` entry). */
export const interruptedEntrySchema = activePhaseSchema.extend({
  end_at: z.iso.datetime().describe('中断收口时刻（ISO 8601）'),
});

/**
 * One file_log entry — the log-structured change file inventory
 * (`workflow.json.file_log`). `scope` is the recording context: a phase id
 * for phase-scoped (executor) writes, `'workflow'` for everything else;
 * `attempt` is present only for phase-scoped entries.
 */
export const fileLogSchema = z.object({
  op: z.enum(['write', 'delete', 'revert']).describe('记录的文件操作'),
  scope: z.string().describe('记录来源：phase id 或 workflow'),
  attempt: z.number().int().optional().describe('phase scope 携带的尝试序号'),
  path: z.string().describe('净写入路径（相对项目根 POSIX 风格；目录删除记录目录路径）'),
  at: z.iso.datetime().describe('记录时刻（ISO 8601）'),
});

/**
 * Shape of `openspec/changes/<name>/workflow.json`, created by `change_create`
 * and required by `phase_next` / `backtrack` / `phase_log`.
 *
 * - `workflow_type` is required and restricted to `workflowTypeSchema`;
 * - `created` is optional and, when present, MUST be `YYYY-MM-DD`;
 * - `eval` (the change's evaluation history) is optional;
 * - `file_log` (the log-structured change file inventory) is optional here; it
 *   is initialized by `change_create` and consumers hard-error when missing
 *   (legacy change created before the log mechanism — rebuild required);
 * - `active_phase` (running phase state) is optional and may be `null`;
 * - `interrupted` (archived interrupted attempts) is optional;
 * - unknown keys are allowed and preserved as-is on write (a legacy `files`
 *   bucket or `source` side-map falls into this bucket untouched).
 */
export const workflowFileSchema = z.looseObject({
  workflow_type: workflowTypeSchema,
  created: z
    .string()
    .regex(/^\d{4}-\d{2}-\d{2}$/)
    .optional()
    .describe('change_create 写入 YYYY-MM-DD；缺失不视为非法'),
  eval: workflowEvalSchema.optional().describe('评估历史条目数组'),
  file_log: z
    .array(fileLogSchema)
    .optional()
    .describe('日志式文件清单；change_create 初始化为 []，消费方对缺失硬报错'),
  active_phase: activePhaseSchema
    .nullable()
    .optional()
    .describe('运行中的 phase 状态；phase_start 写入，phase_log 清场后置 null'),
  interrupted: z
    .array(interruptedEntrySchema)
    .optional()
    .describe('中断留档：sweep 从 active_phase 归档的条目，仅留档无消费方'),
});

export type WorkflowFile = z.infer<typeof workflowFileSchema>;
