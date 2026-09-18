/**
 * 集成测试: hooks.canonical.json → 双平台包装产物 — userPromptSubmit 事件与
 * postToolUse matcher 扩展
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - AC-10: canonical 含 userPromptSubmit 条目（matchers.claude 为存在标志、
 *   matcher.cursor=null）→ claude 产出无 matcher 字段的 UserPromptSubmit 条目且
 *   command 经 token 展开指向 sweep-phase；cursor 产出不含 userPromptSubmit 键；
 *   canonical 删除该键 → schema default [] 兜底，两平台均不产出且不抛错；
 *   条目缺 commandTemplate → schema 校验失败（构建期报错）
 * - AC-9: 真实 canonical 的 postToolUse matchers 均含 `__MCP:phase_next__` 与
 *   `__MCP:phase_start__`（经 env 展开为各产物 MCP 全名），record-files command
 *   不变；既有 Write/Edit/Bash 等 matcher 逐字保留（回归）
 *
 * Mock 策略: 无——真实 fs 读取 hooks.canonical.json + getEnv 双平台 env 纯函数
 * 包装（既有 build/__tests__ 模式，参照 hooks-post-tool-use-registry）。
 */

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vite-plus/test';

import { getEnv } from '../../env';
import { buildHooksFile } from '../../hooks-profile';

const CANONICAL_PATH = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../hooks/hooks.canonical.json',
);

function readCanonical(): Record<string, unknown> {
  return JSON.parse(readFileSync(CANONICAL_PATH, 'utf-8')) as Record<string, unknown>;
}

interface ClaudeHooks {
  hooks: Record<string, unknown>;
}

interface CursorHooks {
  hooks: Record<string, unknown>;
}

// ===========================================================================
// 场景: userPromptSubmit 双平台产出差异 (AC-10)
// ===========================================================================

describe('userPromptSubmit 双平台产出差异 (AC-10)', () => {
  it('claude env → UserPromptSubmit 事件注册且条目无 matcher 字段、command 展开后指向 sweep-phase 子命令', () => {
    const canonical = readCanonical();
    // 前置：canonical 确实声明了 userPromptSubmit（claude 存在标志、cursor null）
    const userPromptSubmit = canonical.userPromptSubmit as Array<{
      matchers: { claude: string | null; cursor: string | null };
      commandTemplate: string;
    }>;
    expect(userPromptSubmit).toHaveLength(1);
    expect(userPromptSubmit[0].matchers.claude).not.toBeNull();
    expect(userPromptSubmit[0].matchers.cursor).toBeNull();
    expect(userPromptSubmit[0].commandTemplate).toContain('sweep-phase');

    const parsed = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as {
      hooks: {
        UserPromptSubmit?: Array<{
          matcher?: unknown;
          hooks: Array<{ type: string; command: string }>;
        }>;
      };
    };

    expect(parsed.hooks.UserPromptSubmit).toHaveLength(1);
    const entry = parsed.hooks.UserPromptSubmit![0];
    expect(entry).not.toHaveProperty('matcher');
    expect(entry.hooks).toEqual([
      { type: 'command', command: expect.stringContaining('sweep-phase') },
    ]);
    // token 已展开：无占位符残留
    expect(entry.hooks[0].command).not.toContain('__DEV_TEAM_ROOT__');
    expect(entry.hooks[0].command).not.toContain('__BIN:');
  });

  it('cursor env → 产物不含 userPromptSubmit 键（AC-10 降级）', () => {
    const cursor = JSON.parse(buildHooksFile(readCanonical(), getEnv('cursor'))) as CursorHooks;
    expect(Object.hasOwn(cursor.hooks, 'userPromptSubmit')).toBe(false);
    expect(Object.hasOwn(cursor.hooks, 'UserPromptSubmit')).toBe(false);
  });

  it('cursorHome env → 同样不含 userPromptSubmit 键', () => {
    const home = JSON.parse(buildHooksFile(readCanonical(), getEnv('cursorHome'))) as CursorHooks;
    expect(Object.hasOwn(home.hooks, 'userPromptSubmit')).toBe(false);
  });

  it('边界：canonical 删除 userPromptSubmit 键 → schema default [] 兜底，两平台均不产出且不抛错', () => {
    const canonical = readCanonical();
    const { userPromptSubmit: _omitted, ...withoutEvent } = canonical;

    const claude = JSON.parse(buildHooksFile(withoutEvent, getEnv('claude'))) as ClaudeHooks;
    expect(Object.hasOwn(claude.hooks, 'UserPromptSubmit')).toBe(false);

    const cursor = JSON.parse(buildHooksFile(withoutEvent, getEnv('cursor'))) as CursorHooks;
    expect(Object.hasOwn(cursor.hooks, 'userPromptSubmit')).toBe(false);
  });

  it('异常：userPromptSubmit 条目缺 commandTemplate → schema 校验失败（构建期报错）', () => {
    const canonical = readCanonical();
    const broken = {
      ...canonical,
      userPromptSubmit: [{ matchers: { claude: '*', cursor: null } }],
    };
    expect(() => buildHooksFile(broken, getEnv('claude'))).toThrow();
  });
});

// ===========================================================================
// 场景: postToolUse matcher 扩展贯通双平台 (AC-9)
// ===========================================================================

describe('postToolUse matcher 扩展贯通双平台 (AC-9)', () => {
  it('claude 与 cursor 的 postToolUse matcher 均含展开后的 phase_next 与 phase_start 全名，record-files command 不变', () => {
    const canonical = readCanonical();

    const claude = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as {
      hooks: {
        PostToolUse: Array<{ matcher: string; hooks: Array<{ command: string }> }>;
      };
    };
    expect(claude.hooks.PostToolUse).toHaveLength(1);
    expect(claude.hooks.PostToolUse[0].matcher).toContain(
      'mcp__plugin_dev-team_dev-team__phase_next',
    );
    expect(claude.hooks.PostToolUse[0].matcher).toContain(
      'mcp__plugin_dev-team_dev-team__phase_start',
    );
    expect(claude.hooks.PostToolUse[0].hooks[0].command).toContain('record-files');

    for (const agent of ['cursor', 'cursorHome'] as const) {
      const wrapped = JSON.parse(buildHooksFile(canonical, getEnv(agent))) as {
        hooks: { postToolUse: Array<{ matcher: string; command: string }> };
      };
      expect(wrapped.hooks.postToolUse).toHaveLength(1);
      // __MCP:<tool>__ token 经各产物 env 展开为 mcp__<prefix>__<tool> 全名
      expect(wrapped.hooks.postToolUse[0].matcher).toMatch(/mcp__[a-z0-9_-]*phase_next/);
      expect(wrapped.hooks.postToolUse[0].matcher).toMatch(/mcp__[a-z0-9_-]*phase_start/);
      expect(wrapped.hooks.postToolUse[0].command).toContain('record-files');
    }
  });

  it('边界：既有 Write / Edit / Bash / PowerShell / NotebookEdit 等 matcher 逐字保留（回归）', () => {
    const canonical = readCanonical();

    const claude = JSON.parse(buildHooksFile(canonical, getEnv('claude'))) as {
      hooks: { PostToolUse: Array<{ matcher: string }> };
    };
    for (const tool of ['Write', 'Edit', 'NotebookEdit', 'Bash', 'PowerShell']) {
      expect(claude.hooks.PostToolUse[0].matcher).toContain(tool);
    }

    const cursor = JSON.parse(buildHooksFile(canonical, getEnv('cursor'))) as {
      hooks: { postToolUse: Array<{ matcher: string }> };
    };
    for (const tool of ['Write', 'StrReplace', 'Shell']) {
      expect(cursor.hooks.postToolUse[0].matcher).toContain(tool);
    }
  });
});
