import { z } from 'zod/v4';

/**
 * Input schema for backtrack MCP tool.
 * Sets the backtrack target and reason for a phase entry in eval.json.
 */
export const backtrackInputSchema = z.object({
  change: z.string().min(1).describe('Change name (corresponds to openspec/changes/<name>)'),
  phase: z.string().min(1).describe('Current phase identifier to set backtrack on'),
  backtrack_to: z.string().min(1).describe('Target phase identifier to backtrack to'),
  backtrack_reason: z.string().max(500).describe('Reason for backtracking (max 500 chars)'),
});

/**
 * Output schema for backtrack MCP tool.
 */
export const backtrackOutputSchema = z.object({
  modified: z.boolean().describe('Whether the entry was modified successfully'),
  phase: z.string().describe('The phase identifier that was modified'),
  target: z.string().describe('The backtrack target phase identifier'),
});
