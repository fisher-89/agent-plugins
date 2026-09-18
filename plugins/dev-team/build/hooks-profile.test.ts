/**
 * 单元测试: hooks-profile.ts — hooksProfile 成品 JSON 组装
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vite-plus/test';

import { getEnv, type ProductEnv } from './env';
import { buildHooksFile } from './hooks-profile';

const FIXTURE = {
  description: 'fixture hooks',
  preToolUse: [
    {
      matchers: { claude: 'Write|Edit', cursor: 'Write|StrReplace' },
      commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" protect-files',
    },
    {
      matchers: { claude: 'Bash', cursor: 'Shell' },
      commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" protect-files',
    },
  ],
  subagentStop: [
    {
      matchers: {
        claude: '__CALL_AGENT:implementation-generator__',
        cursor: '__AGENT:implementation-generator__',
      },
      loop_limit: 5,
      commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" static-check',
    },
  ],
};

const FIXTURE_CURSOR_NULL_SUBAGENT = {
  description: 'fixture with cursor-null subagentStop',
  preToolUse: [
    {
      matchers: { claude: 'Write|Edit', cursor: 'Write|StrReplace' },
      commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" protect-files',
    },
    {
      matchers: { claude: 'Bash', cursor: 'Shell' },
      commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" protect-files',
    },
  ],
  subagentStop: [
    {
      matchers: {
        claude: '__CALL_AGENT:implementation-generator__',
        cursor: null,
      },
      loop_limit: 5,
      commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" static-check',
    },
    {
      matchers: {
        claude: '__CALL_AGENT:test-gen-generator__',
        cursor: null,
      },
      loop_limit: 5,
      commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" static-check',
    },
  ],
};

describe('buildHooksFile', () => {
  it('claude PreToolUse / SubagentStop command 非空且展开 hooks 路径', () => {
    const parsed = JSON.parse(buildHooksFile(FIXTURE, getEnv('claude'))) as {
      hooks: {
        PreToolUse: Array<{ hooks: Array<{ command: string }> }>;
        SubagentStop: Array<{ hooks: Array<{ command: string }> }>;
      };
    };
    for (const entry of parsed.hooks.PreToolUse) {
      const command = entry.hooks[0]?.command ?? '';
      expect(command.length).toBeGreaterThan(0);
      expect(command).toContain('hooks.cjs');
      expect(command).toContain('protect-files');
    }
    for (const entry of parsed.hooks.SubagentStop) {
      const command = entry.hooks[0]?.command ?? '';
      expect(command.length).toBeGreaterThan(0);
      expect(command).toContain('hooks.cjs');
      expect(command).toContain('static-check');
    }
  });

  it('fixture 两条 subagentStop.matchers.cursor=null 时 cursor 产物 hooks 无 subagentStop 键', () => {
    const parsed = JSON.parse(buildHooksFile(FIXTURE_CURSOR_NULL_SUBAGENT, getEnv('cursor'))) as {
      hooks: Record<string, unknown>;
    };
    expect(Object.hasOwn(parsed.hooks, 'subagentStop')).toBe(false);
  });

  it('fixture cursor-null 时 cursorHome 产物亦省略 subagentStop 整键', () => {
    const parsed = JSON.parse(
      buildHooksFile(FIXTURE_CURSOR_NULL_SUBAGENT, getEnv('cursorHome')),
    ) as {
      hooks: Record<string, unknown>;
    };
    expect(Object.hasOwn(parsed.hooks, 'subagentStop')).toBe(false);
  });

  it('fixture 保留 claude matcher 时 claude 产物含 SubagentStop 且 command 含 static-check', () => {
    const parsed = JSON.parse(buildHooksFile(FIXTURE_CURSOR_NULL_SUBAGENT, getEnv('claude'))) as {
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

  it('读取真实 hooks.canonical.json：cursor/cursorHome 无 subagentStop；claude 仍有 SubagentStop', () => {
    const canonicalPath = join(
      dirname(fileURLToPath(import.meta.url)),
      '../hooks/hooks.canonical.json',
    );
    const canonical = JSON.parse(readFileSync(canonicalPath, 'utf-8')) as {
      subagentStop: Array<{ matchers: { cursor: string | null } }>;
    };
    for (const entry of canonical.subagentStop) {
      expect(entry.matchers.cursor).toBeNull();
    }

    const cursor = JSON.parse(buildHooksFile(canonical, getEnv('cursor'))) as {
      hooks: Record<string, unknown>;
    };
    expect(Object.hasOwn(cursor.hooks, 'subagentStop')).toBe(false);

    const home = JSON.parse(buildHooksFile(canonical, getEnv('cursorHome'))) as {
      hooks: Record<string, unknown>;
    };
    expect(Object.hasOwn(home.hooks, 'subagentStop')).toBe(false);

    const claude = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as {
      hooks: { SubagentStop: unknown[] };
    };
    expect(claude.hooks.SubagentStop.length).toBe(2);
  });

  it('preToolUse 仍按非 null cursor matcher 发射；仅 subagentStop 因全 null 被省略', () => {
    const parsed = JSON.parse(buildHooksFile(FIXTURE_CURSOR_NULL_SUBAGENT, getEnv('cursor'))) as {
      hooks: { preToolUse: unknown[]; subagentStop?: unknown[] };
    };
    expect(parsed.hooks.preToolUse.length).toBeGreaterThan(0);
    expect(parsed.hooks.subagentStop).toBeUndefined();
  });

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

  it('cursorNative 输出含 version 与 camelCase preToolUse 事件', () => {
    const env = getEnv('cursorHome');
    const parsed = JSON.parse(buildHooksFile(FIXTURE, env)) as {
      version: number;
      hooks: {
        preToolUse: Array<{ matcher: string; command: string }>;
      };
    };
    expect(parsed.version).toBe(1);
    expect(parsed.hooks.preToolUse).toBeDefined();
    const matchers = parsed.hooks.preToolUse.map((e) => e.matcher).join('|');
    expect(matchers).toContain('Shell');
    expect(matchers).toContain('StrReplace');
    expect(parsed.hooks.preToolUse[0]).toHaveProperty('command');
    expect(parsed.hooks.preToolUse[0]).not.toHaveProperty('hooks');
  });

  it('非 null cursor subagentStop matcher 按平台展开 AGENT token', () => {
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

  it('cursorHome 下路径 token 已替换', () => {
    const home = JSON.parse(buildHooksFile(FIXTURE, getEnv('cursorHome'))) as {
      hooks: { preToolUse: Array<{ command: string }> };
    };
    expect(home.hooks.preToolUse[0]?.command).not.toContain('__DEV_TEAM_ROOT__');
  });

  it('canonical 缺必填字段 / 非法类型时 zod parse 抛错', () => {
    expect(() => buildHooksFile({ preToolUse: 'bad' }, getEnv('claude'))).toThrow();
    expect(() => buildHooksFile({ subagentStop: 'bad' }, getEnv('cursor'))).toThrow();
  });

  it('canonical / env 为 null / undefined 时抛错', () => {
    expect(() => buildHooksFile(null as unknown as object, getEnv('claude'))).toThrow();
    expect(() => buildHooksFile(undefined as unknown as object, getEnv('claude'))).toThrow();
    expect(() => buildHooksFile(FIXTURE, null as unknown as ProductEnv)).toThrow();
    expect(() => buildHooksFile(FIXTURE, undefined as unknown as ProductEnv)).toThrow();
  });

  it('canonical={} 缺失 preToolUse / subagentStop 时 zod parse 抛错', () => {
    expect(() => buildHooksFile({}, getEnv('claude'))).toThrow();
  });

  it('env={} 缺失 agent 等必填字段时抛错', () => {
    expect(() => buildHooksFile(FIXTURE, {} as ProductEnv)).toThrow();
  });

  it('subagentStop=[] 时 Cursor 省略键；Claude 不写出 SubagentStop', () => {
    const empty = { preToolUse: FIXTURE.preToolUse, subagentStop: [] };
    const cursor = JSON.parse(buildHooksFile(empty, getEnv('cursor'))) as {
      hooks: Record<string, unknown>;
    };
    expect(Object.hasOwn(cursor.hooks, 'subagentStop')).toBe(false);

    const claude = JSON.parse(buildHooksFile(empty, getEnv('claude'))) as {
      hooks: Record<string, unknown>;
    };
    expect(Object.hasOwn(claude.hooks, 'SubagentStop')).toBe(false);
  });

  it('subagentStop 仅一条 cursor matcher 非 null 时 Cursor 仍写出该键且长度为 1', () => {
    const partial = {
      preToolUse: [],
      subagentStop: [
        {
          matchers: { claude: null, cursor: 'implementation-generator' },
          loop_limit: 5,
          commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" static-check',
        },
        {
          matchers: { claude: '__CALL_AGENT:test-gen-generator__', cursor: null },
          loop_limit: 5,
          commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" static-check',
        },
      ],
    };
    const parsed = JSON.parse(buildHooksFile(partial, getEnv('cursor'))) as {
      hooks: { subagentStop: unknown[] };
    };
    expect(parsed.hooks.subagentStop).toHaveLength(1);
  });

  it('preToolUse 全 null cursor matcher 时 Cursor 亦省略 preToolUse 键', () => {
    const onlyClaude = {
      preToolUse: [
        {
          matchers: { claude: 'Write', cursor: null },
          commandTemplate: 'echo ok',
        },
      ],
      subagentStop: [],
    };
    const parsed = JSON.parse(buildHooksFile(onlyClaude, getEnv('cursor'))) as {
      hooks: Record<string, unknown>;
    };
    expect(Object.hasOwn(parsed.hooks, 'preToolUse')).toBe(false);
  });

  it('loop_limit 为 0 / -1 / 省略时 Claude 路径序列化合法', () => {
    for (const loop_limit of [0, -1, undefined]) {
      const canonical = {
        preToolUse: [],
        subagentStop: [
          {
            matchers: { claude: 'dev-team:implementation-generator', cursor: null },
            ...(loop_limit !== undefined ? { loop_limit } : {}),
            commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" static-check',
          },
        ],
      };
      expect(() => JSON.parse(buildHooksFile(canonical, getEnv('claude')))).not.toThrow();
    }
  });

  it('canonical 含多余顶层字段时仍产出合法 hooks JSON', () => {
    const withExtra = { ...FIXTURE_CURSOR_NULL_SUBAGENT, extraField: 'ignored' };
    expect(() => JSON.parse(buildHooksFile(withExtra, getEnv('cursor')))).not.toThrow();
  });

  it('env 含多余字段时仍按 agent 分形组装', () => {
    const env = { ...getEnv('claude'), extra: true } as ProductEnv;
    const parsed = JSON.parse(buildHooksFile(FIXTURE_CURSOR_NULL_SUBAGENT, env)) as {
      hooks: { SubagentStop: unknown[] };
    };
    expect(parsed.hooks.SubagentStop).toHaveLength(2);
  });

  it('canonical 事件数组为空时产出合法最小 JSON 且不崩溃', () => {
    const empty = { preToolUse: [], subagentStop: [] };
    const nested = JSON.parse(buildHooksFile(empty, getEnv('claude'))) as {
      hooks: { PreToolUse: unknown[] };
    };
    expect(nested.hooks.PreToolUse).toEqual([]);

    const flat = JSON.parse(buildHooksFile(empty, getEnv('cursorHome'))) as {
      version: number;
      hooks: Record<string, unknown>;
    };
    expect(flat.version).toBe(1);
    expect(Object.keys(flat.hooks)).toHaveLength(0);
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
});

// ===========================================================================
// userPromptSubmit 事件 — canonical schema 与双平台包装（AC-10）
// ===========================================================================

const USER_PROMPT_FIXTURE = {
  description: 'fixture with userPromptSubmit',
  preToolUse: [],
  subagentStop: [],
  userPromptSubmit: [
    {
      matchers: { claude: '*', cursor: null },
      commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" sweep-phase',
    },
  ],
};

describe('userPromptSubmit — canonical schema 与双平台包装 (AC-10)', () => {
  it('claude 包装产出 UserPromptSubmit 数组：条目无 matcher 字段、hooks 为 command 且 command 指向 sweep-phase 模板', () => {
    const parsed = JSON.parse(buildHooksFile(USER_PROMPT_FIXTURE, getEnv('claude'))) as {
      hooks: {
        UserPromptSubmit?: Array<{
          matcher?: unknown;
          hooks: Array<{ type: string; command: string }>;
        }>;
      };
    };
    expect(parsed.hooks.UserPromptSubmit).toHaveLength(1);
    const entry = parsed.hooks.UserPromptSubmit![0];
    // claude 包装不携带 matcher 字段（canonical claude 值仅为存在标志）
    expect(entry).not.toHaveProperty('matcher');
    expect(entry.hooks[0]).toEqual({
      type: 'command',
      command: expect.stringContaining('sweep-phase'),
    });
    expect(entry.hooks[0].command).toContain('hooks.cjs');
    expect(entry.hooks[0].command).not.toContain('__DEV_TEAM_ROOT__');
  });

  it('cursor / cursorHome 包装不产出 userPromptSubmit 键（matcher.cursor=null 降级）', () => {
    for (const agent of ['cursor', 'cursorHome'] as const) {
      const parsed = JSON.parse(buildHooksFile(USER_PROMPT_FIXTURE, getEnv(agent))) as {
        hooks: Record<string, unknown>;
      };
      expect(Object.hasOwn(parsed.hooks, 'userPromptSubmit')).toBe(false);
      expect(Object.hasOwn(parsed.hooks, 'UserPromptSubmit')).toBe(false);
    }
  });

  it('canonical 缺省 userPromptSubmit → schema default [] 兜底，两平台包装均不产出该事件键且不抛错 (AC-10)', () => {
    const withoutEvent = {
      description: 'no userPromptSubmit',
      preToolUse: FIXTURE.preToolUse,
      subagentStop: FIXTURE.subagentStop,
    };
    const claude = JSON.parse(buildHooksFile(withoutEvent, getEnv('claude'))) as {
      hooks: Record<string, unknown>;
    };
    expect(Object.hasOwn(claude.hooks, 'UserPromptSubmit')).toBe(false);

    const cursor = JSON.parse(buildHooksFile(withoutEvent, getEnv('cursor'))) as {
      hooks: Record<string, unknown>;
    };
    expect(Object.hasOwn(cursor.hooks, 'userPromptSubmit')).toBe(false);
  });

  it('异常：userPromptSubmit 条目缺 commandTemplate → canonical schema 校验失败（构建期报错）', () => {
    expect(() =>
      buildHooksFile(
        {
          preToolUse: [],
          subagentStop: [],
          userPromptSubmit: [{ matchers: { claude: '*', cursor: null } }],
        },
        getEnv('claude'),
      ),
    ).toThrow();
  });

  it('异常：userPromptSubmit 键非数组 / 条目类型非法 → schema 校验拒绝（default [] 仅对键缺省生效）', () => {
    expect(() =>
      buildHooksFile(
        { preToolUse: [], subagentStop: [], userPromptSubmit: 'bad' },
        getEnv('claude'),
      ),
    ).toThrow();
    expect(() =>
      buildHooksFile(
        { preToolUse: [], subagentStop: [], userPromptSubmit: ['bad'] },
        getEnv('claude'),
      ),
    ).toThrow();
  });

  it('真实 hooks.canonical.json：claude 产出 UserPromptSubmit 指向 sweep-phase；cursor / cursorHome 不产出', () => {
    const canonicalPath = join(
      dirname(fileURLToPath(import.meta.url)),
      '../hooks/hooks.canonical.json',
    );
    const canonical = JSON.parse(readFileSync(canonicalPath, 'utf-8')) as Record<string, unknown>;

    const claude = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as {
      hooks: { UserPromptSubmit?: Array<{ hooks: Array<{ command: string }> }> };
    };
    expect(claude.hooks.UserPromptSubmit).toHaveLength(1);
    expect(claude.hooks.UserPromptSubmit![0].hooks[0].command).toContain('sweep-phase');

    for (const agent of ['cursor', 'cursorHome'] as const) {
      const wrapped = JSON.parse(buildHooksFile(canonical, getEnv(agent))) as {
        hooks: Record<string, unknown>;
      };
      expect(Object.hasOwn(wrapped.hooks, 'userPromptSubmit')).toBe(false);
    }
  });
});

// ===========================================================================
// postToolUse matcher 扩展 — __MCP:phase_start__ token（AC-9）
// ===========================================================================

describe('postToolUse matcher 扩展 — __MCP:phase_start__ (AC-9)', () => {
  it('canonical postToolUse matchers 含 __MCP:phase_start__ → claude 与 cursor 包装均展开该 token', () => {
    const withPhaseStart = {
      description: 'phase_start matcher',
      preToolUse: [],
      subagentStop: [],
      postToolUse: [
        {
          matchers: {
            claude: 'Write|Edit|__MCP:phase_next__|__MCP:phase_start__',
            cursor: 'Write|StrReplace|Shell|__MCP:phase_next__|__MCP:phase_start__',
          },
          commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" record-files',
        },
      ],
    };

    const claude = JSON.parse(buildHooksFile(withPhaseStart, getEnv('claude'))) as {
      hooks: { PostToolUse: Array<{ matcher: string }> };
    };
    // __MCP:<tool>__ token 经 env 展开为 claude 全名
    expect(claude.hooks.PostToolUse[0].matcher).toContain(
      'mcp__plugin_dev-team_dev-team__phase_start',
    );

    const cursor = JSON.parse(buildHooksFile(withPhaseStart, getEnv('cursor'))) as {
      hooks: { postToolUse: Array<{ matcher: string }> };
    };
    expect(cursor.hooks.postToolUse[0].matcher).toContain(
      'mcp__plugin_dev-team_dev-team__phase_start',
    );
  });

  it('真实 hooks.canonical.json：postToolUse matchers 均含 __MCP:phase_next__ 与 __MCP:phase_start__（record-files command 不变）', () => {
    const canonicalPath = join(
      dirname(fileURLToPath(import.meta.url)),
      '../hooks/hooks.canonical.json',
    );
    const canonical = JSON.parse(readFileSync(canonicalPath, 'utf-8')) as {
      postToolUse: Array<{
        matchers: { claude: string | null; cursor: string | null };
        commandTemplate: string;
      }>;
    };
    expect(canonical.postToolUse).toHaveLength(1);
    expect(canonical.postToolUse[0].matchers.claude).toContain('__MCP:phase_next__');
    expect(canonical.postToolUse[0].matchers.claude).toContain('__MCP:phase_start__');
    expect(canonical.postToolUse[0].matchers.cursor).toContain('__MCP:phase_start__');
    expect(canonical.postToolUse[0].commandTemplate).toContain('record-files');
  });
});
