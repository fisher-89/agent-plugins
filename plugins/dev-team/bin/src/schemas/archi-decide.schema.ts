import { z } from 'zod/v4';

import { projectRootSchema } from './public/project-root.schema';

const adrStatusSchema = z.enum(['proposed', 'accepted', 'deprecated', 'superseded']);

export type AdrStatus = z.infer<typeof adrStatusSchema>;

const archiDecideAlternativeSchema = z.object({
  name: z.string().describe('Alternative name'),
  description: z.string().optional().describe('Brief description of the alternative'),
  pros: z
    .union([z.string(), z.array(z.string())])
    .optional()
    .describe('Pros of this alternative'),
  cons: z
    .union([z.string(), z.array(z.string())])
    .optional()
    .describe('Cons of this alternative'),
});

export type ArchiDecideAlternative = z.infer<typeof archiDecideAlternativeSchema>;

const archiDecideCreateInputBranch = z.object({
  action: z.literal('create'),
  project_root: projectRootSchema,
  title: z.string().describe('ADR title'),
  background: z.string().describe('Background context'),
  decision: z.string().describe('The decision'),
  consequences: z.string().optional().describe('Consequences text'),
  alternatives: z.array(archiDecideAlternativeSchema).optional().describe('Alternative options'),
  scope: z.array(z.string()).optional().describe('Affected model element IDs or FQNs'),
  status: adrStatusSchema.optional().describe('Initial status (defaults to proposed)'),
});

const archiDecideListInputBranch = z.object({
  action: z.literal('list'),
  project_root: projectRootSchema,
  status: adrStatusSchema.optional().describe('Filter by status'),
});

const archiDecideUpdateInputBranch = z.object({
  action: z.literal('update'),
  project_root: projectRootSchema,
  file: z.string().describe('ADR filename (e.g. 2026-05-12-use-postgresql.md)'),
  status: adrStatusSchema.describe('New status value'),
  superseded_by: z
    .string()
    .optional()
    .describe('Reference to superseding ADR (required when status is superseded)'),
});

export const archiDecideInputSchema = z.discriminatedUnion('action', [
  archiDecideCreateInputBranch,
  archiDecideListInputBranch,
  archiDecideUpdateInputBranch,
]);

const archiDecideCreateOutputBranch = z.object({
  action: z.literal('create'),
  success: z.boolean(),
  path: z.string().optional(),
  filename: z.string().optional(),
  error: z.string().optional(),
});

const archiDecideListEntrySchema = z.object({
  filename: z.string(),
  title: z.string(),
  status: z.string(),
  date: z.string(),
  scope: z.array(z.string()),
  path: z.string(),
});

const archiDecideListOutputBranch = z.object({
  action: z.literal('list'),
  adrs: z.array(archiDecideListEntrySchema),
  count: z.number(),
});

const archiDecideUpdateOutputBranch = z.object({
  action: z.literal('update'),
  success: z.boolean(),
  path: z.string().optional(),
  old_status: z.string().optional(),
  new_status: z.string().optional(),
  error: z.string().optional(),
});

export const archiDecideOutputSchema = z.discriminatedUnion('action', [
  archiDecideCreateOutputBranch,
  archiDecideListOutputBranch,
  archiDecideUpdateOutputBranch,
]);

export type ArchiDecideCreateInput = Omit<
  z.input<typeof archiDecideCreateInputBranch>,
  'action' | 'project_root'
>;

export type ArchiDecideUpdateInput = Omit<
  z.input<typeof archiDecideUpdateInputBranch>,
  'action' | 'project_root'
>;

export type ArchiDecideCreateResult = Omit<
  z.output<typeof archiDecideCreateOutputBranch>,
  'action'
>;

export type ArchiDecideListResult = Omit<z.output<typeof archiDecideListOutputBranch>, 'action'>;

export type ArchiDecideUpdateResult = Omit<
  z.output<typeof archiDecideUpdateOutputBranch>,
  'action'
>;
