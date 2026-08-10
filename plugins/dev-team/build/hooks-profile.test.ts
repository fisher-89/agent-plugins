/**
 * 单元测试: hooks-profile.ts — hooksProfile 成品 JSON 组装
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vite-plus/test';

import { getEnv } from './env';
import { buildHooksFile } from './hooks-profile';

const FIXTURE = {
  description: 'fixture hooks',
  preToolUse: [
    {
      matchers: { claude: 'Write|Edit', cursor: 'Write|StrReplace' },
      commandTemplate: 'node "__DEV_TEAM_RUNTIME_ROOT__/bin/__BIN:hooks__" protect-files',
    },
    {
      matchers: { claude: 'Bash', cursor: 'Shell' },
      commandTemplate: 'node "__DEV_TEAM_RUNTIME_ROOT__/bin/__BIN:hooks__" protect-files',
    },
  ],
  subagentStop: [
    {
      matchers: {
        claude: '__CALL_AGENT:implementation-generator__',
        cursor: '__AGENT:implementation-generator__',
      },
      loop_limit: 5,
      commandTemplate: 'node "__DEV_TEAM_RUNTIME_ROOT__/bin/__BIN:hooks__" static-check',
    },
  ],
};

describe('buildHooksFile', () => {
  it('claude 输出可 JSON.parse，含嵌套 hooks.PreToolUse / SubagentStop', () => {
    const env = getEnv('claude');
    const parsed = JSON.parse(buildHooksFile(FIXTURE, env)) as {
      hooks: { PreToolUse: unknown[]; SubagentStop: unknown[] };
    };
    expect(parsed.hooks.PreToolUse).toBeInstanceOf(Array);
    expect(parsed.hooks.SubagentStop).toBeInstanceOf(Array);
    expect(parsed.hooks.PreToolUse[0]).toMatchObject({
      matcher: 'Write|Edit',
      hooks: [{ type: 'command' }],
    });
  });

  it('claude matcher 覆盖 Claude 工具名族', () => {
    const env = getEnv('cursor');
    const parsed = JSON.parse(buildHooksFile(FIXTURE, env)) as {
      hooks: { preToolUse: Array<{ matcher: string }> };
    };
    const matchers = parsed.hooks.preToolUse.map((e) => e.matcher).join('|');
    expect(matchers).toContain('Write');
    expect(matchers).toContain('StrReplace');
    expect(matchers).toContain('Shell');
  });

  it('cursorNative 输出含 version 与 camelCase 事件，matcher 含 Shell 与 StrReplace', () => {
    const env = getEnv('cursorHome');
    const parsed = JSON.parse(buildHooksFile(FIXTURE, env)) as {
      version: number;
      hooks: {
        preToolUse: Array<{ matcher: string; command: string }>;
        subagentStop: unknown[];
      };
    };
    expect(parsed.version).toBe(1);
    expect(parsed.hooks.preToolUse).toBeDefined();
    expect(parsed.hooks.subagentStop).toBeDefined();
    const matchers = parsed.hooks.preToolUse.map((e) => e.matcher).join('|');
    expect(matchers).toContain('Shell');
    expect(matchers).toContain('StrReplace');
    expect(parsed.hooks.preToolUse[0]).toHaveProperty('command');
    expect(parsed.hooks.preToolUse[0]).not.toHaveProperty('hooks');
  });

  it('SubagentStop matcher 按平台展开 CALL_AGENT / AGENT token', () => {
    const claude = JSON.parse(buildHooksFile(FIXTURE, getEnv('claude'))) as {
      hooks: { SubagentStop: Array<{ matcher: string }> };
    };
    expect(claude.hooks.SubagentStop[0]?.matcher).toBe('dev-team:implementation-generator');

    const cursor = JSON.parse(buildHooksFile(FIXTURE, getEnv('cursor'))) as {
      hooks: { subagentStop: Array<{ matcher: string }> };
    };
    expect(cursor.hooks.subagentStop[0]?.matcher).toBe('implementation-generator');

    const home = JSON.parse(buildHooksFile(FIXTURE, getEnv('cursorHome'))) as {
      hooks: { subagentStop: Array<{ matcher: string }> };
    };
    expect(home.hooks.subagentStop[0]?.matcher).toBe('dev-team_implementation-generator');
  });

  it('command 模板中 __BIN:hooks__ 经 env 展开为对应 bin 文件名', () => {
    const plugin = JSON.parse(buildHooksFile(FIXTURE, getEnv('claude'))) as {
      hooks: { PreToolUse: Array<{ hooks: Array<{ command: string }> }> };
    };
    expect(plugin.hooks.PreToolUse[0]?.hooks[0]?.command).toContain('hooks.cjs');
    expect(plugin.hooks.PreToolUse[0]?.hooks[0]?.command).not.toContain('__BIN:');

    const home = JSON.parse(buildHooksFile(FIXTURE, getEnv('cursorHome'))) as {
      hooks: { preToolUse: Array<{ command: string }> };
    };
    expect(home.hooks.preToolUse[0]?.command).toContain('dev-team_hooks.cjs');
  });

  it('cursorHome 下路径 token 可仍保留在 command 中', () => {
    const home = JSON.parse(buildHooksFile(FIXTURE, getEnv('cursorHome'))) as {
      hooks: { preToolUse: Array<{ command: string }> };
    };
    expect(home.hooks.preToolUse[0]?.command).toContain('__DEV_TEAM_RUNTIME_ROOT__');
  });

  it('canonical 事件数组为空时产出合法最小 JSON 且不崩溃', () => {
    const empty = { preToolUse: [], subagentStop: [] };
    const nested = JSON.parse(buildHooksFile(empty, getEnv('claude'))) as {
      hooks: { PreToolUse: unknown[]; SubagentStop: unknown[] };
    };
    expect(nested.hooks.PreToolUse).toEqual([]);
    expect(nested.hooks.SubagentStop).toEqual([]);

    const flat = JSON.parse(buildHooksFile(empty, getEnv('cursorHome'))) as {
      version: number;
      hooks: { preToolUse: unknown[]; subagentStop: unknown[] };
    };
    expect(flat.version).toBe(1);
    expect(flat.hooks.preToolUse).toEqual([]);
  });

  it('preToolUse 为单元素 / 超大列表时均能序列化', () => {
    const single = {
      preToolUse: [FIXTURE.preToolUse[0]],
      subagentStop: [],
    };
    expect(() => JSON.parse(buildHooksFile(single, getEnv('claude')))).not.toThrow();

    const large = {
      preToolUse: Array.from({ length: 200 }, (_, i) => ({
        matchers: { claude: `Tool${i}`, cursor: `Tool${i}` },
        commandTemplate: 'echo __BIN:cli__',
      })),
      subagentStop: [],
    };
    const parsed = JSON.parse(buildHooksFile(large, getEnv('cursor'))) as {
      hooks: { preToolUse: unknown[] };
    };
    expect(parsed.hooks.preToolUse).toHaveLength(200);
  });

  it('同一 canonical 在不同 profile 下结构字段不同（nested vs flat）', () => {
    const nested = JSON.parse(buildHooksFile(FIXTURE, getEnv('claude'))) as Record<string, unknown>;
    const flat = JSON.parse(buildHooksFile(FIXTURE, getEnv('cursorHome'))) as Record<
      string,
      unknown
    >;
    expect(nested).toHaveProperty('hooks');
    expect(flat).toHaveProperty('version');
    const nestedHooks = nested.hooks as { PreToolUse?: unknown };
    const flatHooks = flat.hooks as { preToolUse?: unknown; PreToolUse?: unknown };
    expect(nestedHooks.PreToolUse).toBeDefined();
    expect(flatHooks.preToolUse).toBeDefined();
    expect(flatHooks.PreToolUse).toBeUndefined();
  });

  it('可选：读取真实 hooks.canonical.json 可成功组装', () => {
    const canonicalPath = join(
      dirname(fileURLToPath(import.meta.url)),
      '../hooks/hooks.canonical.json',
    );
    const canonical = JSON.parse(readFileSync(canonicalPath, 'utf-8'));
    const out = buildHooksFile(canonical, getEnv('cursorHome'));
    const parsed = JSON.parse(out);
    expect(parsed.hooks.preToolUse.length).toBeGreaterThan(0);
  });
});
