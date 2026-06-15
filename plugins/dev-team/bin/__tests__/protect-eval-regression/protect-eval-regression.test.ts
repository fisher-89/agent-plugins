/**
 * 集成测试: protect-eval.sh PreToolUse 回归
 *
 * 覆盖 AC-10：eval.json 写入拦截行为不受 static-check hook 变更影响。
 *
 * @see openspec/changes/static-check-agent-hook/test-design.md
 */

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

import { resolveBash } from '../helpers/resolve-bash';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

/** vitest CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');
const scriptPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/scripts/protect-eval.sh');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface HookResult {
  permissionDecision: string;
  permissionDecisionReason?: string;
}

function runProtectEval(stdinJson: string): HookResult {
  const stdout = execFileSync(resolveBash(), [scriptPath], {
    input: stdinJson,
    encoding: 'utf-8',
  });
  const parsed = JSON.parse(stdout.trim()) as {
    hookSpecificOutput?: {
      permissionDecision?: string;
      permissionDecisionReason?: string;
    };
  };
  return {
    permissionDecision: parsed.hookSpecificOutput?.permissionDecision ?? '',
    permissionDecisionReason: parsed.hookSpecificOutput?.permissionDecisionReason,
  };
}

// ---------------------------------------------------------------------------
// AC-10: Write / Edit / Bash 拦截
// ---------------------------------------------------------------------------

describe('protect-eval.sh — Write 拦截 (AC-10)', () => {
  it('Write openspec/changes/test/eval.json 应返回 permissionDecision deny', () => {
    const input = JSON.stringify({
      tool_name: 'Write',
      tool_input: { file_path: 'openspec/changes/test-change/eval.json' },
    });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('deny');
    expect(result.permissionDecisionReason).toMatch(/phase_log|eval\.json/i);
  });
});

describe('protect-eval.sh — Edit 拦截 (AC-10)', () => {
  it('Edit eval.json 路径仍应被拦截', () => {
    const input = JSON.stringify({
      tool_name: 'Edit',
      tool_input: { file_path: 'openspec/changes/my-change/eval.json' },
    });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('deny');
  });
});

describe('protect-eval.sh — Bash 重定向拦截 (AC-10)', () => {
  it("echo '[]' > openspec/changes/test/eval.json 仍应被拦截", () => {
    const input = JSON.stringify({
      tool_name: 'Bash',
      tool_input: { command: "echo '[]' > openspec/changes/test-change/eval.json" },
    });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('deny');
  });
});

describe('protect-eval.sh — 无关文件放行 (AC-10)', () => {
  it('Write 非 eval.json 文件应返回 allow', () => {
    const input = JSON.stringify({
      tool_name: 'Write',
      tool_input: { file_path: 'src/utils/helper.ts' },
    });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('allow');
  });
});

// ---------------------------------------------------------------------------
// 前置检查
// ---------------------------------------------------------------------------

describe('protect-eval.sh — 脚本可用性', () => {
  it('脚本文件应存在且非空', () => {
    expect(fs.existsSync(scriptPath)).toBe(true);
    expect(fs.statSync(scriptPath).size).toBeGreaterThan(0);
  });
});
