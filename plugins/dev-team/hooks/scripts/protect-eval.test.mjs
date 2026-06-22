/**
 * 单元测试: protect-eval.mjs — PreToolUse eval.json 保护逻辑
 *
 * @see openspec/changes/rewrite-hooks-to-node/test-design.md
 *
 * 用法: node --test protect-eval.test.mjs
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(new URL('./protect-eval.mjs', import.meta.url));
const scriptExists = fs.existsSync(scriptPath);

// TODO: protect-eval.mjs 实现后需导出以下函数
const mod = scriptExists ? await import('./protect-eval.mjs') : null;

const {
  isEvalJsonPath,
  detectBashWrite,
  extractChangeName,
  buildDenyReason,
  parseInput,
  outputDeny,
} = mod ?? {};

function skipIfMissing(name, fn) {
  return mod ? it(name, fn) : it.skip(name, fn);
}

function skipIfNoScript(name, fn) {
  return scriptExists ? it(name, fn) : it.skip(name, fn);
}

function parseHookStdout(stdout) {
  const parsed = JSON.parse(stdout.trim());
  return parsed.hookSpecificOutput?.permissionDecision ?? '';
}

// ---------------------------------------------------------------------------
// isEvalJsonPath — 路径匹配 (AC-3)
// ---------------------------------------------------------------------------

describe('isEvalJsonPath — 路径匹配', () => {
  skipIfMissing('相对路径 openspec/changes/test-change/eval.json 应返回 true', () => {
    assert.equal(isEvalJsonPath('openspec/changes/test-change/eval.json'), true);
  });

  skipIfMissing('绝对路径 D:/Projects/.../eval.json 应返回 true', () => {
    assert.equal(
      isEvalJsonPath('D:/Projects/wps-claude-plugin/openspec/changes/demo/eval.json'),
      true,
    );
  });

  skipIfMissing('非 eval 文件 openspec/changes/test/design.md 应返回 false', () => {
    assert.equal(isEvalJsonPath('openspec/changes/test/design.md'), false);
  });

  skipIfMissing('changes 目录外路径应返回 false', () => {
    assert.equal(isEvalJsonPath('plugins/dev-team/bin/src/commands/phase-log.ts'), false);
  });

  skipIfMissing('反斜杠归一化 openspec\\changes\\test\\eval.json 应返回 true', () => {
    assert.equal(isEvalJsonPath('openspec\\changes\\test\\eval.json'), true);
  });

  skipIfMissing('空字符串应返回 false', () => {
    assert.equal(isEvalJsonPath(''), false);
  });

  skipIfMissing('超长路径仍含 eval.json 后缀时应返回 true', () => {
    const prefix = 'a'.repeat(1000);
    const path = `${prefix}/openspec/changes/test-change/eval.json`;
    assert.equal(isEvalJsonPath(path), true);
  });
});

// ---------------------------------------------------------------------------
// detectBashWrite — Bash 写入检测与豁免 (AC-4)
// ---------------------------------------------------------------------------

describe('detectBashWrite — 写入模式', () => {
  skipIfMissing('> 重定向到 eval.json 应返回 true', () => {
    assert.equal(
      detectBashWrite("echo '[]' > openspec/changes/test/eval.json"),
      true,
    );
  });

  skipIfMissing('>> 追加到 eval.json 应返回 true', () => {
    assert.equal(
      detectBashWrite("echo '[]' >> openspec/changes/test/eval.json"),
      true,
    );
  });

  skipIfMissing('tee 写入 eval.json 应返回 true', () => {
    assert.equal(
      detectBashWrite("echo '[]' | tee openspec/changes/test/eval.json"),
      true,
    );
  });

  skipIfMissing('tee -a 追加 eval.json 应返回 true', () => {
    assert.equal(
      detectBashWrite("echo '[]' | tee -a openspec/changes/test/eval.json"),
      true,
    );
  });

  skipIfMissing('heredoc 写入 eval.json 应返回 true', () => {
    const cmd = "cat > openspec/changes/test/eval.json <<EOF\n[]\nEOF";
    assert.equal(detectBashWrite(cmd), true);
  });

  skipIfMissing('>& 重定向到 eval.json 应返回 true', () => {
    assert.equal(
      detectBashWrite("echo '[]' >& openspec/changes/test/eval.json"),
      true,
    );
  });

  skipIfMissing('含 -> 但无 eval.json 写入时应返回 false', () => {
    assert.equal(detectBashWrite('echo foo -> bar'), false);
  });
});

describe('detectBashWrite — 豁免与只读', () => {
  skipIfMissing('python 命令应豁免并返回 false', () => {
    assert.equal(
      detectBashWrite('python plugins/dev-team/utils/eval-check.py --change test'),
      false,
    );
  });

  skipIfMissing('python3 命令应豁免并返回 false', () => {
    assert.equal(detectBashWrite('python3 -c "print(1)"'), false);
  });

  skipIfMissing('node 命令应豁免并返回 false', () => {
    assert.equal(detectBashWrite('node scripts/write-eval.mjs'), false);
  });

  skipIfMissing('只读 cat eval.json 应返回 false', () => {
    assert.equal(detectBashWrite('cat openspec/changes/test/eval.json'), false);
  });

  skipIfMissing('无 eval.json 的命令 ls -la 应返回 false', () => {
    assert.equal(detectBashWrite('ls -la'), false);
  });

  skipIfMissing('空命令应返回 false', () => {
    assert.equal(detectBashWrite(''), false);
  });
});

// ---------------------------------------------------------------------------
// extractChangeName / buildDenyReason (AC-3)
// ---------------------------------------------------------------------------

describe('extractChangeName', () => {
  skipIfMissing('应从 eval.json 路径提取变更名 my-feature', () => {
    assert.equal(
      extractChangeName('openspec/changes/my-feature/eval.json'),
      'my-feature',
    );
  });

  skipIfMissing('无匹配路径应返回空字符串', () => {
    assert.equal(extractChangeName('src/utils/helper.ts'), '');
  });
});

describe('buildDenyReason', () => {
  skipIfMissing('拒绝原因应含 phase_log、eval.json 及变更名', () => {
    const reason = buildDenyReason('my-change', 'Write');
    assert.match(reason, /phase_log/);
    assert.match(reason, /eval\.json/);
    assert.match(reason, /my-change/);
  });
});

// ---------------------------------------------------------------------------
// parseInput — fail-open (AC-9)
// ---------------------------------------------------------------------------

describe('parseInput — fail-open', () => {
  skipIfMissing('空 stdin 应触发 allow', () => {
    const result = parseInput('');
    assert.equal(result.decision, 'allow');
  });

  skipIfMissing('无效 JSON 应触发 allow', () => {
    const result = parseInput('{not json');
    assert.equal(result.decision, 'allow');
  });

  skipIfMissing('缺失 tool_name 应触发 allow', () => {
    const result = parseInput('{}');
    assert.equal(result.decision, 'allow');
  });

  skipIfMissing('Write 缺失 tool_input.file_path 应触发 allow', () => {
    const result = parseInput(JSON.stringify({ tool_name: 'Write', tool_input: {} }));
    assert.equal(result.decision, 'allow');
  });

  skipIfMissing('未知工具 Read 应触发 allow', () => {
    const result = parseInput(
      JSON.stringify({ tool_name: 'Read', tool_input: { file_path: 'foo.txt' } }),
    );
    assert.equal(result.decision, 'allow');
  });
});

// ---------------------------------------------------------------------------
// outputDeny — JSON 输出格式
// ---------------------------------------------------------------------------

describe('outputDeny — JSON 特殊字符', () => {
  skipIfMissing('reason 含换行、引号、反斜杠时 stdout 可 JSON.parse', () => {
    const reason = 'line1\nline2\t"quoted"\r\nbackslash\\test';
    const stdout = outputDeny(reason);
    assert.doesNotThrow(() => JSON.parse(stdout));
    const parsed = JSON.parse(stdout);
    assert.equal(parsed.hookSpecificOutput.permissionDecision, 'deny');
    assert.ok(parsed.hookSpecificOutput.permissionDecisionReason.includes('line1'));
  });
});

// ---------------------------------------------------------------------------
// 黑盒: spawnSync 注入 stdin（Mock 策略 — 未导出函数时的 fail-open 回归）
// ---------------------------------------------------------------------------

describe('protect-eval.mjs — 黑盒 fail-open (AC-9)', () => {
  skipIfNoScript('空 stdin 应输出 allow', () => {
    const { status, stdout } = spawnSync(process.execPath, [scriptPath], {
      input: '',
      encoding: 'utf-8',
    });
    assert.equal(status, 0);
    assert.equal(parseHookStdout(stdout), 'allow');
  });

  skipIfNoScript('无效 JSON stdin 应输出 allow', () => {
    const { status, stdout } = spawnSync(process.execPath, [scriptPath], {
      input: '{not json',
      encoding: 'utf-8',
    });
    assert.equal(status, 0);
    assert.equal(parseHookStdout(stdout), 'allow');
  });

  skipIfNoScript('未知工具 Read 应输出 allow', () => {
    const input = JSON.stringify({
      tool_name: 'Read',
      tool_input: { file_path: 'foo.txt' },
    });
    const { status, stdout } = spawnSync(process.execPath, [scriptPath], {
      input,
      encoding: 'utf-8',
    });
    assert.equal(status, 0);
    assert.equal(parseHookStdout(stdout), 'allow');
  });
});
