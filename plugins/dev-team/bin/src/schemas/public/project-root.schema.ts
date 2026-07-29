import { z } from 'zod/v4';

export const projectRootSchema = z
  .string()
  .describe(
    'Absolute host workspace folder for this call (must match a collected candidate, or be force-confirmed by resubmitting identical arguments)',
  );
