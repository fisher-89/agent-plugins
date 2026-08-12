import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import type { ProductEnv } from './env';

const FRAGMENTS_ROOT = '_fragments';
const INCLUDE_MAX_DEPTH = 16;
const INCLUDE_TOKEN_PATTERN = /__INCLUDE:([a-z0-9-]+)__/g;
const VALID_ID_PATTERN = /^[a-z0-9-]+$/;

function sourcePath(...segments: string[]): string {
  return join(process.cwd(), ...segments);
}

function resolveFragment(id: string, env: ProductEnv): string {
  const agentPath = join(FRAGMENTS_ROOT, `${id}.${env.agent}.md`);
  if (existsSync(sourcePath(agentPath))) return agentPath;
  const defaultPath = join(FRAGMENTS_ROOT, `${id}.md`);
  if (existsSync(sourcePath(defaultPath))) return defaultPath;
  throw new Error(
    `Missing fragment "${id}" for agent "${env.agent}" (looked for ${agentPath}, ${defaultPath})`,
  );
}

export function expandIncludes(
  text: string,
  env: ProductEnv,
  stack: readonly string[] = [],
  depth = 0,
): string {
  if (depth > INCLUDE_MAX_DEPTH) {
    throw new Error(`Include depth exceeded ${INCLUDE_MAX_DEPTH}: ${stack.join(' → ')}`);
  }

  INCLUDE_TOKEN_PATTERN.lastIndex = 0;
  return text.replace(INCLUDE_TOKEN_PATTERN, (_match, id: string) => {
    if (!VALID_ID_PATTERN.test(id)) {
      throw new Error(`Invalid include id "${id}" in token __INCLUDE:${id}__`);
    }
    if (stack.includes(id)) {
      throw new Error(`Include cycle detected: ${[...stack, id].join(' → ')}`);
    }

    const fragmentPath = resolveFragment(id, env);
    const fragmentBody = readFileSync(sourcePath(fragmentPath), 'utf-8');
    const expanded = expandIncludes(fragmentBody, env, [...stack, id], depth + 1);
    return expanded.trim();
  });
}
