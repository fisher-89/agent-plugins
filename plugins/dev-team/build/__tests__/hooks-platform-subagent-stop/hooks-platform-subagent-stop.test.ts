/**
 * 集成测试: hooks.canonical.json → buildHooksFile → 三平台 hooks 产物
 *
 * @see openspec/changes/cursor-omit-subagent-stop/test-design.md
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vite-plus/test';

import { getEnv } from '../../env';
import { buildHooksFile } from '../../hooks-profile';

const canonicalPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../hooks/hooks.canonical.json',
);

describe('真实 canonical 三平台 hooks 形态', () => {
  const canonical = JSON.parse(readFileSync(canonicalPath, 'utf-8')) as {
    subagentStop: Array<{
      matchers: { claude: string; cursor: string | null };
      loop_limit?: number;
      commandTemplate: string;
    }>;
  };

  it('真实 canonical：matchers.cursor 两条均为 null', () => {
    expect(canonical.subagentStop).toHaveLength(2);
    for (const entry of canonical.subagentStop) {
      expect(entry.matchers.cursor).toBeNull();
    }
  });

  it('cursor / cursorHome：hooks 无 subagentStop 键', () => {
    for (const key of ['cursor', 'cursorHome'] as const) {
      const parsed = JSON.parse(buildHooksFile(canonical, getEnv(key))) as {
        hooks: Record<string, unknown>;
      };
      expect(Object.hasOwn(parsed.hooks, 'subagentStop')).toBe(false);
    }
  });

  it('claude：SubagentStop 含两 generator matcher 且 command 含 static-check', () => {
    const parsed = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as {
      hooks: {
        SubagentStop: Array<{ matcher: string; hooks: Array<{ command: string }> }>;
      };
    };
    expect(parsed.hooks.SubagentStop).toHaveLength(2);
    const matchers = parsed.hooks.SubagentStop.map((e) => e.matcher);
    expect(matchers).toContain('dev-team:implementation-generator');
    expect(matchers).toContain('dev-team:test-gen-generator');
    for (const entry of parsed.hooks.SubagentStop) {
      expect(entry.hooks[0]?.command).toContain('static-check');
    }
  });

  it('cursor 产物不得出现 subagentStop: []', () => {
    const raw = buildHooksFile(canonical, getEnv('cursor'));
    expect(raw).not.toMatch(/"subagentStop"\s*:\s*\[\s*\]/);
    const parsed = JSON.parse(raw) as { hooks: Record<string, unknown> };
    expect(parsed.hooks.subagentStop).toBeUndefined();
  });

  it('claude loop_limit 语义保持：canonical 为 5 且 matcher 数仍为 2', () => {
    for (const entry of canonical.subagentStop) {
      expect(entry.loop_limit).toBe(5);
    }
    const parsed = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as {
      hooks: { SubagentStop: unknown[] };
    };
    expect(parsed.hooks.SubagentStop).toHaveLength(2);
  });
});
