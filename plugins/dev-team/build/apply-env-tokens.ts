import type { ProductEnv } from './env';

export interface ApplyEnvTokensOptions {
  /** When false, leave path tokens untouched. Default: expand when pathReplacePhase==='build'. */
  pathTokens?: boolean;
}

const NAME_TOKEN_REPLACERS: Array<{
  pattern: RegExp;
  resolve: (env: ProductEnv, id: string) => string;
}> = [
  {
    pattern: /__SKILL:([a-z0-9-]+)__/g,
    resolve: (env, id) => `${env.namePrefix}${id}`,
  },
  {
    pattern: /__CALL_SKILL:([a-z0-9-]+)__/g,
    resolve: (env, id) => `${env.pluginPrefix}${env.namePrefix}${id}`,
  },
  {
    pattern: /__AGENT:([a-z0-9-]+)__/g,
    resolve: (env, id) => `${env.namePrefix}${id}`,
  },
  {
    pattern: /__CALL_AGENT:([a-z0-9-]+)__/g,
    resolve: (env, id) => `${env.pluginPrefix}${env.namePrefix}${id}`,
  },
  {
    pattern: /__MCP:([a-z0-9_]+)__/g,
    resolve: (env, id) => `${env.mcpToolPrefix}${id}`,
  },
  {
    pattern: /__BIN:(mcp|cli|hooks)__/g,
    resolve: (env, id) => `${env.namePrefix}${id}.cjs`,
  },
];

function shouldExpandPathTokens(env: ProductEnv, options?: ApplyEnvTokensOptions): boolean {
  if (options?.pathTokens === false) return false;
  if (options?.pathTokens === true) return true;
  return env.pathReplacePhase === 'build';
}

export function applyEnvTokens(
  text: string,
  env: ProductEnv,
  options?: ApplyEnvTokensOptions,
): string {
  let result = text;
  for (const { pattern, resolve } of NAME_TOKEN_REPLACERS) {
    pattern.lastIndex = 0;
    result = result.replace(pattern, (_match, id: string) => resolve(env, id));
  }
  if (shouldExpandPathTokens(env, options)) {
    result = result
      .replaceAll('__DEV_TEAM_ROOT__', env.contentRoot)
      .replaceAll('__DEV_TEAM_RUNTIME_ROOT__', env.runtimeRoot);
  }
  return result;
}
