import { z } from 'zod/v4';

export const evalCheckInputSchema = {
  change: z.string().describe('Change name (corresponds to openspec/changes/<name>)'),
  phase: z.string().describe('Phase identifier (e.g. 02-dev-design)'),
};

export const evalCheckOutputSchema = z.object({
  passed: z.boolean().describe('Whether all gate checks passed'),
  phase: z.string().describe('The phase being checked'),
  prior_phases: z.array(z.string()).describe('Prior phases that were checked'),
  block_reasons: z.array(z.string()).describe('Reasons why the gate is blocked'),
  phase_state: z.enum(['first_run', 'retry', 'passed']).describe('Current state of the phase'),
  details: z.object({
    prior_phase_gate: z.object({
      passed: z.boolean(),
      missing: z.array(z.string()),
    }),
    timestamp_order: z.object({
      passed: z.boolean(),
      order_valid: z.boolean(),
    }),
    backtrack: z.object({
      passed: z.boolean(),
      active_backtrack_phases: z.array(z.string()),
    }),
  }),
});
