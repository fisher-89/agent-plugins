/**
 * 集成测试: hooks.json / plugin.json — Node .mjs hook 声明
 *
 * @see openspec/changes/rewrite-hooks-to-node/test-design.md
 */

import * as fs from 'node:fs';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

/** vitest CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');
const hooksJsonPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/hooks.json');
const pluginJsonPath = path.resolve(projectRoot, 'plugins/dev-team/.claude-plugin/plugin.json');

type HooksJson = {
  description?: string;
  hooks?: {
    PreToolUse?: Array<{
      matcher?: string;
      hooks?: Array<{ type?: string; command?: string }>;
    }>;
    SubagentStop?: Array<{
      matcher?: string;
      loop_limit?: number;
      hooks?: Array<{ type?: string; command?: string }>;
    }>;
  };
};

function readHooksJson(): HooksJson {
  return JSON.parse(fs.readFileSync(hooksJsonPath, 'utf-8')) as HooksJson;
}

function parseSemver(version: string): [number, number, number] {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!match) {
    throw new Error(`invalid semver: ${version}`);
  }
  return [Number(match[1]), Number(match[2]), Number(match[3])];
}

function isGreaterThan(a: string, b: string): boolean {
  const [aMaj, aMin, aPatch] = parseSemver(a);
  const [bMaj, bMin, bPatch] = parseSemver(b);
  if (aMaj !== bMaj) return aMaj > bMaj;
  if (aMin !== bMin) return aMin > bMin;
  return aPatch > bPatch;
}

// ---------------------------------------------------------------------------
// AC-1: hooks.json command 使用 node + .mjs
// ---------------------------------------------------------------------------

describe('hooks.json — PreToolUse Write|Edit (AC-1)', () => {
  it('command 应以 node 开头且引用 protect-eval.mjs，不含 bash 或 .sh', () => {
    const parsed = readHooksJson();
    const entry = parsed.hooks?.PreToolUse?.find((e) => e.matcher === 'Write|Edit');
    const command = entry?.hooks?.[0]?.command ?? '';
    expect(command).toMatch(/^node\s/);
    expect(command).toMatch(/protect-eval\.mjs/);
    expect(command).not.toMatch(/bash|\.sh/);
    expect(command).toContain('${CLAUDE_PLUGIN_ROOT}');
  });
});

describe('hooks.json — PreToolUse Bash (AC-1)', () => {
  it('command 应以 node 开头且引用 protect-eval.mjs', () => {
    const parsed = readHooksJson();
    const entry = parsed.hooks?.PreToolUse?.find((e) => e.matcher === 'Bash');
    const command = entry?.hooks?.[0]?.command ?? '';
    expect(command).toMatch(/^node\s/);
    expect(command).toMatch(/protect-eval\.mjs/);
    expect(command).not.toMatch(/bash|\.sh/);
  });
});

describe('hooks.json — SubagentStop (AC-1)', () => {
  it('command 应以 node 开头且引用 static-check.mjs，loop_limit 仍为 5', () => {
    const parsed = readHooksJson();
    const entry = parsed.hooks?.SubagentStop?.[0];
    expect(entry?.matcher).toBe('implementation-generator');
    expect(entry?.loop_limit).toBe(5);
    const command = entry?.hooks?.[0]?.command ?? '';
    expect(command).toMatch(/^node\s/);
    expect(command).toMatch(/static-check\.mjs/);
    expect(command).not.toMatch(/bash|\.sh/);
    expect(command).toContain('${CLAUDE_PLUGIN_ROOT}');
  });
});

describe('hooks.json — command 不含 .sh (AC-7)', () => {
  it('所有 hook command 不应引用 .sh 脚本', () => {
    const parsed = readHooksJson();
    const commands: string[] = [];
    for (const entry of parsed.hooks?.PreToolUse ?? []) {
      for (const hook of entry.hooks ?? []) {
        if (hook.command) commands.push(hook.command);
      }
    }
    for (const entry of parsed.hooks?.SubagentStop ?? []) {
      for (const hook of entry.hooks ?? []) {
        if (hook.command) commands.push(hook.command);
      }
    }
    for (const command of commands) {
      expect(command).not.toMatch(/\.sh/);
    }
  });
});

// ---------------------------------------------------------------------------
// AC-8: plugin.json 版本号
// ---------------------------------------------------------------------------

describe('plugin.json — 版本号 (AC-8)', () => {
  it('version 应大于 2.6.22 且符合 semver', () => {
    const parsed = JSON.parse(fs.readFileSync(pluginJsonPath, 'utf-8')) as { version?: string };
    expect(parsed.version).toMatch(/^\d+\.\d+\.\d+$/);
    expect(isGreaterThan(parsed.version!, '2.6.22')).toBe(true);
  });
});
