import { z } from 'zod/v4';

import { phaseIdSchema } from './phase-log.schema';
import { projectRootSchema } from './public/project-root.schema';

/**
 * Input schema for the phase_start MCP tool.
 * `change` is required; the phase MUST belong to the change's
 * `workflow_type` phase table (validated by the command layer).
 */
export const phaseStartInputSchema = z.object({
  change: z.string().min(1).describe('Change name'),
  phase: phaseIdSchema.describe('Phase identifier to mark as running'),
  project_root: projectRootSchema,
});

/**
 * Output schema for phase_start: the running state written to
 * `workflow.json.active_phase` (last-wins on re-entry).
 */
export const phaseStartOutputSchema = z.object({
  started: z.literal(true).describe('Whether the running state was written'),
  phase: z.string().describe('The phase identifier that was marked as running'),
  attempt: z.number().int().describe('The attempt number derived from the eval history'),
  start_at: z.iso.datetime().describe('Attempt start timestamp (ISO 8601)'),
});

export type PhaseStartOptions = z.input<typeof phaseStartInputSchema>;

export type PhaseStartResult = z.output<typeof phaseStartOutputSchema>;
