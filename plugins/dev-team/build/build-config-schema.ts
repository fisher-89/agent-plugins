import { writeFileSync } from 'node:fs';

import { configSchema } from '../bin/src/schemas';

export function generateConfigJsonSchema(): void {
  const jsonSchemaContent = JSON.stringify(configSchema.toJSONSchema(), null, 2);
  writeFileSync('bin/dev-team-config.schema.json', jsonSchemaContent, 'utf-8');
}
