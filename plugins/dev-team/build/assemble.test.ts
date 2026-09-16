/**
 * 单元测试: assemble.ts — 多平台产物组装
 *
 * 每个用例都在临时目录中自行生成 assemble 所需的源码树（含 .pack-staging
 * 构建产物），不读取仓库中的任何文件，因此不依赖先执行 pnpm build。
 */

import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vite-plus/test';

import * as envMod from './env';

const tempRoots: string[] = [];
let outBase = '';
let cwdSpy: Mock | undefined;

function mockCwd(root: string): void {
  cwdSpy?.mockRestore();
  cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(root);
}

function makeTempRoot(prefix: string): string {
  const root = mkdtempSync(join(tmpdir(), prefix));
  tempRoots.push(root);
  return root;
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

function writeFixtureFile(root: string, rel: string, content: string): void {
  const full = join(root, rel);
  mkdirSync(dirname(full), { recursive: true });
  writeFileSync(full, content, 'utf-8');
}

/** canonical hooks：一条两端通用的 preToolUse，一条仅 claude 生效的 subagentStop。 */
const CANONICAL_HOOKS = {
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
};

/** 在 root 下生成 assemble 会从 cwd 读取的全部源码（含 .pack-staging 构建产物）。 */
function seedSourceTree(root: string): void {
  writeFixtureFile(root, 'hooks/hooks.canonical.json', JSON.stringify(CANONICAL_HOOKS, null, 2));
  writeFixtureFile(root, '.pack-staging/bin/cli.cjs', '// stub\n');
  writeFixtureFile(root, '.pack-staging/bin/hooks.cjs', '// stub\n');
  writeFixtureFile(root, '.pack-staging/bin/mcp.cjs', '// stub\n');
  writeFixtureFile(root, '.pack-staging/install.mjs', 'export {};\n');
  writeFixtureFile(root, 'templates/adr.md', '# ADR\n');
  writeFixtureFile(
    root,
    'skills/phase-proposal/SKILL.md',
    '# proposal\n\n__SKILL:phase-proposal__\n',
  );
  writeFixtureFile(
    root,
    'agents/implementation-generator.md',
    '# gen\n\n__INCLUDE:static-analysis-gate__\n',
  );
  writeFixtureFile(root, '_fragments/static-analysis-gate.md', '');
  writeFixtureFile(root, '_fragments/static-analysis-gate.cursor.md', 'run_static_analysis gate\n');
}

/** 在源码树中写入已退役的 openspec 静态 bin 文件，模拟它们仍留在仓库里。 */
const RETIRED_PATHS = ['bin/openspec', 'bin/openspec-bundled.js', 'bin/openspec.cmd'];

function seedRetiredStaticBins(root: string): void {
  for (const rel of RETIRED_PATHS) {
    writeFixtureFile(root, rel, '// retired stub\n');
  }
}

/** 生成完整源码树并切换 cwd，返回临时根目录供用例继续改造。 */
function makeSourceRoot(prefix: string): string {
  const root = makeTempRoot(prefix);
  seedSourceTree(root);
  mockCwd(root);
  return root;
}

async function runAssembleAll(prefix = 'assemble-src-'): Promise<void> {
  makeSourceRoot(prefix);
  await assembleAll();
}

beforeEach(() => {
  outBase = makeTempRoot('assemble-out-');
  // 默认 cwd 指向空临时目录：用例忘写源码树时会直接失败，而不是回读仓库。
  mockCwd(makeTempRoot('assemble-no-source-'));
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
        const prefix = key === 'cursorHome' ? 'dev-team_' : '';
        const paths = [
          `agents/${prefix}implementation-generator.md`,
          `skills/${prefix}phase-proposal/SKILL.md`,
          env.hooksFilePath,
        ];
        expect(existsSync(outDir)).toBe(true);
        for (const rel of paths) {
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
      const root = makeSourceRoot('assemble-fixture-');
      writeFixtureFile(root, 'agents/broken-agent.md', '__INCLUDE:missing-fragment__\n');
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'fragment 环引用时 assemble 失败',
    async () => {
      const root = makeSourceRoot('assemble-cycle-');
      writeFixtureFile(root, '_fragments/loop-a.md', '__INCLUDE:loop-b__');
      writeFixtureFile(root, '_fragments/loop-b.md', '__INCLUDE:loop-a__');
      writeFixtureFile(root, 'agents/cycle-agent.md', '__INCLUDE:loop-a__\n');
      await expect(assembleAll()).rejects.toThrow(/Include cycle|Include depth/);
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    '_fragments/ 存在于源码树但不出现在任一 env.outDir',
    async () => {
      const root = makeSourceRoot('assemble-fragments-');
      expect(existsSync(join(root, '_fragments'))).toBe(true);
      await assembleAll();
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
        expect(existsSync(join(outDir, 'templates/adr.md'))).toBe(true);
      }
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'PRODUCT_ENV_KEYS 单轮全量执行失败即抛错',
    async () => {
      const root = makeSourceRoot('assemble-fail-fast-');
      rmSync(join(root, '_fragments/static-analysis-gate.md'));
      rmSync(join(root, '_fragments/static-analysis-gate.cursor.md'));
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'STATIC_BIN_FILES 不再包含退役的 openspec 静态 bin 文件',
    async () => {
      const root = makeSourceRoot('assemble-static-bins-');
      seedRetiredStaticBins(root);
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
      const root = makeSourceRoot('assemble-empty-staging-');
      rmSync(join(root, '.pack-staging/bin'), { recursive: true, force: true });
      mkdirSync(join(root, '.pack-staging/bin'), { recursive: true });
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
      const root = makeSourceRoot('assemble-no-staging-bin-');
      rmSync(join(root, '.pack-staging/bin'), { recursive: true, force: true });
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    '缺少 hooks/hooks.canonical.json 时 assembleAll 拒绝而非静默部分执行',
    async () => {
      const root = makeSourceRoot('assemble-no-hooks-');
      rmSync(join(root, 'hooks/hooks.canonical.json'));
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    'process.cwd 指向不存在路径时 assembleAll 抛出 ENOENT',
    async () => {
      const missing = join(tmpdir(), `assemble-missing-cwd-${Date.now()}`);
      mockCwd(missing);
      await expect(assembleAll()).rejects.toThrow();
    },
    ASSEMBLE_TIMEOUT,
  );

  it(
    '无 skills/ 与 agents/ 目录时 assembleAll 仍完成',
    async () => {
      const root = makeSourceRoot('assemble-no-skills-agents-');
      rmSync(join(root, 'skills'), { recursive: true, force: true });
      rmSync(join(root, 'agents'), { recursive: true, force: true });
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
