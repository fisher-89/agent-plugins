import { z } from 'zod/v4';

/**
 * Input schema for phase_next MCP tool.
 * `change` is required; workflow_type is read from change `workflow.json`.
 */
export const phaseNextInputSchema = z.object({
  change: z.string().min(1).describe('Change name (corresponds to openspec/changes/<name>)'),
});

/**
 * Phase agent definition — which agent type and prompt to use.
 */
const phaseAgentSchema = z.object({
  agent_type: z.string().describe('Agent identifier (e.g. dev-team:proposal-planner)'),
  prompt: z.string().describe('Prompt for the agent'),
});

/**
 * Output schema for a normal phase_next response (next phase available).
 */
export const phaseNextOutputSchema = z.object({
  done: z.boolean().describe('Whether the workflow is complete'),
  error: z.string().nullable().describe('Error code if something went wrong (null on success)'),
  message: z.string().nullable().describe('Human-readable message (error details or info)'),
  next_phase: z.string().nullable().describe('Next phase identifier (null if done or error)'),
  phase_pattern: z
    .string()
    .nullable()
    .describe('Phase pattern: DESIGN | EXEC | EVAL-ONLY (null if done or error)'),
  planner: phaseAgentSchema
    .nullable()
    .describe('Planner agent config (null for EVAL-ONLY or done/error)'),
  evaluator: phaseAgentSchema.nullable().describe('Evaluator agent config (null if done or error)'),
  auto_steps: z
    .array(z.string())
    .describe('Bash commands to execute between planner and evaluator'),
  total_phases: z.number().int().describe('Total number of phases in this workflow'),
  phase_index: z.number().int().describe('1-based index of the current phase'),
  round: z.number().int().describe('Current workflow round (1-based)'),
});
