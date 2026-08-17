/**
 * 单元测试: assemble.ts — 多平台产物组装
 */

import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vite-plus/test';

import * as envMod from './env';

const tempRoots: string[] = [];
const pluginRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
let outBase = '';
let cwdSpy: Mock | undefined;

function mockCwd(root: string): void {
  cwdSpy?.mockRestore();
  cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(root);
}

async function runAssembleAll(): Promise<void> {
  mockCwd(pluginRoot);
  await assembleAll();
}

function makeOutDir(key: envMod.ProductEnvKey): string {
  const dir = join(outBase, key);
  mkdirSync(dir, { recursive: true });
  return dir;
}

vi.mock('./env', async (importOriginal) => {
  const actual = await importOriginal<typeof envMod>();
  return {
    ...actual,
    getEnv: (key: envMod.ProductEnvKey) => ({
      ...actual.getEnv(key),
      outDir: makeOutDir(key),
    }),
  };
});

const { assembleAll } = await import('./assemble');

function writeMinimalCanonical(root: string): void {
  mkdirSync(join(root, 'hooks'), { recursive: true });
  writeFileSync(
    join(root, 'hooks/hooks.canonical.json'),
    JSON.stringify(
      {
        preToolUse: [
          {
            matchers: { claude: 'Write', cursor: 'Write' },
            commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" protect-files',
          },
        ],
        subagentStop: [
          {
            matchers: { claude: '__CALL_AGENT:implementation-generator__', cursor: null },
            loop_limit: 5,
            commandTemplate: 'node "__DEV_TEAM_ROOT__/bin/__BIN:hooks__" static-check',
          },
        ],
      },
      null,
      2,
    ),
  );
}

function writeMinimalStaging(root: string): void {
  mkdirSync(join(root, '.pack-staging/bin'), { recursive: true });
  writeFileSync(join(root, '.pack-staging/bin/cli.cjs'), '// stub\n');
  writeFileSync(join(root, '.pack-staging/install.mjs'), 'export {};\n');
}

function writeMinimalTree(root: string): void {
  writeMinimalCanonical(root);
  writeMinimalStaging(root);
  mkdirSync(join(root, 'templates'), { recursive: true });
  mkdirSync(join(root, 'utils'), { recursive: true });
  mkdirSync(join(root, 'agents'), { recursive: true });
  mkdirSync(join(root, '_fragments'), { recursive: true });
  writeFileSync(
    join(root, '_fragments/static-analysis-gate.cursor.md'),
    'run_static_analysis gate\n',
  );
  writeFileSync(join(root, '_fragments/static-analysis-gate.md'), '');
  writeFileSync(
    join(root, 'agents/implementation-generator.md'),
    '# gen\n\n__INCLUDE:static-analysis-gate__\n',
  );
}

/** 种子 .pack-staging/bin 中的 openspec 产物，模拟已退役的静态 bin 文件存在。 */
function seedRetiredStaticBins(root: string): void {
  mkdirSync(join(root, 'bin'), { recursive: true });
  for (const rel of ['bin/openspec', 'bin/openspec-bundled.js', 'bin/openspec.cmd']) {
    writeFileSync(join(root, rel), '// retired stub\n');
  }
}

const RETIRED_PATHS = ['bin/openspec', 'bin/openspec-bundled.js', 'bin/openspec.cmd'];

beforeEach(() => {
  outBase = mkdtempSync(join(tmpdir(), 'assemble-out-'));
  tempRoots.push(outBase);
  mockCwd(pluginRoot);
});

afterEach(() => {
  cwdSpy?.mockRestore();
  cwdSpy = undefined;
  vi.clearAllMocks();
  while (tempRoots.length > 0) {
    const dir = tempRoots.pop();
    if (dir) rmSync(dir, { recursive: true, force: true });
  }
});

describe('assembleAll', () => {
  const ASSEMBLE_TIMEOUT = 30_000;

  it(
    '三平台 outDir 均生成且无残留 __INCLUDE:',
    async () => {
      await runAssembleAll();
      for (const key of envMod.PRODUCT_ENV_KEYS) {
        const outDir = makeOutDir(key);
        const env = envMod.getEnv(key);
        const agentRel =
          key === 'cursorHome'
            ? 'agents/dev-team_implementation-generator.md'
            : 'agents/implementation-generator.md';
        expect(existsSync(outDir)).toBe(true);
        for (const rel of [agentRel, env.hooksFilePath]) {
          const full = join(outDir, rel);
          expect(existsSync(full)).toBe(true);
          const content = readFileSync(full, 'utf-8');
          expect(content).not.toContain('__INCLUDE:');
        }
      }
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'Cursor/cursorHome outDir 的 hooks JSON 无 subagentStop 键',
    async () => {
      await runAssembleAll();
      for (const key of ['cursor', 'cursorHome'] as const) {
        const hooksPath = join(makeOutDir(key), envMod.getEnv(key).hooksFilePath);
        const parsed = JSON.parse(readFileSync(hooksPath, 'utf-8')) as {
          hooks: Record<string, unknown>;
        };
        expect(Object.hasOwn(parsed.hooks, 'subagentStop')).toBe(false);
      }
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'Claude outDir hooks 含 SubagentStop 且 command 含 static-check',
    async () => {
      await runAssembleAll();
      const hooksPath = join(makeOutDir('claude'), envMod.getEnv('claude').hooksFilePath);
      const parsed = JSON.parse(readFileSync(hooksPath, 'utf-8')) as {
        hooks: { SubagentStop: Array<{ hooks: Array<{ command: string }> }> };
      };
      expect(parsed.hooks.SubagentStop.length).toBeGreaterThan(0);
      expect(parsed.hooks.SubagentStop[0]?.hooks[0]?.command).toContain('static-check');
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'generator 引用缺失 fragment 时 assemble 失败',
    async () => {
      const root = mkdtempSync(join(tmpdir(), 'assemble-fixture-'));
      tempRoots.push(root);
      writeMinimalTree(root);
      writeFileSync(join(root, 'agents/broken-agent.md'), '__INCLUDE:missing-fragment__\n');
      mockCwd(root);
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'fragment 环引用时 assemble 失败',
    async () => {
      const root = mkdtempSync(join(tmpdir(), 'assemble-cycle-'));
      tempRoots.push(root);
      writeMinimalTree(root);
      writeFileSync(join(root, '_fragments/loop-a.md'), '__INCLUDE:loop-b__');
      writeFileSync(join(root, '_fragments/loop-b.md'), '__INCLUDE:loop-a__');
      writeFileSync(join(root, 'agents/cycle-agent.md'), '__INCLUDE:loop-a__\n');
      mockCwd(root);
      await expect(assembleAll()).rejects.toThrow(/Include cycle|Include depth/);
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    '_fragments/ 存在于源码树但不出现在任一 env.outDir',
    async () => {
      await runAssembleAll();
      for (const key of envMod.PRODUCT_ENV_KEYS) {
        expect(existsSync(join(makeOutDir(key), '_fragments'))).toBe(false);
      }
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'Claude generator 无 run_static_analysis；Cursor 产物含该步骤',
    async () => {
      await runAssembleAll();
      const claudeAgent = readFileSync(
        join(makeOutDir('claude'), 'agents/implementation-generator.md'),
        'utf-8',
      );
      expect(claudeAgent).not.toContain('run_static_analysis');

      for (const key of ['cursor', 'cursorHome'] as const) {
        const prefix = key === 'cursorHome' ? 'dev-team_' : '';
        const agent = readFileSync(
          join(makeOutDir(key), `agents/${prefix}implementation-generator.md`),
          'utf-8',
        );
        expect(agent).toContain('run_static_analysis');
      }
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'assemble 复制 templates 静态资源到各 outDir',
    async () => {
      await runAssembleAll();
      for (const key of envMod.PRODUCT_ENV_KEYS) {
        const outDir = makeOutDir(key);
        expect(existsSync(join(outDir, 'templates'))).toBe(true);
      }
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'PRODUCT_ENV_KEYS 单轮全量执行失败即抛错',
    async () => {
      const root = mkdtempSync(join(tmpdir(), 'assemble-fail-fast-'));
      tempRoots.push(root);
      writeMinimalTree(root);
      rmSync(join(root, '_fragments/static-analysis-gate.md'));
      rmSync(join(root, '_fragments/static-analysis-gate.cursor.md'));
      mockCwd(root);
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'STATIC_BIN_FILES 不再包含退役的 openspec 静态 bin 文件',
    async () => {
      const root = mkdtempSync(join(tmpdir(), 'assemble-static-bins-'));
      tempRoots.push(root);
      writeMinimalTree(root);
      seedRetiredStaticBins(root);
      mockCwd(root);
      await assembleAll();
      for (const key of envMod.PRODUCT_ENV_KEYS) {
        for (const rel of RETIRED_PATHS) {
          expect(existsSync(join(makeOutDir(key), rel))).toBe(false);
        }
      }
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    '空 .pack-staging/bin 目录时 assembleAll 仍完成且各 outDir 生成',
    async () => {
      const root = mkdtempSync(join(tmpdir(), 'assemble-empty-staging-'));
      tempRoots.push(root);
      writeMinimalTree(root);
      // 清空 .pack-staging/bin（仅保留空的 bin 目录与 install.mjs）
      mkdirSync(join(root, '.pack-staging/bin'), { recursive: true });
      writeFileSync(join(root, '.pack-staging/install.mjs'), 'export {};\n');
      mockCwd(root);
      await expect(assembleAll()).resolves.toBeUndefined();
      for (const key of envMod.PRODUCT_ENV_KEYS) {
        expect(existsSync(makeOutDir(key))).toBe(true);
      }
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    '缺少 .pack-staging/bin 目录时 assembleAll 拒绝而非静默部分执行',
    async () => {
      const root = mkdtempSync(join(tmpdir(), 'assemble-no-staging-bin-'));
      tempRoots.push(root);
      writeMinimalCanonical(root);
      // 故意省略 .pack-staging/bin，只留 install.mjs
      mkdirSync(join(root, '.pack-staging'), { recursive: true });
      writeFileSync(join(root, '.pack-staging/install.mjs'), 'export {};\n');
      mkdirSync(join(root, 'templates'), { recursive: true });
      mkdirSync(join(root, 'utils'), { recursive: true });
      mkdirSync(join(root, 'agents'), { recursive: true });
      mockCwd(root);
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    '缺少 hooks/hooks.canonical.json 时 assembleAll 拒绝而非静默部分执行',
    async () => {
      const root = mkdtempSync(join(tmpdir(), 'assemble-no-hooks-'));
      tempRoots.push(root);
      writeMinimalStaging(root);
      // 故意省略 hooks/hooks.canonical.json
      mkdirSync(join(root, 'templates'), { recursive: true });
      mkdirSync(join(root, 'utils'), { recursive: true });
      mkdirSync(join(root, 'agents'), { recursive: true });
      mockCwd(root);
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'process.cwd 指向不存在路径时 assembleAll 抛出 ENOENT',
    async () => {
      const missing = join(tmpdir(), 'assemble-missing-cwd-' + Date.now());
      mockCwd(missing);
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    '无 skills/ 与 agents/ 目录时 assembleAll 仍完成',
    async () => {
      const root = mkdtempSync(join(tmpdir(), 'assemble-no-skills-agents-'));
      tempRoots.push(root);
      writeMinimalCanonical(root);
      writeMinimalStaging(root);
      mkdirSync(join(root, 'templates'), { recursive: true });
      mkdirSync(join(root, 'utils'), { recursive: true });
      // 故意省略 skills/ 与 agents/
      mockCwd(root);
      await expect(assembleAll()).resolves.toBeUndefined();
      for (const key of envMod.PRODUCT_ENV_KEYS) {
        const outDir = makeOutDir(key);
        expect(existsSync(outDir)).toBe(true);
        expect(existsSync(join(outDir, 'skills'))).toBe(false);
        expect(existsSync(join(outDir, 'agents'))).toBe(false);
      }
    },
    ASSEMBLE_TIMEOUT,
  );
});
