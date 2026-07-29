import { z } from 'zod/v4';

import { projectRootSchema } from './public/project-root.schema';

export const phaseIdSchema = z
  .enum([
    'proposal',
    'dev-design',
    'test-design',
    'implement',
    'test-gen',
    'test-execution',
    'code-review',
    'acceptance',
    'code-analyze',
  ])
  .describe('Phase identifier');

export const phaseLogSchema = z.object({
  phase: phaseIdSchema,
  attempt: z.number().int().optional().describe('Attempt number (auto-calculated if omitted)'),
  verdict: z.enum(['pass', 'fail']).describe('Evaluation verdict'),
  report: z.string().max(500).describe('Evaluation report text (max 500 chars)'),
  checklist: z
    .array(
      z.object({
        item: z.string().describe('检查项名称，不是ID'),
        pass: z.boolean().describe('是否通过'),
        evidence: z.string().describe('检查通过/不通过的依据'),
      }),
    )
    .describe('Checklist evaluation items'),
  backtrack_to: z
    .string()
    .min(1)
    .refine((v) => v !== 'null', {
      message:
        'backtrack_to 不能为字符串 "null"。若要表示空值，请传入 null（不传引号）或不传该字段',
    })
    .optional()
    .nullable()
    .describe('Backtrack target phase identifier (string, array of strings, or null)'),
  backtrack_reason: z
    .string()
    .max(500)
    .optional()
    .nullable()
    .describe('回溯原因（backtrack_to 非空时必填，最长 500 字符）'),
  skipped: z
    .boolean()
    .optional()
    .describe('Mark entry as skipped (no-op phase, requires verdict pass)'),
  timestamp: z.iso.datetime().describe('Recorded at'),
  stale: z.boolean().optional().describe('Phase need redo'),
});

export const phaseLogInputSchema = phaseLogSchema
  .pick({
    phase: true,
    attempt: true,
    report: true,
    checklist: true,
    skipped: true,
  })
  .extend({
    project_root: projectRootSchema,
    change: z.string().describe('Change name'),
  });

export const phaseLogOutputSchema = z.object({
  written: z.boolean().describe('Whether the entry was written successfully'),
  phase: z.string().describe('The phase identifier that was logged'),
  attempt: z.number().int().describe('The attempt number for this phase'),
});
