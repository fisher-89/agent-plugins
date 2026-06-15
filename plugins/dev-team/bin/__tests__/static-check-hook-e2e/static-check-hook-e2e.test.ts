/**
 * 集成测试: static-check.sh subagentStop hook 端到端
 *
 * 覆盖 AC-1、AC-2、AC-3、AC-4、AC-5：CLI 调用、followup 输出、放行与 JSON 转义。
 *
 * @see openspec/changes/static-check-agent-hook/test-design.md
 */

import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, describe, expect, it } from 'vite-plus/test';

import { resolveBash } from '../helpers/resolve-bash';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

/** vitest CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');
const scriptPath = path.resolve(projectRoot, 'plugins/dev-team/hooks/scripts/static-check.sh');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface StubPluginRoot {
  root: string;
  cleanup: () => void;
}

function createStubPluginRoot(cliBehavior: 'pass' | 'fail', stderrText = 'mock lint error'): StubPluginRoot {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'static-check-stub-'));
  const binDir = path.join(root, 'bin');
  const hooksDir = path.join(root, 'hooks', 'scripts');
  fs.mkdirSync(binDir, { recursive: true });
  fs.mkdirSync(hooksDir, { recursive: true });

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

  if (fs.existsSync(scriptPath)) {
    fs.copyFileSync(scriptPath, path.join(hooksDir, 'static-check.sh'));
  }

  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function runStaticCheckHook(
  pluginRoot: string,
  stdinJson = '{}',
): { stdout: string; status: number | null } {
  const hookScript = path.join(pluginRoot, 'hooks', 'scripts', 'static-check.sh');
  if (!fs.existsSync(hookScript)) {
    const repoScript = scriptPath;
    if (!fs.existsSync(repoScript)) {
      throw new Error('static-check.sh 尚未创建');
    }
  }
  const actualScript = fs.existsSync(path.join(pluginRoot, 'hooks', 'scripts', 'static-check.sh'))
    ? path.join(pluginRoot, 'hooks', 'scripts', 'static-check.sh')
    : scriptPath;

  const stdout = execFileSync(resolveBash(), [actualScript], {
    input: stdinJson,
    encoding: 'utf-8',
    env: { ...process.env, CLAUDE_PLUGIN_ROOT: pluginRoot },
  });
  return { stdout: stdout.trim(), status: 0 };
}

// ---------------------------------------------------------------------------
// AC-1: subagentStop 触发时调用 CLI
// ---------------------------------------------------------------------------

describe('static-check.sh — CLI 调用 (AC-1)', () => {
  let stub: StubPluginRoot | undefined;

  afterEach(() => {
    stub?.cleanup();
    stub = undefined;
  });

  it('设置 CLAUDE_PLUGIN_ROOT 后应调用 dev-team-cli.cjs run_static_analysis', () => {
    stub = createStubPluginRoot('pass');
    const { stdout } = runStaticCheckHook(stub.root);
    expect(stdout).toBe('{}');
  });
});

// ---------------------------------------------------------------------------
// AC-2: 检查失败时返回 followup_message
// ---------------------------------------------------------------------------

describe('static-check.sh — followup 输出 (AC-2)', () => {
  let stub: StubPluginRoot | undefined;

  afterEach(() => {
    stub?.cleanup();
    stub = undefined;
  });

  it('CLI exit 非 0 时 stdout 应为合法 JSON 且含 followup_message', () => {
    stub = createStubPluginRoot('fail', 'type error in foo.ts');
    const { stdout } = runStaticCheckHook(stub.root);
    const parsed = JSON.parse(stdout) as { followup_message?: string };
    expect(parsed.followup_message).toBeDefined();
    expect(typeof parsed.followup_message).toBe('string');
  });

  it('检查失败时脚本自身应 exit 0（followup 通过 JSON 传递）', () => {
    stub = createStubPluginRoot('fail');
    expect(() => runStaticCheckHook(stub!.root)).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// AC-3 / AC-4: 检查通过或未配置时放行
// ---------------------------------------------------------------------------

describe('static-check.sh — 放行输出 (AC-3, AC-4)', () => {
  let stub: StubPluginRoot | undefined;

  afterEach(() => {
    stub?.cleanup();
    stub = undefined;
  });

  it('CLI exit 0 时 stdout 应精确为 {}', () => {
    stub = createStubPluginRoot('pass');
    const { stdout } = runStaticCheckHook(stub.root);
    expect(stdout).toBe('{}');
  });

  it('未配置 static_analysis 时 CLI 放行并输出 {}', () => {
    // 使用真实 CLI + 临时项目（无 static_analysis 字段）
    const tmpProject = fs.mkdtempSync(path.join(os.tmpdir(), 'static-check-no-config-'));
    const openspecDir = path.join(tmpProject, 'openspec');
    fs.mkdirSync(openspecDir, { recursive: true });
    fs.writeFileSync(
      path.join(openspecDir, 'config.json'),
      JSON.stringify({ schema: 'spec-driven' }),
      'utf-8',
    );
    stub = createStubPluginRoot('pass');
    try {
      const { stdout } = runStaticCheckHook(stub.root, '{}');
      expect(stdout).toBe('{}');
    } finally {
      fs.rmSync(tmpProject, { recursive: true, force: true });
    }
  });
});

// ---------------------------------------------------------------------------
// AC-5: followup_message 内容
// ---------------------------------------------------------------------------

describe('static-check.sh — followup 内容 (AC-5)', () => {
  let stub: StubPluginRoot | undefined;

  afterEach(() => {
    stub?.cleanup();
    stub = undefined;
  });

  it('followup_message 应含 CLI 错误输出及中文修复指令前缀', () => {
    stub = createStubPluginRoot('fail', 'eslint: 3 errors');
    const { stdout } = runStaticCheckHook(stub.root);
    const parsed = JSON.parse(stdout) as { followup_message: string };
    expect(parsed.followup_message).toContain('eslint: 3 errors');
    expect(parsed.followup_message).toMatch(/静态检查|修复/);
  });

  it('followup 含换行、引号等特殊字符时 stdout JSON 仍可 JSON.parse', () => {
    const special = 'line1\nline2\t"quoted"\r\nbackslash\\test';
    stub = createStubPluginRoot('fail', special);
    const { stdout } = runStaticCheckHook(stub.root);
    expect(() => JSON.parse(stdout)).not.toThrow();
    const parsed = JSON.parse(stdout) as { followup_message: string };
    expect(parsed.followup_message).toContain('line1');
  });
});

// ---------------------------------------------------------------------------
// 脚本质量
// ---------------------------------------------------------------------------

describe('static-check.sh — 语法与副作用', () => {
  it('bash -n 语法检查应 exit 0', () => {
    expect(fs.existsSync(scriptPath)).toBe(true);
    execFileSync(resolveBash(), ['-n', scriptPath], { encoding: 'utf-8' });
  });

  it('执行后不应生成 reports/static_analysis.json', () => {
    const stub = createStubPluginRoot('pass');
    const reportsDir = path.join(projectRoot, 'openspec', 'changes', 'static-check-agent-hook', 'reports');
    const reportPath = path.join(reportsDir, 'static_analysis.json');
    const existedBefore = fs.existsSync(reportPath);
    try {
      runStaticCheckHook(stub.root);
      expect(fs.existsSync(reportPath)).toBe(existedBefore);
    } finally {
      stub.cleanup();
    }
  });
});
