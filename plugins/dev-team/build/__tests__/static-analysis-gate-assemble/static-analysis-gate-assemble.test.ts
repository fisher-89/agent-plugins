/**
 * 集成测试: _fragments + generator agents → assemble outDir
 *
 * @see openspec/changes/cursor-omit-subagent-stop/test-design.md
 */

import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vite-plus/test';

import * as envMod from '../../env';

const tempRoots: string[] = [];
let outBase = '';
const pluginRoot = process.cwd();
let cwdSpy: Mock | undefined;

function mockCwd(root: string): void {
  cwdSpy?.mockRestore();
  cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(root);
}

function makeOutDir(key: envMod.ProductEnvKey): string {
  const dir = join(outBase, key);
  mkdirSync(dir, { recursive: true });
  return dir;
}

vi.mock('../../env', async (importOriginal) => {
  const actual = await importOriginal<typeof envMod>();
  return {
    ...actual,
    getEnv: (key: envMod.ProductEnvKey) => ({
      ...actual.getEnv(key),
      outDir: makeOutDir(key),
    }),
  };
});

const { assembleAll } = await import('../../assemble');

beforeEach(() => {
  outBase = mkdtempSync(join(tmpdir(), 'gate-assemble-out-'));
  tempRoots.push(outBase);
});

afterEach(() => {
  cwdSpy?.mockRestore();
  cwdSpy = undefined;
  while (tempRoots.length > 0) {
    const dir = tempRoots.pop();
    if (dir) rmSync(dir, { recursive: true, force: true });
  }
});

describe('三平台 generator 软门禁与 _fragments 排除', () => {
  it('cursor / cursorHome：implementation-generator 与 test-gen-generator 含 run_static_analysis', async () => {
    await assembleAll();
    for (const key of ['cursor', 'cursorHome'] as const) {
      const prefix = key === 'cursorHome' ? 'dev-team_' : '';
      for (const agent of ['implementation-generator', 'test-gen-generator']) {
        const path = join(makeOutDir(key), `agents/${prefix}${agent}.md`);
        expect(existsSync(path)).toBe(true);
        expect(readFileSync(path, 'utf-8')).toContain('run_static_analysis');
      }
    }
  });

  it('claude：两 generator 产物不含 Cursor 软门禁步骤', async () => {
    await assembleAll();
    for (const agent of ['implementation-generator', 'test-gen-generator']) {
      const content = readFileSync(join(makeOutDir('claude'), `agents/${agent}.md`), 'utf-8');
      expect(content).not.toContain('run_static_analysis');
      expect(content).not.toContain('You MUST NOT finish this agent until static analysis passes');
    }
  });

  it('任一 outDir 不存在 _fragments/ 目录', async () => {
    await assembleAll();
    for (const key of envMod.PRODUCT_ENV_KEYS) {
      expect(existsSync(join(makeOutDir(key), '_fragments'))).toBe(false);
    }
  });

  it('删除平台档与 default 后 assemble 失败', async () => {
    const root = mkdtempSync(join(tmpdir(), 'gate-assemble-missing-'));
    tempRoots.push(root);
    mkdirSync(join(root, 'hooks'), { recursive: true });
    writeFileSync(join(root, 'hooks/hooks.canonical.json'), '{"preToolUse":[],"subagentStop":[]}');
    mkdirSync(join(root, '.pack-staging/bin'), { recursive: true });
    writeFileSync(join(root, '.pack-staging/bin/cli.cjs'), '// stub');
    writeFileSync(join(root, '.pack-staging/install.mjs'), 'export {};');
    mkdirSync(join(root, 'templates'), { recursive: true });
    mkdirSync(join(root, 'utils'), { recursive: true });
    mkdirSync(join(root, 'agents'), { recursive: true });
    mkdirSync(join(root, '_fragments'), { recursive: true });
    writeFileSync(
      join(root, 'agents/implementation-generator.md'),
      '__INCLUDE:static-analysis-gate__\n',
    );
    mockCwd(root);
    await expect(assembleAll()).rejects.toThrow(/Missing fragment/);
  });

  it('cursorHome 产物中 __BIN:cli__ / __DEV_TEAM_ROOT__ 已按 home env 展开', async () => {
    await assembleAll();
    const agent = readFileSync(
      join(makeOutDir('cursorHome'), 'agents/dev-team_implementation-generator.md'),
      'utf-8',
    );
    expect(agent).toContain('dev-team_cli.cjs');
    expect(agent).toContain('__INSTALL_PLUGIN_ROOT__');
    expect(agent).not.toContain('__BIN:cli__');
    expect(agent).not.toContain('__DEV_TEAM_ROOT__');
  });

  it('源码 _fragments 仍存在于包根，仅 outDir 排除', async () => {
    expect(existsSync(join(pluginRoot, '_fragments/static-analysis-gate.cursor.md'))).toBe(true);
    await assembleAll();
    for (const key of envMod.PRODUCT_ENV_KEYS) {
      expect(existsSync(join(makeOutDir(key), '_fragments'))).toBe(false);
    }
  });
});
