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
    static_analysis: z.string().optional(),
    test: z
      .object({
        frameworks: z
          .union([
            z.enum(['jest', 'vitest', 'vite-plus', 'bun', 'rust']),
            z.array(
              z.object({
                glob: z.string(),
                framework: z.string(),
              }),
            ),
          ])
          .optional(),
        coverage: z
          .object({
            thresholds: z
              .object({
                lines: z.number().default(80),
                branches: z.number().default(70),
                functions: z.number().default(75),
              })
              .optional(),
            overrides: z
              .array(
                z.object({
                  glob: z.string(),
                  thresholds: z.object({
                    lines: z.number().optional(),
                    branches: z.number().optional(),
                    functions: z.number().optional(),
                  }),
                }),
              )
              .optional(),
          })
          .optional(),
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
