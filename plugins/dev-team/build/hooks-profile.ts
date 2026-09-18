import { z } from 'zod/v4';

import { applyEnvTokens } from './apply-env-tokens';
import { requireProductEnv, type ProductEnv } from './env';

const hooksCanonicalMatcherSchema = z.object({
  claude: z.string().nullable(),
  cursor: z.string().nullable(),
});

const hooksCanonicalPreToolUseSchema = z.object({
  matchers: hooksCanonicalMatcherSchema,
  commandTemplate: z.string(),
});

const hooksCanonicalPostToolUseSchema = z.object({
  matchers: hooksCanonicalMatcherSchema,
  commandTemplate: z.string(),
});

const hooksCanonicalSubagentStopSchema = z.object({
  matchers: hooksCanonicalMatcherSchema,
  loop_limit: z.number().optional(),
  commandTemplate: z.string(),
});

const hooksCanonicalUserPromptSubmitSchema = z.object({
  matchers: hooksCanonicalMatcherSchema,
  commandTemplate: z.string(),
});

const hooksCanonicalSchema = z.object({
  description: z.string().optional(),
  preToolUse: z.array(hooksCanonicalPreToolUseSchema),
  postToolUse: z.array(hooksCanonicalPostToolUseSchema).default([]),
  subagentStop: z.array(hooksCanonicalSubagentStopSchema),
  userPromptSubmit: z.array(hooksCanonicalUserPromptSubmitSchema).default([]),
});

type HooksCanonical = z.infer<typeof hooksCanonicalSchema>;

/** Expand name/path tokens in canonical JSON before platform wrapping. */
function expandCanonical(canonical: HooksCanonical, env: ProductEnv): HooksCanonical {
  const expanded = applyEnvTokens(JSON.stringify(canonical), env);
  return JSON.parse(expanded);
}

/** Wrap already-expanded canonical into Claude Code plugin hooks.json shape. */
function buildClaudeNested(canonical: HooksCanonical): unknown {
  const hooks: Record<string, unknown> = {
    PreToolUse: canonical.preToolUse
      .filter((entry) => entry.matchers.claude)
      .map((entry) => ({
        matcher: entry.matchers.claude,
        hooks: [{ type: 'command', command: entry.commandTemplate }],
      })),
  };

  const postToolUse = canonical.postToolUse
    .filter((entry) => entry.matchers.claude)
    .map((entry) => ({
      matcher: entry.matchers.claude,
      hooks: [{ type: 'command', command: entry.commandTemplate }],
    }));
  if (postToolUse.length > 0) {
    hooks.PostToolUse = postToolUse;
  }

  const subagentStop = canonical.subagentStop
    .filter((entry) => entry.matchers.claude)
    .map((entry) => ({
      matcher: entry.matchers.claude,
      hooks: [{ type: 'command', command: entry.commandTemplate }],
    }));
  if (subagentStop.length > 0) {
    hooks.SubagentStop = subagentStop;
  }

  // UserPromptSubmit takes no matcher: the canonical `claude` value is only a
  // presence flag and is dropped from the wrapped output.
  const userPromptSubmit = canonical.userPromptSubmit
    .filter((entry) => entry.matchers.claude)
    .map((entry) => ({
      hooks: [{ type: 'command', command: entry.commandTemplate }],
    }));
  if (userPromptSubmit.length > 0) {
    hooks.UserPromptSubmit = userPromptSubmit;
  }

  return {
    description: canonical.description,
    hooks,
  };
}

/** Wrap already-expanded canonical into Cursor native/plugin hooks.json shape. */
function buildCursorNative(canonical: HooksCanonical): unknown {
  const hooks: Record<string, unknown> = {};

  const preToolUse = canonical.preToolUse
    .filter((entry) => entry.matchers.cursor)
    .map((entry) => ({
      matcher: entry.matchers.cursor,
      command: entry.commandTemplate,
    }));
  if (preToolUse.length > 0) {
    hooks.preToolUse = preToolUse;
  }

  const postToolUse = canonical.postToolUse
    .filter((entry) => entry.matchers.cursor)
    .map((entry) => ({
      matcher: entry.matchers.cursor,
      command: entry.commandTemplate,
    }));
  if (postToolUse.length > 0) {
    hooks.postToolUse = postToolUse;
  }

  const subagentStop = canonical.subagentStop
    .filter((entry) => entry.matchers.cursor)
    .map((entry) => ({
      matcher: entry.matchers.cursor,
      loop_limit: entry.loop_limit,
      command: entry.commandTemplate,
    }));
  if (subagentStop.length > 0) {
    hooks.subagentStop = subagentStop;
  }

  return {
    version: 1,
    hooks,
  };
}

export function buildHooksFile(canonical: object, env: ProductEnv): string {
  requireProductEnv(env);
  const parsed = hooksCanonicalSchema.parse(canonical);
  const expanded = expandCanonical(parsed, env);
  const doc = env.agent === 'claude' ? buildClaudeNested(expanded) : buildCursorNative(expanded);
  return `${JSON.stringify(doc, null, 2)}\n`;
}
