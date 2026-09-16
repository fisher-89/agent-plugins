import type { ProductEnv } from './env';

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

export function applyEnvTokens(text: string, env: ProductEnv): string {
  let result = text;
  for (const { pattern, resolve } of NAME_TOKEN_REPLACERS) {
    pattern.lastIndex = 0;
    result = result.replace(pattern, (_match, id: string) => resolve(env, id));
  }
  result = result
    .replaceAll('__DEV_TEAM_ROOT__', env.pluginRoot)
    .replaceAll('__MODEL_HIGH__', env.modelHigh)
    .replaceAll('__MODEL_FAST__', env.modelFast)
    .replaceAll('__TOOL_WRITE__', env.toolWrite)
    .replaceAll('__TOOL_EDIT__', env.toolEdit)
    .replaceAll('__TOOL_BASH__', env.toolBash)
    .replaceAll('__TOOL_CMD__', env.toolCmd)
    .replaceAll('__TOOL_ASK_USER__', env.toolAskUser)
  return result;
}
