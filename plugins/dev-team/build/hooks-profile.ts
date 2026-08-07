import { z } from 'zod/v4';

import { applyEnvTokens } from './apply-env-tokens';
import type { ProductEnv } from './env';

const hooksCanonicalMatcherSchema = z.object({
  claudeNested: z.string(),
  cursorNative: z.string(),
});

const hooksCanonicalPreToolUseSchema = z.object({
  matchers: hooksCanonicalMatcherSchema,
  commandTemplate: z.string(),
});

const hooksCanonicalSubagentStopSchema = z.object({
  agentLogicalId: z.string(),
  loop_limit: z.number().optional(),
  commandTemplate: z.string(),
});

export const hooksCanonicalSchema = z.object({
  description: z.string().optional(),
  preToolUse: z.array(hooksCanonicalPreToolUseSchema),
  subagentStop: z.array(hooksCanonicalSubagentStopSchema),
});

export type HooksCanonical = z.infer<typeof hooksCanonicalSchema>;

function expandCommand(template: string, env: ProductEnv): string {
  return applyEnvTokens(template, env, {
    pathTokens: env.pathReplacePhase === 'build',
  });
}

function buildClaudeNested(canonical: HooksCanonical, env: ProductEnv): unknown {
  return {
    description: canonical.description,
    hooks: {
      PreToolUse: canonical.preToolUse.map((entry) => ({
        matcher: entry.matchers.claudeNested,
        hooks: [{ type: 'command', command: expandCommand(entry.commandTemplate, env) }],
      })),
      SubagentStop: canonical.subagentStop.map((entry) => ({
        matcher: `${env.namePrefix}${entry.agentLogicalId}`,
        loop_limit: entry.loop_limit,
        hooks: [{ type: 'command', command: expandCommand(entry.commandTemplate, env) }],
      })),
    },
  };
}

function buildCursorNative(canonical: HooksCanonical, env: ProductEnv): unknown {
  return {
    version: 1,
    hooks: {
      preToolUse: canonical.preToolUse.map((entry) => ({
        matcher: entry.matchers.cursorNative,
        command: expandCommand(entry.commandTemplate, env),
      })),
      subagentStop: canonical.subagentStop.map((entry) => ({
        matcher: `${env.namePrefix}${entry.agentLogicalId}`,
        loop_limit: entry.loop_limit,
        command: expandCommand(entry.commandTemplate, env),
      })),
    },
  };
}

export function buildHooksFile(canonical: HooksCanonical, env: ProductEnv): string {
  const doc =
    env.hooksProfile === 'claudeNested'
      ? buildClaudeNested(canonical, env)
      : buildCursorNative(canonical, env);
  return `${JSON.stringify(doc, null, 2)}\n`;
}
