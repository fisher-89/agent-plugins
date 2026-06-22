/**
 * 集成测试: static-check.mjs subagentStop hook 端到端
 *
 * @see openspec/changes/rewrite-hooks-to-node/test-design.md
 */

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

/** vitest CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');
const scriptPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/scripts/static-check.mjs');
const legacyShPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/scripts/static-check.sh');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface StubPluginRoot {
  root: string;
  cleanup: () => void;
}

function createStubPluginRoot(options: {
  cliBehavior?: 'pass' | 'fail' | 'missing';
  stderrText?: string;
}): StubPluginRoot {
  const { cliBehavior = 'pass', stderrText = 'mock lint error' } = options;
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'static-check-stub-'));
  const binDir = path.join(root, 'bin');
  const hooksDir = path.join(root, 'hooks', 'scripts');
  fs.mkdirSync(hooksDir, { recursive: true });

  if (cliBehavior !== 'missing') {
    fs.mkdirSync(binDir, { recursive: true });
    const exitCode = cliBehavior === 'pass' ? 0 : 1;
    const stubCli = `#!/usr/bin/env node
const args = process.argv.slice(2);
if (!args.includes('run_static_analysis')) {
  process.stderr.write('unexpected args: ' + args.join(' '));
  process.exit(2);
}
process.stderr.write(${JSON.stringify(stderrText)});
process.exit(${exitCode});
`;
    fs.writeFileSync(path.join(binDir, 'dev-team-cli.cjs'), stubCli, 'utf-8');
  }

  if (fs.existsSync(scriptPath)) {
    fs.copyFileSync(scriptPath, path.join(hooksDir, 'static-check.mjs'));
  }

  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function runStaticCheckHook(
  pluginRoot: string,
  stdinJson = '{}',
): { stdout: string } {
  const copiedScript = path.join(pluginRoot, 'hooks', 'scripts', 'static-check.mjs');
  const actualScript = fs.existsSync(copiedScript) ? copiedScript : scriptPath;

  const stdout = execFileSync(process.execPath, [actualScript], {
    input: stdinJson,
    encoding: 'utf-8',
    env: { ...process.env, CLAUDE_PLUGIN_ROOT: pluginRoot },
  });
  return { stdout: stdout.trim() };
}

// ---------------------------------------------------------------------------
// AC-10: CLI 缺失
// ---------------------------------------------------------------------------

describe('static-check.mjs — CLI 缺失 (AC-10)', () => {
  let stub: StubPluginRoot | undefined;

  afterEach(() => {
    stub?.cleanup();
    stub = undefined;
  });

  it('无 bin/dev-team-cli.cjs 时应输出 followup_message 且不抛异常', () => {
    stub = createStubPluginRoot({ cliBehavior: 'missing' });
    expect(() => runStaticCheckHook(stub!.root)).not.toThrow();
    const { stdout } = runStaticCheckHook(stub!.root);
    const parsed = JSON.parse(stdout) as { followup_message?: string };
    expect(parsed.followup_message).toBeDefined();
    expect(typeof parsed.followup_message).toBe('string');
  });
});

// ---------------------------------------------------------------------------
// AC-7: 脚本质量与可用性
// ---------------------------------------------------------------------------

describe('static-check.mjs — 脚本质量 (AC-7)', () => {
  let stub: StubPluginRoot | undefined;

  afterEach(() => {
    stub?.cleanup();
    stub = undefined;
  });

  it('node static-check.mjs 执行时不应抛出未捕获异常', () => {
    stub = createStubPluginRoot({ cliBehavior: 'pass' });
    expect(() => runStaticCheckHook(stub!.root)).not.toThrow();
  });

  it('执行后不应生成 reports/static_analysis.json', () => {
    stub = createStubPluginRoot({ cliBehavior: 'pass' });
    const reportsDir = path.join(
      projectRoot,
      'openspec',
      'changes',
      'rewrite-hooks-to-node',
      'reports',
    );
    const reportPath = path.join(reportsDir, 'static_analysis.json');
    const existedBefore = fs.existsSync(reportPath);
    try {
      runStaticCheckHook(stub!.root);
      expect(fs.existsSync(reportPath)).toBe(existedBefore);
    } finally {
      stub?.cleanup();
      stub = undefined;
    }
  });
});

describe('static-check.mjs — 脚本可用性 (AC-7)', () => {
  it('static-check.mjs 应存在且非空', () => {
    expect(fs.existsSync(scriptPath)).toBe(true);
    expect(fs.statSync(scriptPath).size).toBeGreaterThan(0);
  });

  it('static-check.sh 不应再存在', () => {
    expect(fs.existsSync(legacyShPath)).toBe(false);
  });
});
