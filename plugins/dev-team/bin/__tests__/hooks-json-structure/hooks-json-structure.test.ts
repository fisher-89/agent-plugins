/**
 * 集成测试: hooks.json SubagentStop 声明与 plugin.json 版本
 *
 * 覆盖 AC-11、AC-12：matcher、loop_limit、PreToolUse 不变、版本递增。
 *
 * @see openspec/changes/static-check-agent-hook/test-design.md
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
// AC-11: SubagentStop 声明
// ---------------------------------------------------------------------------

describe('hooks.json — SubagentStop 声明 (AC-11)', () => {
  it('JSON 解析成功且 hooks.SubagentStop 数组至少一项', () => {
    const parsed = readHooksJson();
    expect(parsed.hooks?.SubagentStop?.length).toBeGreaterThan(0);
  });

  it('SubagentStop 项 matcher 应为 implementation-generator', () => {
    const parsed = readHooksJson();
    const entry = parsed.hooks?.SubagentStop?.[0];
    expect(entry?.matcher).toBe('implementation-generator');
  });

  it('SubagentStop 项 loop_limit 应为 5', () => {
    const parsed = readHooksJson();
    const entry = parsed.hooks?.SubagentStop?.[0];
    expect(entry?.loop_limit).toBe(5);
  });

  it('hook command 应指向 static-check.sh', () => {
    const parsed = readHooksJson();
    const command = parsed.hooks?.SubagentStop?.[0]?.hooks?.[0]?.command ?? '';
    expect(command).toMatch(/static-check\.sh/);
    expect(command).toContain('${CLAUDE_PLUGIN_ROOT}');
  });
});

// ---------------------------------------------------------------------------
// AC-10 / AC-11: PreToolUse 不变
// ---------------------------------------------------------------------------

describe('hooks.json — PreToolUse 不变 (AC-10)', () => {
  it('PreToolUse 应仍有两条 hook（Write|Edit、Bash → protect-eval.sh）', () => {
    const parsed = readHooksJson();
    const preToolUse = parsed.hooks?.PreToolUse ?? [];
    expect(preToolUse).toHaveLength(2);
    expect(preToolUse[0]?.matcher).toBe('Write|Edit');
    expect(preToolUse[1]?.matcher).toBe('Bash');
    for (const entry of preToolUse) {
      expect(entry.hooks?.[0]?.type).toBe('command');
      expect(entry.hooks?.[0]?.command).toMatch(/protect-eval\.sh/);
    }
  });
});

// ---------------------------------------------------------------------------
// AC-12: plugin.json 版本号
// ---------------------------------------------------------------------------

describe('plugin.json — 版本号 (AC-12)', () => {
  it('version 应大于 2.6.9 且符合 semver', () => {
    const parsed = JSON.parse(fs.readFileSync(pluginJsonPath, 'utf-8')) as { version?: string };
    expect(parsed.version).toMatch(/^\d+\.\d+\.\d+$/);
    expect(isGreaterThan(parsed.version!, '2.6.9')).toBe(true);
  });
});
