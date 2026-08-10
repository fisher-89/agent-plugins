import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import type { ProductEnv } from './env';
import { scanTextFiles } from './scan-files';

const NAME_TOKEN_PATTERN = /__(?:CALL_SKILL|CALL_AGENT|SKILL|AGENT|MCP|BIN):[a-z0-9_-]+__/;
const PATH_TOKEN_PATTERN = /__DEV_TEAM_(?:ROOT|RUNTIME_ROOT)__/;

export function assertNoNameTokens(rootDir: string, env: ProductEnv): void {
  if (rootDir == null || rootDir === '') {
    throw new Error('rootDir is required');
  }
  if (env == null) {
    throw new Error('env is required');
  }
  const leftovers: string[] = [];
  for (const rel of scanTextFiles(rootDir)) {
    const content = readFileSync(join(rootDir, rel), 'utf-8');
    if (NAME_TOKEN_PATTERN.test(content)) {
      leftovers.push(rel);
      continue;
    }
    // Plugin builds must clear path tokens at assemble time; home image may keep them.
    if (PATH_TOKEN_PATTERN.test(content)) {
      leftovers.push(rel);
    }
  }
  if (leftovers.length === 0) return;
  throw new Error(
    `[assemble:${env.agent} ${env.layout}] unresolved name-class tokens in:\n  - ${leftovers.join('\n  - ')}`,
  );
}
