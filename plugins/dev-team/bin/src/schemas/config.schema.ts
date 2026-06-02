import { z } from 'zod/v4';

/**
 * Zod schema for openspec/config.json.
 *
 * Uses `.passthrough()` to allow extra unknown fields (e.g. tool-specific keys).
 * This ensures backward compatibility with tools that write custom fields.
 *
 * @see design.md (D2 — zod/v4, D3 — .passthrough())
 */
export const configSchema = z
  .object({
    schema: z.literal('spec-driven').default('spec-driven'),
    context: z.string().optional(),
    rules: z
      .object({
        proposal: z.array(z.string()).optional(),
        tasks: z.array(z.string()).optional(),
      })
      .optional(),
  })
  .passthrough();

/** TypeScript type inferred from configSchema — replaces `Record<string, unknown>`. */
export type OpenSpecConfig = z.infer<typeof configSchema>;

/**
 * Parse and validate an unknown value against the config schema.
 * Throws a descriptive error on invalid data.
 * Returns a deep-cloned copy of the validated object.
 */
export function parseConfig(data: unknown): OpenSpecConfig {
  try {
    const result = configSchema.parse(data);
    // Deep clone to avoid external mutation of the returned config
    return structuredClone(result);
  } catch (e) {
    if (e instanceof z.ZodError) {
      throw new Error(`Invalid OpenSpec configuration: ${e.message}`);
    }
    throw e;
  }
}

/**
 * Safely parse and validate an unknown value against the config schema.
 * Never throws — returns a discriminated union result.
 */
export function safeParseConfig(
  data: unknown,
): { success: true; data: OpenSpecConfig } | { success: false; error: z.ZodError } {
  const result = configSchema.safeParse(data);
  if (result.success) {
    return { success: true, data: result.data };
  }
  return { success: false, error: result.error };
}
