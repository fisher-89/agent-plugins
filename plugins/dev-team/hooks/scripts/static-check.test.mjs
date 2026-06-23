/**
 * 单元测试: static-check.mjs — SubagentStop 静态检查 hook 逻辑
 *
 * @see openspec/changes/rewrite-hooks-to-node/test-design.md
 *
 * 用法: node --test static-check.test.mjs
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { fileURLToPath } from 'node:url';

const scriptPath = fileURLToPath(new URL('./static-check.mjs', import.meta.url));
const scriptExists = fs.existsSync(scriptPath);

const mod = scriptExists ? await import('./static-check.mjs') : null;

const {
  buildFollowupMessage,
  formatOutput,
  resolveCliPath,
  handleMissingCli,
  mergeCliOutput,
  parseWorkspaceRoot,
} = mod ?? {};

function skipIfMissing(name, fn) {
  return mod ? it(name, fn) : it.skip(name, fn);
}

function skipIfNoScript(name, fn) {
  return scriptExists ? it(name, fn) : it.skip(name, fn);
}

// ---------------------------------------------------------------------------
// buildFollowupMessage (AC-6)
// ---------------------------------------------------------------------------

describe('buildFollowupMessage', () => {
  skipIfMissing('应含中文修复前缀', () => {
    const msg = buildFollowupMessage('stderr output');
    assert.match(msg, /静态检查未通过，请修复以下错误后重新提交：/);
  });

  skipIfMissing('合并后的 CLI 输出应出现在 followup_message 中', () => {
    const merged = mergeCliOutput
      ? mergeCliOutput('stdout line', 'stderr line')
      : 'stdout line\nstderr line';
    const msg = buildFollowupMessage(merged);
    assert.match(msg, /stdout line/);
    assert.match(msg, /stderr line/);
  });
});

// ---------------------------------------------------------------------------
// formatOutput (AC-5, AC-6)
// ---------------------------------------------------------------------------

describe('formatOutput', () => {
  skipIfMissing('CLI exit 0 时应返回 {}', () => {
    const result = formatOutput({ status: 0, stdout: '', stderr: '' });
    assert.deepEqual(result, {});
  });

  skipIfMissing('CLI exit 非 0 时应返回含 followup_message 的对象', () => {
    const result = formatOutput({ status: 1, stdout: '', stderr: 'type error' });
    assert.ok(typeof result.followup_message === 'string');
    assert.match(result.followup_message, /type error/);
  });

  skipIfMissing('CLI 输出含特殊字符时 JSON.stringify 可解析', () => {
    const special = 'line1\nline2\t"quoted"\r\nbackslash\\test';
    const result = formatOutput({ status: 1, stdout: special, stderr: '' });
    const serialized = JSON.stringify(result);
    assert.doesNotThrow(() => JSON.parse(serialized));
    const reparsed = JSON.parse(serialized);
    assert.ok(reparsed.followup_message.includes('line1'));
  });

  skipIfMissing('应将 stdout 与 stderr 合并进 followup_message', () => {
    const result = formatOutput({
      status: 1,
      stdout: 'stdout line',
      stderr: 'stderr line',
    });
    assert.match(result.followup_message, /stdout line/);
    assert.match(result.followup_message, /stderr line/);
  });
});

// ---------------------------------------------------------------------------
// resolveCliPath (AC-5, AC-6)
// ---------------------------------------------------------------------------

describe('resolveCliPath', () => {
  skipIfMissing('正常 CLAUDE_PLUGIN_ROOT 下应拼接 bin/dev-team-cli.cjs', () => {
    const root = '/tmp/plugin-root';
    const cliPath = resolveCliPath(root);
    assert.equal(cliPath, path.join(root, 'bin', 'dev-team-cli.cjs'));
  });

  skipIfMissing('含空格的根目录 path.join 不应截断', () => {
    const root = '/tmp/my plugin root';
    const cliPath = resolveCliPath(root);
    assert.equal(cliPath, path.join(root, 'bin', 'dev-team-cli.cjs'));
    assert.ok(cliPath.includes('my plugin root'));
  });

  skipIfMissing('未传参时应读取 process.env.CLAUDE_PLUGIN_ROOT', () => {
    const prev = process.env.CLAUDE_PLUGIN_ROOT;
    try {
      process.env.CLAUDE_PLUGIN_ROOT = '/tmp/env plugin root';
      const cliPath =
        resolveCliPath.length === 0 ? resolveCliPath() : resolveCliPath(process.env.CLAUDE_PLUGIN_ROOT);
      assert.equal(cliPath, path.join('/tmp/env plugin root', 'bin', 'dev-team-cli.cjs'));
    } finally {
      if (prev === undefined) {
        delete process.env.CLAUDE_PLUGIN_ROOT;
      } else {
        process.env.CLAUDE_PLUGIN_ROOT = prev;
      }
    }
  });
});

// ---------------------------------------------------------------------------
// handleMissingCli (AC-10)
// ---------------------------------------------------------------------------

describe('handleMissingCli', () => {
  skipIfMissing('CLI 文件不存在时应返回 followup_message 且不抛异常', () => {
    const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'static-check-missing-'));
    try {
      const cliPath = path.join(tmpRoot, 'bin', 'dev-team-cli.cjs');
      const result = handleMissingCli(cliPath);
      assert.ok(typeof result.followup_message === 'string');
      assert.match(result.followup_message, /not found|CLI|dev-team/i);
    } finally {
      fs.rmSync(tmpRoot, { recursive: true, force: true });
    }
  });

  skipIfMissing('空 CLAUDE_PLUGIN_ROOT 时输出应含路径说明', () => {
    const prev = process.env.CLAUDE_PLUGIN_ROOT;
    try {
      delete process.env.CLAUDE_PLUGIN_ROOT;
      const result =
        handleMissingCli.length <= 1
          ? handleMissingCli('')
          : handleMissingCli(undefined);
      assert.ok(typeof result.followup_message === 'string');
      assert.ok(result.followup_message.length > 0);
    } finally {
      if (prev === undefined) {
        delete process.env.CLAUDE_PLUGIN_ROOT;
      } else {
        process.env.CLAUDE_PLUGIN_ROOT = prev;
      }
    }
  });
});

// ---------------------------------------------------------------------------
// parseWorkspaceRoot
// ---------------------------------------------------------------------------

describe('parseWorkspaceRoot', () => {
  skipIfMissing('应从合法 event JSON 提取 workspace_roots[0]', () => {
    const event = JSON.stringify({ workspace_roots: ['/d:/Projects/my-proj'] });
    const root = parseWorkspaceRoot(event);
    assert.ok(typeof root === 'string');
    assert.ok(root.length > 0);
    assert.ok(root.includes('my-proj'));
  });

  skipIfMissing('workspace_roots 为空数组时返回 null', () => {
    const event = JSON.stringify({ workspace_roots: [] });
    assert.equal(parseWorkspaceRoot(event), null);
  });

  skipIfMissing('无 workspace_roots 字段时返回 null', () => {
    const event = JSON.stringify({ subagent_type: 'implementation-generator' });
    assert.equal(parseWorkspaceRoot(event), null);
  });

  skipIfMissing('非法 JSON 时返回 null 且不抛异常', () => {
    assert.equal(parseWorkspaceRoot('not json'), null);
    assert.equal(parseWorkspaceRoot(''), null);
  });
});

// ---------------------------------------------------------------------------
// 黑盒: spawnSync + CLAUDE_PLUGIN_ROOT 临时目录 (Mock 策略)
// ---------------------------------------------------------------------------

describe('static-check.mjs — 黑盒 CLI 缺失 (AC-10)', () => {
  skipIfNoScript('无 dev-team-cli.cjs 时不应抛未捕获异常', () => {
    const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'static-check-e2e-'));
    try {
      const stdinJson = JSON.stringify({ workspace_roots: [tmpRoot] });
      const { status, stdout } = spawnSync(process.execPath, [scriptPath], {
        input: stdinJson,
        encoding: 'utf-8',
        env: { ...process.env, CLAUDE_PLUGIN_ROOT: tmpRoot },
      });
      assert.equal(status, 0);
      const parsed = JSON.parse(stdout.trim());
      assert.ok(typeof parsed.followup_message === 'string');
    } finally {
      fs.rmSync(tmpRoot, { recursive: true, force: true });
    }
  });
});
