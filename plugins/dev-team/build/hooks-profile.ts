import { z } from 'zod/v4';

import { applyEnvTokens } from './apply-env-tokens';
import type { ProductEnv } from './env';

const hooksCanonicalMatcherSchema = z.object({
  claude: z.string().nullable(),
  cursor: z.string().nullable(),
});

const hooksCanonicalPreToolUseSchema = z.object({
  matchers: hooksCanonicalMatcherSchema,
  commandTemplate: z.string(),
});

const hooksCanonicalSubagentStopSchema = z.object({
  matchers: hooksCanonicalMatcherSchema,
  loop_limit: z.number().optional(),
  commandTemplate: z.string(),
});

const hooksCanonicalSchema = z.object({
  description: z.string().optional(),
  preToolUse: z.array(hooksCanonicalPreToolUseSchema),
  subagentStop: z.array(hooksCanonicalSubagentStopSchema),
});

type HooksCanonical = z.infer<typeof hooksCanonicalSchema>;

/** Expand name/path tokens in canonical JSON before platform wrapping. */
function expandCanonical(canonical: HooksCanonical, env: ProductEnv): HooksCanonical {
  const expanded = applyEnvTokens(JSON.stringify(canonical), env, {
    pathTokens: env.pathReplacePhase === 'build',
  });
  return JSON.parse(expanded);
}

/** Wrap already-expanded canonical into Claude Code plugin hooks.json shape. */
function buildClaudeNested(canonical: HooksCanonical): unknown {
  return {
    description: canonical.description,
    hooks: {
      PreToolUse: canonical.preToolUse
        .filter((entry) => entry.matchers.claude)
        .map((entry) => ({
          matcher: entry.matchers.claude,
          hooks: [{ type: 'command', command: entry.commandTemplate }],
        })),
      SubagentStop: canonical.subagentStop.map((entry) => ({
        matcher: entry.matchers.claude,
        hooks: [{ type: 'command', command: entry.commandTemplate }],
      })),
    },
  };
}

/** Wrap already-expanded canonical into Cursor native/plugin hooks.json shape. */
function buildCursorNative(canonical: HooksCanonical): unknown {
  return {
    version: 1,
    hooks: {
      preToolUse: canonical.preToolUse
        .filter((entry) => entry.matchers.cursor)
        .map((entry) => ({
          matcher: entry.matchers.cursor,
          command: entry.commandTemplate,
        })),
      subagentStop: canonical.subagentStop.map((entry) => ({
        matcher: entry.matchers.cursor,
        loop_limit: entry.loop_limit,
        command: entry.commandTemplate,
      })),
    },
  };
}

export function buildHooksFile(canonical: object, env: ProductEnv): string {
  const parsed = hooksCanonicalSchema.parse(canonical);
  const expanded = expandCanonical(parsed, env);
  const doc = env.agent === 'claude' ? buildClaudeNested(expanded) : buildCursorNative(expanded);
  return `${JSON.stringify(doc, null, 2)}\n`;
}
