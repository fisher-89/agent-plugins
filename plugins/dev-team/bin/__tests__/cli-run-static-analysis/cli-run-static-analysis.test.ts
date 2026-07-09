/**
 * 集成测试: dev-team-cli.cjs run_static_analysis 打包产物
 *
 * 覆盖 AC-7：help 输出、build 产物、端到端 exit code 传播、MCP 回归。
 *
 * @see openspec/changes/static-check-agent-hook/test-design.md
 */

import { execFileSync, execSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterEach, beforeAll, describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

const binDir = fileURLToPath(new URL('../../', import.meta.url));
const projectRoot = path.resolve(binDir, '../../../');
const cliPath = path.join(binDir, 'dev-team-cli.cjs');
const mcpPath = path.join(binDir, 'dev-team-mcp.cjs');

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(staticAnalysis?: string): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'cli-static-analysis-'));
  const openspecDir = path.join(root, 'openspec');
  fs.mkdirSync(openspecDir, { recursive: true });
  const config: Record<string, unknown> = { schema: 'spec-driven' };
  if (staticAnalysis !== undefined) {
    config.static_analysis = staticAnalysis;
  }
  fs.writeFileSync(path.join(openspecDir, 'config.json'), JSON.stringify(config, null, 2), 'utf-8');
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function isExecFileError(err: unknown): err is {
  stdout?: Buffer;
  stderr?: Buffer;
  status?: number;
} {
  return typeof err === 'object' && err !== null;
}

function runCli(
  args: string[],
  env: NodeJS.ProcessEnv = {},
): {
  stdout: string;
  stderr: string;
  status: number;
} {
  try {
    const stdout = execFileSync('node', [cliPath, ...args], {
      encoding: 'utf-8',
      cwd: projectRoot,
      env: { ...process.env, ...env },
      stdio: 'pipe',
    });
    return { stdout: stdout.trim(), stderr: '', status: 0 };
  } catch (err: unknown) {
    if (!isExecFileError(err)) {
      throw err;
    }
    return {
      stdout: err.stdout?.toString().trim() ?? '',
      stderr: err.stderr?.toString().trim() ?? '',
      status: err.status ?? 1,
    };
  }
}

let built = false;

beforeAll(() => {
  if (!fs.existsSync(cliPath)) {
    try {
      execSync('pnpm run build', { cwd: binDir, stdio: 'pipe' });
      built = true;
    } catch {
      built = false;
    }
  } else {
    built = true;
  }
});

// ---------------------------------------------------------------------------
// AC-7: help 与打包产物
// ---------------------------------------------------------------------------

describe('dev-team-cli.cjs — help 与打包 (AC-7)', () => {
  it('node dev-team-cli.cjs --help 输出应含 run_static_analysis 子命令', () => {
    expect(fs.existsSync(cliPath)).toBe(true);
    const { stdout, status } = runCli(['--help']);
    expect(status).toBe(0);
    expect(stdout).toContain('run_static_analysis');
  });

  it('pnpm run build 后 dev-team-cli.cjs 应存在且可执行', () => {
    expect(built).toBe(true);
    expect(fs.existsSync(cliPath)).toBe(true);
    const stats = fs.statSync(cliPath);
    expect(stats.size).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// AC-7: run_static_analysis 端到端
// ---------------------------------------------------------------------------

describe('dev-team-cli.cjs run_static_analysis — 端到端 (AC-7)', () => {
  let project: TempProject | undefined;

  afterEach(() => {
    project?.cleanup();
    project = undefined;
  });

  it('配置 node -e process.exit(0) 时 CLI 应 exit 0', () => {
    project = createTempProject('node -e process.exit(0)');
    const { status } = runCli(['run_static_analysis'], {
      CLAUDE_PROJECT_DIR: project.root,
    });
    expect(status).toBe(0);
  });

  it('配置 node -e process.exit(1) 时 CLI 应 exit 1 且 stderr 非空', () => {
    project = createTempProject('node -e "process.stderr.write(\\"fail\\"); process.exit(1)"');
    const { status, stderr } = runCli(['run_static_analysis'], {
      CLAUDE_PROJECT_DIR: project.root,
    });
    expect(status).toBe(1);
    expect(stderr.length).toBeGreaterThan(0);
  });
});

// ---------------------------------------------------------------------------
// AC-7: MCP 打包回归
// ---------------------------------------------------------------------------

describe('dev-team-mcp.cjs — 打包回归 (AC-7)', () => {
  it('build 后 dev-team-mcp.cjs 仍应正常生成', () => {
    expect(fs.existsSync(mcpPath)).toBe(true);
    const stats = fs.statSync(mcpPath);
    expect(stats.size).toBeGreaterThan(0);
  });
});
