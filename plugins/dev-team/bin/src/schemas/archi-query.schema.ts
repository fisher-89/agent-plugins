import { z } from 'zod/v4';

export const archiQueryInputSchema = {
  element: z.string().optional().describe('Filter by element FQN (optional)'),
  project_root: z.string().optional().describe('Project root directory (defaults to cwd)'),
};

export const archiQueryOutputSchema = z.object({
  elements: z.array(z.unknown()).optional().describe('All model elements (when no filter)'),
  relationships: z.array(z.unknown()).optional().describe('All or filtered relationships'),
  element: z.unknown().optional().describe('Single matched element (when filter is provided)'),
  error: z.string().optional().describe('Error message if query failed'),
});
