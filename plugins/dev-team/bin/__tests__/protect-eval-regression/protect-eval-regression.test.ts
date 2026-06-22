/**
 * 集成测试: protect-eval.mjs PreToolUse 回归
 *
 * @see openspec/changes/rewrite-hooks-to-node/test-design.md
 */

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

/** vitest CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');
const scriptPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/scripts/protect-eval.mjs');
const legacyShPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/scripts/protect-eval.sh');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface HookResult {
  permissionDecision: string;
  permissionDecisionReason?: string;
}

function runProtectEval(stdinJson: string): HookResult {
  const stdout = execFileSync(process.execPath, [scriptPath], {
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
// AC-4: Bash 写入检测与豁免
// ---------------------------------------------------------------------------

describe('protect-eval.mjs — Bash tee (AC-4)', () => {
  it("echo '[]' | tee openspec/changes/test/eval.json 应返回 deny", () => {
    const input = JSON.stringify({
      tool_name: 'Bash',
      tool_input: { command: "echo '[]' | tee openspec/changes/test/eval.json" },
    });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('deny');
  });
});

describe('protect-eval.mjs — Bash heredoc (AC-4)', () => {
  it('heredoc 写 eval.json 应返回 deny', () => {
    const command = "cat > openspec/changes/test-change/eval.json <<EOF\n[]\nEOF";
    const input = JSON.stringify({
      tool_name: 'Bash',
      tool_input: { command },
    });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('deny');
  });
});

describe('protect-eval.mjs — python 豁免 (AC-4)', () => {
  it('python 命令应返回 allow', () => {
    const input = JSON.stringify({
      tool_name: 'Bash',
      tool_input: {
        command: 'python plugins/dev-team/utils/eval-check.py --change test',
      },
    });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('allow');
  });
});

describe('protect-eval.mjs — node 豁免 (AC-4)', () => {
  it('node scripts/write-eval.mjs 应返回 allow', () => {
    const input = JSON.stringify({
      tool_name: 'Bash',
      tool_input: { command: 'node scripts/write-eval.mjs' },
    });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('allow');
  });
});

describe('protect-eval.mjs — 只读 Bash (AC-4)', () => {
  it('cat openspec/changes/test/eval.json 应返回 allow', () => {
    const input = JSON.stringify({
      tool_name: 'Bash',
      tool_input: { command: 'cat openspec/changes/test/eval.json' },
    });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('allow');
  });
});

// ---------------------------------------------------------------------------
// AC-9: fail-open 回归
// ---------------------------------------------------------------------------

describe('protect-eval.mjs — fail-open (AC-9)', () => {
  it('空 stdin 应返回 allow', () => {
    const result = runProtectEval('');
    expect(result.permissionDecision).toBe('allow');
  });

  it('无效 JSON stdin 应返回 allow', () => {
    const result = runProtectEval('{not json');
    expect(result.permissionDecision).toBe('allow');
  });

  it('Write 缺失 tool_input.file_path 应返回 allow', () => {
    const input = JSON.stringify({ tool_name: 'Write', tool_input: {} });
    const result = runProtectEval(input);
    expect(result.permissionDecision).toBe('allow');
  });
});

// ---------------------------------------------------------------------------
// AC-7: 脚本可用性
// ---------------------------------------------------------------------------

describe('protect-eval.mjs — 脚本可用性 (AC-7)', () => {
  it('protect-eval.mjs 应存在且非空', () => {
    expect(fs.existsSync(scriptPath)).toBe(true);
    expect(fs.statSync(scriptPath).size).toBeGreaterThan(0);
  });

  it('protect-eval.sh 不应再存在', () => {
    expect(fs.existsSync(legacyShPath)).toBe(false);
  });
});
