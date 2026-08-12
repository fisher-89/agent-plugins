import { z } from 'zod/v4';

import { phaseIdSchema } from './phase-log.schema';
import { projectRootSchema } from './public/project-root.schema';

/**
 * Input schema for phase_next MCP tool.
 * `change` is required; workflow_type is read from change `workflow.json`.
 */
export const phaseNextInputSchema = z.object({
  project_root: projectRootSchema,
  change: z.string().min(1).describe('Change name (corresponds to openspec/changes/<name>)'),
  run_id: z
    .string()
    .trim()
    .min(1)
    .describe('Session window identifier; caller generates a new opaque id per agent turn'),
});

/**
 * Phase agent definition — which agent type and prompt to use.
 */
const phaseAgentSchema = z.object({
  agent_type: z.string().describe('Agent identifier (e.g. __CALL_AGENT:proposal-planner__)'),
  prompt: z.string().describe('Prompt for the agent'),
});

/**
 * Backtrack target phase info — a phase that can be set as backtrack_to.
 */
const backtrackPhaseSchema = z.object({
  id: z.string().describe('Phase identifier (e.g. proposal)'),
  description: z.string().describe('Human-readable phase description'),
});

/**
 * Output schema for a normal phase_next response (next phase available).
 */
export const phaseNextOutputSchema = z.object({
  done: z.boolean().describe('Whether the workflow is complete'),
  error: z.string().nullable().describe('Error code if something went wrong (null on success)'),
  message: z.string().nullable().describe('Human-readable message (error details or info)'),
  next_phase: phaseIdSchema.nullable().describe('Next phase identifier (null if done or error)'),
  executor: phaseAgentSchema
    .nullable()
    .describe('Executor agent config (null for EVAL-ONLY or done/error)'),
  evaluator: phaseAgentSchema.nullable().describe('Evaluator agent config (null if done or error)'),
  allowed_backtrack_phases: z
    .array(backtrackPhaseSchema)
    .describe('Phases that the evaluator can backtrack to from the current phase'),
  last_result: z
    .object({
      phase: phaseIdSchema,
      verdict: z.enum(['pass', 'fail']),
      report: z.string(),
      timestamp: z.iso.datetime(),
    })
    .nullable()
    .describe('Latest eval entry snapshot (null if no entries exist)'),
  total_phases: z.number().int().describe('Total number of phases in this workflow'),
  phase_index: z.number().int().describe('1-based index of the current phase'),
  round: z
    .number()
    .int()
    .describe('Current session window round (1-based); not change lifetime cumulative'),
});
