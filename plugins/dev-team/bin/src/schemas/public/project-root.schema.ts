import { z } from 'zod/v4';

export const projectRootSchema = z
  .string()
  .describe(
    'Absolute agent workspace folder for this call (will return error if not match a collected candidate)',
  );
