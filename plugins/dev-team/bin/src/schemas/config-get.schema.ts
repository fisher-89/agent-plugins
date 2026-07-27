import { z } from 'zod/v4';

import { configSchema } from './config/config.schema';

export const configGetInputSchema = z.object({
  key: z
    .enum(configSchema.keyof().options)
    .describe('Key path to read, supports dot-separated nested paths (e.g. "test_scripts.unit")'),
});

export const configGetOutputSchema = z.object({
  key: z.string().describe('The requested key path'),
  value: z.unknown().optional().describe('The value at the key path, if it exists'),
  exists: z.boolean().describe('Whether the key path exists in the config'),
});
