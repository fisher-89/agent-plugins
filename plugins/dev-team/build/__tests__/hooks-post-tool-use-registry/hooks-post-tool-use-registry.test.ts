/**
 * 集成测试: hooks.canonical.json → buildHooksFile → 产物 PostToolUse 注册
 *
 * 以真实 hooks.canonical.json 为配置源、走真实 applyEnvTokens 展开，验证 AC-2 /
 * AC-3 的组装链语义：
 * - claude 产物 hooks.PostToolUse 含唯一 record-files 条目，matcher 含工具集与
 *   phase_next 的 MCP 全名，`__BIN:hooks__` / `__DEV_TEAM_ROOT__` token 已展开；
 * - 既有 PreToolUse 三条目与 SubagentStop 两条目的包装形态不变（回归）；
 * - cursor / cursorHome 产物 postToolUse 注册 Cursor 工具集（Write|StrReplace|Shell
 *   加各产物 phase_next 的 MCP 全名），command 同样含 record-files 与已展开 token。
 *
 * @see openspec/changes/workflow-file-inventory/test-design.md — 集成测试「hooks.canonical.json → build 组装 → 产物 PostToolUse 注册」
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

interface CommandHook {
  type: string;
  command: string;
}

interface NestedHookEntry {
  matcher: string;
  hooks: CommandHook[];
}

interface ClaudeHooks {
  hooks: {
    PreToolUse: NestedHookEntry[];
    PostToolUse: NestedHookEntry[];
    SubagentStop: NestedHookEntry[];
  };
}

interface CursorHooks {
  hooks: {
    preToolUse?: Array<{ matcher: string; command: string }>;
    postToolUse?: Array<{ matcher: string; command: string }>;
    subagentStop?: unknown;
  };
}

describe('真实 canonical 组装 PostToolUse 注册', () => {
  const canonical = JSON.parse(readFileSync(canonicalPath, 'utf-8')) as object;

  it('claude：PostToolUse 唯一条目 matcher 为工具集加 phase_next MCP 全名，command 含 record-files 与已展开 token', () => {
    const parsed = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as ClaudeHooks;

    expect(parsed.hooks.PostToolUse).toHaveLength(1);
    const entry = parsed.hooks.PostToolUse[0];
    expect(entry.matcher).toBe(
      'Write|Edit|NotebookEdit|Bash|PowerShell|mcp__plugin_dev-team_dev-team__phase_next|mcp__plugin_dev-team_dev-team__phase_start',
    );
    expect(entry.hooks).toHaveLength(1);
    const command = entry.hooks[0].command;
    expect(command).toContain('record-files');
    // `__DEV_TEAM_ROOT__` 已展开为 claude 插件根 token
    expect(command).toContain('${CLAUDE_PLUGIN_ROOT}');
    // `__BIN:hooks__` 已展开为 hooks.cjs
    expect(command).toContain('hooks.cjs');
  });

  it('claude：record-files command 无残留 __ 风格 token', () => {
    const parsed = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as ClaudeHooks;
    const command = parsed.hooks.PostToolUse[0].hooks[0].command;

    expect(command).not.toMatch(/__[A-Z]/);
    expect(command).not.toContain('__MCP:');
    expect(command).not.toContain('__BIN:');
    expect(command).not.toContain('__DEV_TEAM_ROOT__');
  });

  it('claude：既有 PreToolUse 三条目与 SubagentStop 两条目的包装形态不变（回归）', () => {
    const parsed = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as ClaudeHooks;

    expect(parsed.hooks.PreToolUse).toHaveLength(3);
    expect(parsed.hooks.PreToolUse.map((e) => e.matcher)).toEqual([
      'Write|Edit',
      'Bash',
      'PowerShell',
    ]);
    for (const entry of parsed.hooks.PreToolUse) {
      expect(entry.hooks[0].command).toContain('protect-files');
    }

    expect(parsed.hooks.SubagentStop).toHaveLength(2);
    for (const entry of parsed.hooks.SubagentStop) {
      expect(entry.hooks).toEqual([
        { type: 'command', command: expect.stringContaining('static-check') },
      ]);
    }
  });

  it('cursor / cursorHome：postToolUse 唯一条目 matcher 为 Cursor 工具集加各产物 phase_next MCP 全名，command 含 record-files 与已展开 token', () => {
    const expected = {
      cursor: {
        matcher:
          'Write|StrReplace|Shell|mcp__plugin_dev-team_dev-team__phase_next|mcp__plugin_dev-team_dev-team__phase_start',
        bin: 'bin/hooks.cjs',
      },
      cursorHome: {
        matcher:
          'Write|StrReplace|Shell|mcp__user-dev-team_mcp__phase_next|mcp__user-dev-team_mcp__phase_start',
        bin: 'bin/dev-team_hooks.cjs',
      },
    } as const;
    for (const key of ['cursor', 'cursorHome'] as const) {
      const raw = buildHooksFile(canonical, getEnv(key));
      expect(raw).not.toContain('__MCP:');
      expect(raw).not.toContain('__BIN:');
      expect(raw).not.toContain('__DEV_TEAM_ROOT__');
      const parsed = JSON.parse(raw) as CursorHooks;
      expect(parsed.hooks.postToolUse).toHaveLength(1);
      const entry = parsed.hooks.postToolUse![0];
      expect(entry.matcher).toBe(expected[key].matcher);
      expect(entry.command).toContain('record-files');
      expect(entry.command).toContain(expected[key].bin);
    }
  });

  it('cursor：preToolUse 两条目 matchers 为 Write|StrReplace 与 Shell 且 command 含 protect-files；subagentStop 整段省略', () => {
    const parsed = JSON.parse(buildHooksFile(canonical, getEnv('cursor'))) as CursorHooks;

    expect(parsed.hooks.preToolUse).toHaveLength(2);
    expect(parsed.hooks.preToolUse!.map((e) => e.matcher)).toEqual(['Write|StrReplace', 'Shell']);
    for (const entry of parsed.hooks.preToolUse!) {
      expect(entry.command).toContain('protect-files');
    }
    // cursor 的 subagentStop matcher 均为 null，整段省略
    expect(parsed.hooks.subagentStop).toBeUndefined();
  });
});
