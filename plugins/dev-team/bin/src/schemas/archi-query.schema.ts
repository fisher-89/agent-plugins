import { z } from 'zod/v4';

const c4ElementSchema = z.object({
  kind: z.string(),
  name: z.string(),
  paths: z.array(z.string()),
  metadata: z.record(z.string(), z.array(z.string())),
});

const c4RelationSchema = z.object({
  source: z.string(),
  target: z.string(),
  description: z.string().nullable(),
});

export const archiQueryInputSchema = z.object({
  element: z.string().optional().describe('Filter by element FQN (optional)'),
});

export const archiQueryOutputSchema = z.object({
  elements: z.array(c4ElementSchema).optional().describe('All model elements (when no filter)'),
  relationships: z.array(c4RelationSchema).optional().describe('All or filtered relationships'),
  element: c4ElementSchema.optional().describe('Single matched element (when filter is provided)'),
  error: z.string().optional().describe('Error message if query failed'),
});
