/**
 * 单元测试: MCP 项目根候选采集与 per-call resolve（无默认锁定）
 *
 * @see openspec/changes/mcp-workspace-root/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { pathToFileURL } from 'node:url';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import type { McpServerLike } from './project-root';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

interface MockServerOptions {
  capabilities?: { roots?: { listChanged?: boolean } } | Record<string, never>;
  listRootsResult?: { roots: { uri: string; name?: string }[] };
  listRootsError?: Error;
  /** When true, listRoots never resolves (exercises ROOTS_LIST_TIMEOUT_MS). */
  listRootsHang?: boolean;
}

function createMockServer(options: MockServerOptions = {}) {
  let listRootsCalls = 0;

  const server: McpServerLike = {
    getClientCapabilities: () =>
      options.capabilities !== undefined ? options.capabilities : { roots: { listChanged: false } },
    listRoots: async () => {
      listRootsCalls += 1;
      if (options.listRootsHang) {
        return new Promise(() => {
          /* never resolves */
        });
      }
      if (options.listRootsError) {
        throw options.listRootsError;
      }
      return options.listRootsResult ?? { roots: [] };
    },
  };

  return {
    server,
    get listRootsCalls() {
      return listRootsCalls;
    },
  };
}

const ENV_KEYS = ['CLAUDE_PROJECT_DIR', 'WORKSPACE_FOLDER_PATHS'] as const;

function saveEnv(): Record<(typeof ENV_KEYS)[number], string | undefined> {
  return {
    CLAUDE_PROJECT_DIR: process.env.CLAUDE_PROJECT_DIR,
    WORKSPACE_FOLDER_PATHS: process.env.WORKSPACE_FOLDER_PATHS,
  };
}

function restoreEnv(saved: ReturnType<typeof saveEnv>): void {
  for (const key of ENV_KEYS) {
    if (saved[key] === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = saved[key];
    }
  }
}

function clearProjectEnv(): void {
  delete process.env.CLAUDE_PROJECT_DIR;
  delete process.env.WORKSPACE_FOLDER_PATHS;
}

function makeTempDir(prefix = 'project-root-'): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

function fileUriFor(dir: string): string {
  return pathToFileURL(dir).href;
}

function withTrailingSep(p: string): string {
  return p.endsWith(path.sep) ? p : p + path.sep;
}

async function loadFreshModule() {
  vi.resetModules();
  return import('./project-root');
}

type ProjectRootMod = Awaited<ReturnType<typeof loadFreshModule>>;

type ResolveErrorBody = {
  code: string;
  message: string;
  project_root?: string;
  candidates?: string[];
  force_hint?: string;
};

function isResolveErrorResult(
  result: unknown,
): result is { isError: true; content: { type: 'text'; text: string }[] } {
  return (
    typeof result === 'object' &&
    result !== null &&
    'isError' in result &&
    (result as { isError: unknown }).isError === true &&
    'content' in result &&
    Array.isArray((result as { content: unknown }).content)
  );
}

function parseResolveError(result: unknown): ResolveErrorBody {
  expect(isResolveErrorResult(result)).toBe(true);
  if (!isResolveErrorResult(result)) {
    throw new Error('expected resolve error result');
  }
  return JSON.parse(result.content[0].text) as ResolveErrorBody;
}

/** Success path: withResolvedProjectRoot returns the resolved root via identity run. */
async function resolveRoot(
  mod: ProjectRootMod,
  toolName: string,
  args: Record<string, unknown>,
): Promise<string> {
  const result = await mod.withResolvedProjectRoot(toolName, args, (r) => r);
  expect(isResolveErrorResult(result)).toBe(false);
  return result as string;
}

async function expectNotInCandidates(
  mod: ProjectRootMod,
  toolName: string,
  args: Record<string, unknown>,
): Promise<ResolveErrorBody> {
  const result = await mod.withResolvedProjectRoot(toolName, args, (r) => r);
  const body = parseResolveError(result);
  expect(body.code).toBe('not_in_candidates');
  return body;
}

async function expectInvalidPath(
  mod: ProjectRootMod,
  toolName: string,
  args: Record<string, unknown>,
): Promise<ResolveErrorBody> {
  const result = await mod.withResolvedProjectRoot(toolName, args, (r) => r);
  const body = parseResolveError(result);
  expect(body.code).toBe('invalid_path');
  return body;
}

// ===========================================================================
// collectProjectRootCandidates — 多通道合并去重
// ===========================================================================

describe('collectProjectRootCandidates — 多通道合并去重', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('CLAUDE + WORKSPACE 双可用 + roots 双可用时 candidates 含全部去重路径；len>1 不抛、不锁定默认根 (AC-1)', async () => {
    const claudeDir = makeTempDir();
    const wsA = makeTempDir();
    const wsB = makeTempDir();
    const rootA = makeTempDir();
    const rootB = makeTempDir();
    temps.push(claudeDir, wsA, wsB, rootA, rootB);
    process.env.CLAUDE_PROJECT_DIR = claudeDir;
    process.env.WORKSPACE_FOLDER_PATHS = `${wsA},${wsB}`;
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [{ uri: fileUriFor(rootA) }, { uri: fileUriFor(rootB) }],
      },
    });

    await expect(mod.collectProjectRootCandidates(mock.server)).resolves.toBeUndefined();
    const candidates = mod.getProjectRootCandidates();
    expect(candidates.length).toBeGreaterThanOrEqual(2);
    for (const p of [claudeDir, wsA, wsB, rootA, rootB]) {
      expect(candidates).toContain(path.resolve(p).replace(/[\\/]+$/, '') || p);
    }
    expect(mod).not.toHaveProperty('requireLockedProjectRoot');
  });

  it('仅 CLAUDE 单可用路径时 candidates 恰好含该路径；len==1 亦不自动锁定 (AC-1)', async () => {
    const claudeDir = makeTempDir();
    temps.push(claudeDir);
    process.env.CLAUDE_PROJECT_DIR = claudeDir;
    const mod = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await mod.collectProjectRootCandidates(mock.server);

    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(claudeDir)]);
    expect(mod).not.toHaveProperty('getMcpCachedProjectRoot');
  });

  it('无 CLAUDE 时 WORKSPACE 按 ,/; 分割后每一个可用路径入候选（非仅 [0]）(AC-1)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.WORKSPACE_FOLDER_PATHS = `${a};${b}`;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);

    const candidates = mod.getProjectRootCandidates();
    expect(candidates).toContain(path.resolve(a));
    expect(candidates).toContain(path.resolve(b));
    expect(candidates.length).toBe(2);
  });

  it('roots 多个可用 file:// / 裸绝对路径全部入候选；与 WORKSPACE 重叠去重仅一份 (AC-1)', async () => {
    const shared = makeTempDir();
    const onlyRoot = makeTempDir();
    temps.push(shared, onlyRoot);
    process.env.WORKSPACE_FOLDER_PATHS = shared;
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [{ uri: fileUriFor(shared) }, { uri: onlyRoot }],
      },
    });

    await mod.collectProjectRootCandidates(mock.server);
    const candidates = mod.getProjectRootCandidates();
    expect(candidates.filter((c) => path.resolve(c) === path.resolve(shared))).toHaveLength(1);
    expect(candidates).toContain(path.resolve(onlyRoot));
  });

  it('合并过程中单通道不可用（CLAUDE 非法）时其余通道结果仍完整入集 (AC-1)', async () => {
    const ws = makeTempDir();
    const root = makeTempDir();
    temps.push(ws, root);
    process.env.CLAUDE_PROJECT_DIR = 'relative/not-abs';
    process.env.WORKSPACE_FOLDER_PATHS = ws;
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: fileUriFor(root) }] },
    });

    await mod.collectProjectRootCandidates(mock.server);
    const candidates = mod.getProjectRootCandidates();
    expect(candidates).toContain(path.resolve(ws));
    expect(candidates).toContain(path.resolve(root));
    expect(candidates).not.toContain('relative/not-abs');
  });

  it('Windows 盘符大小写 / 尾斜杠不同的同一路径经 normalize 比较键去重后仅一份 (AC-1)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    const resolved = path.resolve(dir);
    process.env.CLAUDE_PROJECT_DIR = resolved;
    process.env.WORKSPACE_FOLDER_PATHS = withTrailingSep(resolved);
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toHaveLength(1);
  });
});

// ===========================================================================
// collectProjectRootCandidates — 通道失败不阻断
// ===========================================================================

describe('collectProjectRootCandidates — 通道失败不阻断', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it("CLAUDE_PROJECT_DIR='${workspaceFolder}' 被跳过；WORKSPACE 单可用仍入候选 (AC-1)", async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = '${workspaceFolder}';
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(dir)]);
    expect(mod.getProjectRootCandidates().join(',')).not.toContain('${workspaceFolder}');
  });

  it('CLAUDE 为相对路径 / 绝对不存在时跳过该通道，继续合并 WORKSPACE/roots (AC-1)', async () => {
    const ws = makeTempDir();
    temps.push(ws);
    process.env.CLAUDE_PROJECT_DIR = path.join(os.tmpdir(), `missing-${Date.now()}`);
    process.env.WORKSPACE_FOLDER_PATHS = ws;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(ws)]);
  });

  it('listRoots 抛错 / -32601 时 stderr 含 MCP roots/list failed:；Promise resolve；其它通道结果保留 (AC-1)', async () => {
    const ws = makeTempDir();
    temps.push(ws);
    process.env.WORKSPACE_FOLDER_PATHS = ws;
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsError: Object.assign(new Error('Method not found'), { code: -32601 }),
    });

    await expect(mod.collectProjectRootCandidates(mock.server)).resolves.toBeUndefined();
    expect(stderrSpy.mock.calls.some((c) => String(c[0]).includes('MCP roots/list failed:'))).toBe(
      true,
    );
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(ws)]);
    stderrSpy.mockRestore();
  });
});

// ===========================================================================
// collectProjectRootCandidates — listRoots 超时
// ===========================================================================

describe('collectProjectRootCandidates — listRoots 超时', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('listRoots hang 时 advance ≥2000ms 后 collect resolve；stderr 含 timed out 与 2000；clearTimeout ≥1；WORKSPACE 仍入候选 (AC-1)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const clearSpy = vi.spyOn(globalThis, 'clearTimeout');
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsHang: true,
    });

    const collectPromise = mod.collectProjectRootCandidates(mock.server);
    await vi.advanceTimersByTimeAsync(2000);
    await expect(collectPromise).resolves.toBeUndefined();

    const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(stderrText).toMatch(/timed out/i);
    expect(stderrText).toContain('2000');
    expect(clearSpy.mock.calls.length).toBeGreaterThanOrEqual(1);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(dir)]);
    stderrSpy.mockRestore();
    clearSpy.mockRestore();
  });

  it('advance 仅 1999ms 时仍 pending；再 +1ms 完成（证明 2000ms 常量）(AC-1)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsHang: true,
    });

    let settled = false;
    const collectPromise = mod.collectProjectRootCandidates(mock.server).then(() => {
      settled = true;
    });

    await vi.advanceTimersByTimeAsync(1999);
    expect(settled).toBe(false);

    await vi.advanceTimersByTimeAsync(1);
    await collectPromise;
    expect(settled).toBe(true);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(dir)]);
  });
});

// ===========================================================================
// collectProjectRootCandidates — 边界 / 无 cwd / 无默认根
// ===========================================================================

describe('collectProjectRootCandidates — 边界', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('CLAUDE 为 undefined / "" 时跳过 env 通道', async () => {
    const ws = makeTempDir();
    temps.push(ws);
    process.env.WORKSPACE_FOLDER_PATHS = ws;
    delete process.env.CLAUDE_PROJECT_DIR;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(ws)]);

    clearProjectEnv();
    process.env.CLAUDE_PROJECT_DIR = '';
    process.env.WORKSPACE_FOLDER_PATHS = ws;
    const mod2 = await loadFreshModule();
    await mod2.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod2.getProjectRootCandidates()).toEqual([path.resolve(ws)]);
  });

  it('CLAUDE 超长不存在路径（>1000 chars）不入候选', async () => {
    const longMissing = path.join(os.tmpdir(), 'x'.repeat(1001));
    process.env.CLAUDE_PROJECT_DIR = longMissing;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toEqual([]);
  });

  it('CLAUDE 含空格 / emoji 且存在时入候选且展示路径严格等于该路径', async () => {
    const parent = makeTempDir();
    temps.push(parent);
    const special = path.join(parent, 'space dir 🧪');
    fs.mkdirSync(special);
    process.env.CLAUDE_PROJECT_DIR = special;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(special)]);
  });

  it('WORKSPACE 为 undefined / ,,;; / 仅相对段时该通道无贡献；空候选不含 process.cwd() (AC-1/AC-8)', async () => {
    process.env.WORKSPACE_FOLDER_PATHS = ',,;;';
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toEqual([]);
    expect(mod.getProjectRootCandidates()).not.toContain(process.cwd());

    clearProjectEnv();
    process.env.WORKSPACE_FOLDER_PATHS = 'relative/a,relative/b';
    const mod2 = await loadFreshModule();
    await mod2.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod2.getProjectRootCandidates()).toEqual([]);
  });

  it('WORKSPACE 含空白 trim 后可用路径正确纳入', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = `  ${dir}  `;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(dir)]);
  });

  it('WORKSPACE 超大列表（≥100 个可用绝对段）：全部入候选、去重正确、不崩溃 (AC-1)', async () => {
    const base = makeTempDir();
    temps.push(base);
    const dirs: string[] = [];
    for (let i = 0; i < 100; i++) {
      const d = path.join(base, `d${i}`);
      fs.mkdirSync(d);
      dirs.push(d);
    }
    // duplicate first path at end to exercise dedupe
    process.env.WORKSPACE_FOLDER_PATHS = [...dirs, dirs[0]].join(',');
    const mod = await loadFreshModule();
    const start = Date.now();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(Date.now() - start).toBeLessThan(30_000);
    expect(mod.getProjectRootCandidates()).toHaveLength(100);
  });

  it('roots 超大列表（≥100 个可用根）：全部可用项入候选、重叠去重、不崩溃 (AC-1)', async () => {
    const base = makeTempDir();
    temps.push(base);
    const dirs: string[] = [];
    for (let i = 0; i < 100; i++) {
      const d = path.join(base, `r${i}`);
      fs.mkdirSync(d);
      dirs.push(d);
    }
    process.env.WORKSPACE_FOLDER_PATHS = dirs[0];
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: dirs.map((d) => ({ uri: fileUriFor(d) })),
      },
    });
    await mod.collectProjectRootCandidates(mock.server);
    expect(mod.getProjectRootCandidates()).toHaveLength(100);
  });

  it('客户端无 roots capability 时 listRootsCalls === 0', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const mod = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });
    await mod.collectProjectRootCandidates(mock.server);
    expect(mock.listRootsCalls).toBe(0);
  });

  it('roots 含 http:// / "" / 相对 / 不存在条目时跳过坏项，可用项仍入候选', async () => {
    const usable = makeTempDir();
    temps.push(usable);
    const missing = path.join(os.tmpdir(), `missing-root-${Date.now()}`);
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [
          { uri: '' },
          { uri: 'http://example.com/repo' },
          { uri: 'relative/not-absolute' },
          { uri: missing },
          { uri: fileUriFor(usable) },
        ],
      },
    });
    await mod.collectProjectRootCandidates(mock.server);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(usable)]);
  });

  it('roots 全部不可用时 stderr 含 no usable file roots；candidates 不因此通道写入 (AC-1)', async () => {
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [{ uri: '' }, { uri: 'http://example.com/repo' }, { uri: 'relative/path' }],
      },
    });
    await mod.collectProjectRootCandidates(mock.server);
    const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(stderrText).toMatch(/no usable file roots/i);
    expect(mod.getProjectRootCandidates()).toEqual([]);
    stderrSpy.mockRestore();
  });

  it('URI 含 %20 时解码路径与本地 path.resolve 一致后入候选', async () => {
    const parent = makeTempDir();
    temps.push(parent);
    const spaced = path.join(parent, 'has space');
    fs.mkdirSync(spaced);
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: pathToFileURL(spaced).href }] },
    });
    await mod.collectProjectRootCandidates(mock.server);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(spaced)]);
  });

  it('WORKSPACE 超大列表中夹杂坏段时：坏段跳过、可用段仍完整入集 (AC-1)', async () => {
    const base = makeTempDir();
    temps.push(base);
    const good: string[] = [];
    const segments: string[] = [];
    for (let i = 0; i < 20; i++) {
      const d = path.join(base, `ok${i}`);
      fs.mkdirSync(d);
      good.push(d);
      segments.push(d, 'relative', '${x}', path.join(os.tmpdir(), `nope-${i}-${Date.now()}`));
    }
    process.env.WORKSPACE_FOLDER_PATHS = segments.join(',');
    const mod = await loadFreshModule();
    await expect(
      mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server),
    ).resolves.toBeUndefined();
    expect(mod.getProjectRootCandidates()).toHaveLength(good.length);
  });

  it('roots 超大列表中夹杂坏项时跳过坏项；listRoots reject 时 stderr 记录且其它通道保留 (AC-1)', async () => {
    const usable = makeTempDir();
    temps.push(usable);
    process.env.WORKSPACE_FOLDER_PATHS = usable;
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const mod = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsError: new Error('boom-roots'),
    });
    await mod.collectProjectRootCandidates(mock.server);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(usable)]);
    expect(stderrSpy.mock.calls.some((c) => String(c[0]).includes('MCP roots/list failed:'))).toBe(
      true,
    );
    stderrSpy.mockRestore();
  });
});

describe('collectProjectRootCandidates — 无 cwd', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('三通道均失败后 candidates 为空数组；不得含 process.cwd() (AC-8)', async () => {
    process.env.CLAUDE_PROJECT_DIR = '${workspaceFolder}';
    process.env.WORKSPACE_FOLDER_PATHS = 'relative/a';
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toEqual([]);
    expect(mod.getProjectRootCandidates()).not.toContain(process.cwd());
  });
});

describe('collectProjectRootCandidates — 无默认根', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('单候选时二次 collect 可合并新通道路径；无 requireLocked 导出且不因 len==1 抛 multi_root (AC-1)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.CLAUDE_PROJECT_DIR = a;
    const mod = await loadFreshModule();
    expect(mod).not.toHaveProperty('requireLockedProjectRoot');
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toHaveLength(1);

    delete process.env.CLAUDE_PROJECT_DIR;
    process.env.WORKSPACE_FOLDER_PATHS = b;
    await expect(
      mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server),
    ).resolves.toBeUndefined();
    expect(mod.getProjectRootCandidates().length).toBeGreaterThanOrEqual(2);
  });

  it('二次 collect 时某通道抛错不得擦除首次已采候选；Promise 仍 resolve (AC-1)', async () => {
    const a = makeTempDir();
    temps.push(a);
    process.env.CLAUDE_PROJECT_DIR = a;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const first = [...mod.getProjectRootCandidates()];

    vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsError: new Error('second-fail'),
    });
    await expect(mod.collectProjectRootCandidates(mock.server)).resolves.toBeUndefined();
    expect(mod.getProjectRootCandidates()).toEqual(first);
  });

  it('二次 collect 与首次完全相同通道输入时 candidates 幂等 (AC-1)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.WORKSPACE_FOLDER_PATHS = `${a},${b}`;
    const mod = await loadFreshModule();
    const mockOpts = { capabilities: {} as const };
    await mod.collectProjectRootCandidates(createMockServer(mockOpts).server);
    const first = [...mod.getProjectRootCandidates()];
    await mod.collectProjectRootCandidates(createMockServer(mockOpts).server);
    expect(mod.getProjectRootCandidates()).toEqual(first);
  });
});

// ===========================================================================
// withResolvedProjectRoot — resolve / force / call-scoped
// ===========================================================================

describe('withResolvedProjectRoot (resolve) — ∈ 候选放行', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('collect 后传入 ∈ candidates 的合法绝对路径：返回 normalize 后绝对路径；pending 被清除 (AC-3)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);

    const resolved = await resolveRoot(mod, 'config_get', { project_root: dir, key: 'x' });
    expect(resolved).toBe(path.resolve(dir));

    // pending cleared: a subsequent not-in-candidates call should start fresh force flow
    const other = makeTempDir();
    temps.push(other);
    await expectNotInCandidates(mod, 'config_get', { project_root: other, key: 'x' });
  });

  it('Windows 下盘符大小写不同但比较键相同的路径视为 ∈ 候选并放行 (AC-3)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);

    if (process.platform === 'win32') {
      const flipped = dir.replace(/^([a-zA-Z]):/, (_m, d: string) =>
        d === d.toUpperCase() ? `${d.toLowerCase()}:` : `${d.toUpperCase()}:`,
      );
      expect(await resolveRoot(mod, 'config_get', { project_root: flipped })).toBe(
        path.resolve(dir),
      );
    } else {
      expect(await resolveRoot(mod, 'config_get', { project_root: dir })).toBe(path.resolve(dir));
    }
  });

  it('尾斜杠差异经 normalize 后视为 ∈ 候选（保留盘符根/）(AC-3)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const withSep = withTrailingSep(dir);
    expect(await resolveRoot(mod, 'change_list', { project_root: withSep })).toBe(
      path.resolve(dir),
    );
  });

  it('Windows 盘符根作为 project_root 时 ∈ 候选放行且保留根形式 (AC-3)', async () => {
    if (process.platform !== 'win32') {
      return;
    }
    const dir = makeTempDir();
    temps.push(dir);
    const root = path.parse(dir).root; // e.g. "C:\\"
    process.env.CLAUDE_PROJECT_DIR = root;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const resolved = await resolveRoot(mod, 'change_list', { project_root: root });
    expect(resolved).toMatch(/^[A-Za-z]:[\\/]$/);
    expect(mod.getProjectRootCandidates()).toContain(resolved);
  });

  it('∈ 候选放行前若磁盘被删：确定性 invalid_path，不得静默返回已删路径', async () => {
    const dir = makeTempDir();
    process.env.CLAUDE_PROJECT_DIR = dir;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    fs.rmSync(dir, { recursive: true, force: true });
    await expectInvalidPath(mod, 'config_get', { project_root: dir });
  });

  it('多候选时传入比较键等于 candidates[i] 但展示路径尾斜杠不同：放行且返回 normalize 展示形式 (AC-3)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.WORKSPACE_FOLDER_PATHS = `${a},${b}`;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const result = await resolveRoot(mod, 'change_list', {
      project_root: withTrailingSep(b),
    });
    expect(result).toBe(path.resolve(b));
  });
});

describe('withResolvedProjectRoot (resolve) — invalid_path', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  async function setupWithCandidate() {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    return { mod, dir };
  }

  it('project_root 相对路径：抛 invalid_path；不更新 pending (AC-4 前置)', async () => {
    const { mod, dir } = await setupWithCandidate();
    const body = await expectInvalidPath(mod, 'config_get', {
      project_root: 'relative/path',
      key: 'a',
    });
    expect(body.message).toContain('not an absolute path');
    expect(body.message).toContain('relative/path');
    expect(body.force_hint).toBeUndefined();
    // pending not set: first valid out-of-candidates still needs two calls
    const other = makeTempDir();
    temps.push(other);
    await expectNotInCandidates(mod, 'config_get', { project_root: other, key: 'a' });
    expect(await resolveRoot(mod, 'config_get', { project_root: other, key: 'a' })).toBe(
      path.resolve(other),
    );
    expect(mod.getProjectRootCandidates()).toContain(path.resolve(dir));
  });

  it('project_root 绝对但不存在：invalid_path；pending 不变', async () => {
    const { mod } = await setupWithCandidate();
    const missing = path.join(os.tmpdir(), `missing-pr-${Date.now()}`);
    const body = await expectInvalidPath(mod, 'config_get', { project_root: missing });
    expect(body.message).toContain('does not exist');
    expect(body.message).toContain(missing);
  });

  it('project_root 含 ${...}：invalid_path；project_root 字段回显原始入参', async () => {
    const { mod } = await setupWithCandidate();
    const raw = '${workspaceFolder}';
    const body = await expectInvalidPath(mod, 'config_get', { project_root: raw });
    expect(body.project_root).toBe(raw);
    expect(body.message).toContain('unexpanded literal');
    expect(body.message).toContain(raw);
  });

  it('project_root 为 undefined / 缺失 / 非 string：invalid_path 或确定性拒绝', async () => {
    const { mod } = await setupWithCandidate();
    expect((await expectInvalidPath(mod, 'config_get', {})).project_root).toBe('');
    expect((await expectInvalidPath(mod, 'config_get', { project_root: null })).project_root).toBe(
      '',
    );
    expect((await expectInvalidPath(mod, 'config_get', { project_root: 1 })).project_root).toBe(
      '1',
    );
    expect((await expectInvalidPath(mod, 'config_get', { project_root: {} })).project_root).toBe(
      '',
    );
  });

  it('project_root 为 ""：invalid_path', async () => {
    const { mod } = await setupWithCandidate();
    await expectInvalidPath(mod, 'config_get', { project_root: '' });
  });

  it('project_root 超长字符串（>1000 chars）不存在：invalid_path 且不崩溃', async () => {
    const { mod } = await setupWithCandidate();
    const long = path.join(os.tmpdir(), 'y'.repeat(1001));
    await expectInvalidPath(mod, 'config_get', { project_root: long });
  });

  it('project_root 含 \\n / \\0 / emoji 且不存在：确定性 invalid_path', async () => {
    const { mod } = await setupWithCandidate();
    for (const raw of [
      path.join(os.tmpdir(), 'a\nb'),
      path.join(os.tmpdir(), 'a\0b'),
      path.join(os.tmpdir(), `missing-🧪-${Date.now()}`),
    ]) {
      await expectInvalidPath(mod, 'config_get', { project_root: raw });
    }
  });
});

describe('withResolvedProjectRoot (resolve) — toolName 边界', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('常规 toolName（如 config_get）参与 pending 键：同名 + 全参再调 force 成功 (AC-7)', async () => {
    const outside = makeTempDir();
    temps.push(outside);
    const mod = await loadFreshModule();
    const args = { project_root: outside, key: 'schema' };
    await expectNotInCandidates(mod, 'config_get', args);
    expect(await resolveRoot(mod, 'config_get', args)).toBe(path.resolve(outside));
  });

  it('toolName 为 ""：合法 ∉ 候选时仍写 pending；同 "" + 全参再调可 force (AC-7)', async () => {
    const outside = makeTempDir();
    temps.push(outside);
    const mod = await loadFreshModule();
    const args = { project_root: outside };
    await expectNotInCandidates(mod, '', args);
    expect(await resolveRoot(mod, '', args)).toBe(path.resolve(outside));
  });

  it('toolName 超长字符串（>1000 chars）：pending 键可稳定建立与匹配；force 成功 (AC-7)', async () => {
    const outside = makeTempDir();
    temps.push(outside);
    const mod = await loadFreshModule();
    const toolName = 't'.repeat(1001);
    const args = { project_root: outside };
    await expectNotInCandidates(mod, toolName, args);
    expect(await resolveRoot(mod, toolName, args)).toBe(path.resolve(outside));
  });

  it('toolName 含 \\n / emoji / 特殊字符：pending 键按字面拼接，同字面再调 force，改一字面则不 force (AC-7)', async () => {
    const outside = makeTempDir();
    temps.push(outside);
    const mod = await loadFreshModule();
    const toolName = 'cfg\nget-🧪';
    const args = { project_root: outside };
    await expectNotInCandidates(mod, toolName, args);
    await expectNotInCandidates(mod, 'cfg\nget-🧪x', args);
    // overwritten pending — force with the latest toolName
    expect(await resolveRoot(mod, 'cfg\nget-🧪x', args)).toBe(path.resolve(outside));
  });

  it('toolName 为 undefined / null（运行时非 string）：确定性拒绝或规范化为字符串键，不得抛未捕获 TypeError', async () => {
    const outside = makeTempDir();
    temps.push(outside);
    const mod = await loadFreshModule();
    const args = { project_root: outside };
    for (const toolName of [undefined, null] as unknown as string[]) {
      let threwTypeError = false;
      let body: ResolveErrorBody | undefined;
      try {
        body = await expectNotInCandidates(mod, toolName, args);
      } catch (err) {
        threwTypeError = err instanceof TypeError;
      }
      expect(threwTypeError).toBe(false);
      expect(body?.code).toBe('not_in_candidates');
    }
  });
});

describe('withResolvedProjectRoot (resolve) — ∉ 候选', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('合法绝对存在但 ∉ candidates：抛 not_in_candidates；含 candidates 与 force_hint (AC-4)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.CLAUDE_PROJECT_DIR = a;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const e = await expectNotInCandidates(mod, 'config_get', {
      project_root: b,
      key: 'schema',
    });
    expect(e.candidates).toEqual(mod.getProjectRootCandidates());
    expect(e.force_hint).toMatch(/identical complete arguments/i);
    expect(e.force_hint).toMatch(/force-add/i);
    expect(e.project_root).toBe(b);
  });

  it('∉ 候选后紧接一次 invalid_path：pending 不被非法调用清除；再同全参合法调用仍可 force (AC-7)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.CLAUDE_PROJECT_DIR = a;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const args = { project_root: b, key: 'schema' };
    await expectNotInCandidates(mod, 'config_get', args);
    await expectInvalidPath(mod, 'config_get', { project_root: '' });
    expect(await resolveRoot(mod, 'config_get', args)).toBe(path.resolve(b));
  });
});

describe('withResolvedProjectRoot (resolve) — force', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('第一次 ∉ 候选写入 pending 后，同一 toolName + 完整相同 args 再调：加入 candidates、清 pending、返回 P (AC-5)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.CLAUDE_PROJECT_DIR = a;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const args = { project_root: b, key: 'schema' };
    await expectNotInCandidates(mod, 'config_get', args);
    expect(await resolveRoot(mod, 'config_get', args)).toBe(path.resolve(b));
    expect(mod.getProjectRootCandidates()).toContain(path.resolve(b));
  });

  it('第二次调用仅改 arguments 其它字段：pending 键不匹配，仍 not_in_candidates (AC-7)', async () => {
    const b = makeTempDir();
    temps.push(b);
    const mod = await loadFreshModule();
    await expectNotInCandidates(mod, 'config_get', { project_root: b, key: 'a' });
    await expectNotInCandidates(mod, 'config_get', { project_root: b, key: 'a', extra: 1 });
  });

  it('第二次调用仅改 toolName：不 force，写新 pending (AC-7)', async () => {
    const b = makeTempDir();
    temps.push(b);
    const mod = await loadFreshModule();
    const args = { project_root: b };
    await expectNotInCandidates(mod, 'config_get', args);
    await expectNotInCandidates(mod, 'change_list', args);
    expect(await resolveRoot(mod, 'change_list', args)).toBe(path.resolve(b));
  });

  it('arguments 键顺序不同但稳定序列化后相同：应匹配 pending 并 force (AC-7)', async () => {
    const b = makeTempDir();
    temps.push(b);
    const mod = await loadFreshModule();
    await expectNotInCandidates(mod, 'config_get', { project_root: b, key: 'schema', z: 1 });
    expect(await resolveRoot(mod, 'config_get', { z: 1, key: 'schema', project_root: b })).toBe(
      path.resolve(b),
    );
  });

  it('成功 ∈ 放行后 pending 为清除状态；随后另一次 ∉ 可重新建立 pending (AC-7)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    const c = makeTempDir();
    temps.push(a, b, c);
    process.env.CLAUDE_PROJECT_DIR = a;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(await resolveRoot(mod, 'config_get', { project_root: a })).toBe(path.resolve(a));
    await expectNotInCandidates(mod, 'config_get', { project_root: b });
    expect(await resolveRoot(mod, 'config_get', { project_root: b })).toBe(path.resolve(b));
  });
});

describe('withResolvedProjectRoot (resolve) — 空候选逃生', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('candidates 为空时第一次合法绝对路径调用必拒 not_in_candidates 且 candidates: [] (AC-6)', async () => {
    const b = makeTempDir();
    temps.push(b);
    const mod = await loadFreshModule();
    const e = await expectNotInCandidates(mod, 'config_get', { project_root: b });
    expect(e.candidates).toEqual([]);
  });

  it('同全参再调：P 加入 candidates 并放行 (AC-6)', async () => {
    const b = makeTempDir();
    temps.push(b);
    const mod = await loadFreshModule();
    const args = { project_root: b, key: 'schema' };
    await expectNotInCandidates(mod, 'config_get', args);
    expect(await resolveRoot(mod, 'config_get', args)).toBe(path.resolve(b));
    expect(mod.getProjectRootCandidates()).toContain(path.resolve(b));
  });

  it('空候选下第一次非法路径不写入 pending；随后合法全参两次仍需完整 force 流程 (AC-6/AC-7)', async () => {
    const b = makeTempDir();
    temps.push(b);
    const mod = await loadFreshModule();
    await expectInvalidPath(mod, 'config_get', { project_root: '' });
    const args = { project_root: b };
    await expectNotInCandidates(mod, 'config_get', args);
    expect(await resolveRoot(mod, 'config_get', args)).toBe(path.resolve(b));
  });
});

describe('withResolvedProjectRoot (resolve) — 无 cwd', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('空候选 + 未传/非法路径时不得回退 process.cwd() 作为返回值 (AC-8)', async () => {
    const mod = await loadFreshModule();
    const fakeCwd = path.join(os.tmpdir(), 'fake-cwd');
    const cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(fakeCwd);
    const body = await expectInvalidPath(mod, 'config_get', {});
    expect(body.project_root).not.toBe(fakeCwd);
    expect(body.code).toBe('invalid_path');
    cwdSpy.mockRestore();
  });

  it('spy process.cwd 返回固定路径；任意 resolve 成功路径均不得等于该 spy 返回值（除非显式传入）(AC-8)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    const fakeCwd = path.join(os.tmpdir(), `spy-cwd-${Date.now()}`);
    fs.mkdirSync(fakeCwd);
    temps.push(fakeCwd);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(fakeCwd);
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const resolved = await resolveRoot(mod, 'config_get', { project_root: dir });
    expect(resolved).toBe(path.resolve(dir));
    expect(resolved).not.toBe(fakeCwd);
    cwdSpy.mockRestore();
  });
});

describe('withResolvedProjectRoot (resolve) — 根语义原样', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('上层存在 .git 与 openspec/ 时传入子目录路径返回值严格等于该子目录 (AC-9)', async () => {
    const parent = makeTempDir();
    temps.push(parent);
    fs.mkdirSync(path.join(parent, '.git'));
    fs.mkdirSync(path.join(parent, 'openspec'), { recursive: true });
    const child = path.join(parent, 'workspace');
    fs.mkdirSync(child);
    process.env.CLAUDE_PROJECT_DIR = child;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(await resolveRoot(mod, 'change_list', { project_root: child })).toBe(
      path.resolve(child),
    );
    expect(await resolveRoot(mod, 'change_list', { project_root: child })).not.toBe(
      path.resolve(parent),
    );
  });

  it('force 加入候选的路径同样不向上 walk (AC-9)', async () => {
    const parent = makeTempDir();
    temps.push(parent);
    fs.mkdirSync(path.join(parent, '.git'));
    fs.mkdirSync(path.join(parent, 'openspec'), { recursive: true });
    const child = path.join(parent, 'workspace');
    fs.mkdirSync(child);
    const mod = await loadFreshModule();
    const args = { project_root: child };
    await expectNotInCandidates(mod, 'config_get', args);
    expect(await resolveRoot(mod, 'config_get', args)).toBe(path.resolve(child));
  });

  it('传入上层 git 根而候选仅为子目录：应 not_in_candidates (AC-9/AC-4)', async () => {
    const parent = makeTempDir();
    temps.push(parent);
    fs.mkdirSync(path.join(parent, '.git'));
    const child = path.join(parent, 'workspace');
    fs.mkdirSync(child);
    process.env.CLAUDE_PROJECT_DIR = child;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    await expectNotInCandidates(mod, 'config_get', { project_root: parent });
  });

  it('子目录路径含尾斜杠 / . 段经 path.resolve 折叠后仍等于子目录比较键 (AC-9)', async () => {
    const parent = makeTempDir();
    temps.push(parent);
    const child = path.join(parent, 'workspace');
    fs.mkdirSync(child);
    process.env.CLAUDE_PROJECT_DIR = child;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const dotted = path.join(child, '.', '');
    expect(await resolveRoot(mod, 'change_list', { project_root: dotted })).toBe(
      path.resolve(child),
    );
  });
});

describe('withResolvedProjectRoot (resolve) — 无 realpath', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  function tryMakeLink(target: string, linkPath: string): boolean {
    try {
      if (process.platform === 'win32') {
        fs.symlinkSync(target, linkPath, 'junction');
      } else {
        fs.symlinkSync(target, linkPath);
      }
      return true;
    } catch {
      return false;
    }
  }

  it('不跟随符号链接：候选与入参比较不因 realpath 目标不同而误拒/误改写 (AC-9)', async () => {
    const real = makeTempDir();
    temps.push(real);
    const parent = makeTempDir();
    temps.push(parent);
    const linkPath = path.join(parent, 'link-ws');
    const linked = tryMakeLink(real, linkPath);
    if (!linked) {
      // Platform cannot create links: still assert path identity is by normalize, not rewrite to parent.
      process.env.CLAUDE_PROJECT_DIR = real;
      const mod = await loadFreshModule();
      await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
      expect(await resolveRoot(mod, 'config_get', { project_root: real })).toBe(path.resolve(real));
      return;
    }
    process.env.CLAUDE_PROJECT_DIR = linkPath;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(await resolveRoot(mod, 'config_get', { project_root: linkPath })).toBe(
      path.resolve(linkPath),
    );
  });

  it('候选存 symlink 路径、入参为其 realpath 目标且比较键不同：应 not_in_candidates (AC-9/AC-4)', async () => {
    const real = makeTempDir();
    temps.push(real);
    const parent = makeTempDir();
    temps.push(parent);
    const linkPath = path.join(parent, 'link-ws2');
    const linked = tryMakeLink(real, linkPath);
    if (!linked) {
      const a = makeTempDir();
      const b = makeTempDir();
      temps.push(a, b);
      process.env.CLAUDE_PROJECT_DIR = a;
      const mod = await loadFreshModule();
      await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
      await expectNotInCandidates(mod, 'config_get', { project_root: b });
      return;
    }
    process.env.CLAUDE_PROJECT_DIR = linkPath;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const realResolved = fs.realpathSync(linkPath);
    if (path.resolve(realResolved).toLowerCase() === path.resolve(linkPath).toLowerCase()) {
      // Junction may resolve identically on some Windows setups — still assert candidate path returned as-is.
      expect(await resolveRoot(mod, 'config_get', { project_root: linkPath })).toBe(
        path.resolve(linkPath),
      );
      return;
    }
    await expectNotInCandidates(mod, 'config_get', { project_root: realResolved });
  });
});

// ===========================================================================
// withResolvedProjectRoot call-scoped / getProjectDir / candidates / error JSON
// ===========================================================================

describe('withResolvedProjectRoot (call-scoped)', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  async function setupCandidate(dir: string): Promise<ProjectRootMod> {
    process.env.CLAUDE_PROJECT_DIR = dir;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    // Drop CLI env so post-call getProjectDir() cannot echo the same path via CLAUDE fallback.
    delete process.env.CLAUDE_PROJECT_DIR;
    return mod;
  }

  it('projectRoot 为存在的绝对路径：fn 内 getProjectDir() === 传入根；结束后 call-scoped 清除', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    const mod = await setupCandidate(dir);
    const inside = await mod.withResolvedProjectRoot('config_get', { project_root: dir }, () =>
      mod.getProjectDir(),
    );
    expect(inside).toBe(path.resolve(dir));
    expect(mod.getProjectDir()).toBe(process.cwd());
  });

  it('fn 抛错时仍清除 call-scoped；再次 getProjectDir() 不返回已清除的 call-scoped 根', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    const mod = await setupCandidate(dir);
    await expect(
      mod.withResolvedProjectRoot('config_get', { project_root: dir }, () => {
        throw new Error('boom');
      }),
    ).rejects.toThrow('boom');
    expect(mod.getProjectDir()).toBe(process.cwd());
  });

  it('resolve 错误（""）不注入 call-scoped；结束后不得残留', async () => {
    const mod = await loadFreshModule();
    const cwd = process.cwd();
    await expectInvalidPath(mod, 'config_get', { project_root: '' });
    expect(mod.getProjectDir()).toBe(cwd);
  });

  it('嵌套 withResolvedProjectRoot 时内层结束后外层根仍有效', async () => {
    const outer = makeTempDir();
    const inner = makeTempDir();
    temps.push(outer, inner);
    process.env.WORKSPACE_FOLDER_PATHS = `${outer},${inner}`;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const result = await mod.withResolvedProjectRoot(
      'config_get',
      { project_root: outer },
      async () => {
        expect(mod.getProjectDir()).toBe(path.resolve(outer));
        await mod.withResolvedProjectRoot('change_list', { project_root: inner }, () => {
          expect(mod.getProjectDir()).toBe(path.resolve(inner));
        });
        expect(mod.getProjectDir()).toBe(path.resolve(outer));
        return mod.getProjectDir();
      },
    );
    expect(result).toBe(path.resolve(outer));
    expect(mod.getProjectDir()).not.toBe(path.resolve(outer));
  });

  it('普通 Error 从 run 抛出时不得包装为 isError JSON', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    const mod = await setupCandidate(dir);
    await expect(
      mod.withResolvedProjectRoot('config_get', { project_root: dir }, () => {
        throw new Error('biz-boom');
      }),
    ).rejects.toThrow('biz-boom');
  });
});

describe('getProjectDir — CLI 语义', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('call-scoped 优先于 CLAUDE / WORKSPACE / cwd (AC-11)', async () => {
    const scoped = makeTempDir();
    const claude = makeTempDir();
    temps.push(scoped, claude);
    process.env.CLAUDE_PROJECT_DIR = claude;
    process.env.WORKSPACE_FOLDER_PATHS = scoped;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(
      await mod.withResolvedProjectRoot('config_get', { project_root: scoped }, () =>
        mod.getProjectDir(),
      ),
    ).toBe(path.resolve(scoped));
  });

  it('无 call-scoped、可用 CLAUDE 时返回 CLAUDE (AC-11)', async () => {
    const claude = makeTempDir();
    temps.push(claude);
    process.env.CLAUDE_PROJECT_DIR = claude;
    const mod = await loadFreshModule();
    expect(mod.getProjectDir()).toBe(path.resolve(claude));
  });

  it('无 call-scoped/CLAUDE、WORKSPACE 恰 1 可用时返回该路径 (AC-11)', async () => {
    const ws = makeTempDir();
    temps.push(ws);
    process.env.WORKSPACE_FOLDER_PATHS = ws;
    const mod = await loadFreshModule();
    expect(mod.getProjectDir()).toBe(path.resolve(ws));
  });

  it('无 call-scoped 且 CLAUDE 为相对路径：跳过该值，继续 WORKSPACE/cwd (AC-11)', async () => {
    const ws = makeTempDir();
    temps.push(ws);
    process.env.CLAUDE_PROJECT_DIR = 'relative/path';
    process.env.WORKSPACE_FOLDER_PATHS = ws;
    const mod = await loadFreshModule();
    expect(mod.getProjectDir()).toBe(path.resolve(ws));
    expect(mod.getProjectDir()).not.toBe('relative/path');
  });

  it('无 call-scoped 且 CLAUDE 为绝对但不存在：跳过，继续 WORKSPACE/cwd (AC-11)', async () => {
    const ws = makeTempDir();
    temps.push(ws);
    const missing = path.join(os.tmpdir(), `missing-cli-${Date.now()}`);
    process.env.CLAUDE_PROJECT_DIR = missing;
    process.env.WORKSPACE_FOLDER_PATHS = ws;
    const mod = await loadFreshModule();
    expect(mod.getProjectDir()).toBe(path.resolve(ws));
    expect(mod.getProjectDir()).not.toBe(missing);
  });

  it('WORKSPACE 段全为相对 / 不存在 / ${...}：回退 process.cwd() (AC-11)', async () => {
    process.env.WORKSPACE_FOLDER_PATHS = `relative,${path.join(os.tmpdir(), 'nope')},\${x}`;
    const mod = await loadFreshModule();
    expect(mod.getProjectDir()).toBe(process.cwd());
  });

  it('call-scoped 结束后，CLAUDE 不可用且 WORKSPACE 多可用：回退 cwd (AC-11)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    const scoped = makeTempDir();
    temps.push(a, b, scoped);
    process.env.WORKSPACE_FOLDER_PATHS = `${a},${b},${scoped}`;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    await mod.withResolvedProjectRoot('config_get', { project_root: scoped }, () => {
      expect(mod.getProjectDir()).toBe(path.resolve(scoped));
    });
    expect(mod.getProjectDir()).toBe(process.cwd());
  });

  it('均不可用时返回 process.cwd()（仅 CLI）(AC-11)', async () => {
    const mod = await loadFreshModule();
    expect(mod.getProjectDir()).toBe(process.cwd());
  });

  it('CLAUDE 为 undefined / "" / ${...} 时跳过并继续 WORKSPACE/cwd (AC-11)', async () => {
    const ws = makeTempDir();
    temps.push(ws);
    process.env.CLAUDE_PROJECT_DIR = '${workspaceFolder}';
    process.env.WORKSPACE_FOLDER_PATHS = ws;
    const mod = await loadFreshModule();
    expect(mod.getProjectDir()).toBe(path.resolve(ws));
  });

  it('WORKSPACE 双可用时回退 cwd，不得取 [0] (AC-11)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.WORKSPACE_FOLDER_PATHS = `${a},${b}`;
    const mod = await loadFreshModule();
    expect(mod.getProjectDir()).toBe(process.cwd());
    expect(mod.getProjectDir()).not.toBe(path.resolve(a));
  });

  it('CLAUDE 超长不存在路径跳过；含 emoji 且存在时返回该路径 (AC-11)', async () => {
    process.env.CLAUDE_PROJECT_DIR = path.join(os.tmpdir(), 'q'.repeat(1001));
    const mod = await loadFreshModule();
    expect(mod.getProjectDir()).toBe(process.cwd());

    const parent = makeTempDir();
    temps.push(parent);
    const emojiDir = path.join(parent, 'emoji-🧪');
    fs.mkdirSync(emojiDir);
    process.env.CLAUDE_PROJECT_DIR = emojiDir;
    const mod2 = await loadFreshModule();
    expect(mod2.getProjectDir()).toBe(path.resolve(emojiDir));
  });
});

describe('getProjectRootCandidates', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('collect 后返回只读数组，内容与合并结果一致', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    expect(mod.getProjectRootCandidates()).toEqual([path.resolve(dir)]);
  });

  it('未 collect 时返回 []', async () => {
    const mod = await loadFreshModule();
    expect(mod.getProjectRootCandidates()).toEqual([]);
  });

  it('返回值被调用方 push 不得污染内部集合', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const mod = await loadFreshModule();
    await mod.collectProjectRootCandidates(createMockServer({ capabilities: {} }).server);
    const snap = mod.getProjectRootCandidates() as string[];
    expect(() => snap.push('/evil')).not.toThrow();
    expect(mod.getProjectRootCandidates()).not.toContain('/evil');
    expect(mod.getProjectRootCandidates()).toHaveLength(1);
  });
});

describe('withResolvedProjectRoot — 错误 JSON 载荷', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  const temps: string[] = [];

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    for (const dir of temps.splice(0)) {
      fs.rmSync(dir, { recursive: true, force: true });
    }
  });

  it('not_in_candidates 含 force_hint 推荐语义文案 (AC-4)', async () => {
    const b = makeTempDir();
    temps.push(b);
    const mod = await loadFreshModule();
    const body = await expectNotInCandidates(mod, 'config_get', { project_root: b });
    expect(body.force_hint ?? '').toMatch(/force-add/i);
    expect(body.force_hint ?? '').toMatch(/identical complete arguments/i);
  });

  it('invalid_path：code 正确；force_hint/candidates 缺省；project_root 回显入参', async () => {
    const mod = await loadFreshModule();
    const body = await expectInvalidPath(mod, 'config_get', { project_root: 'relative' });
    expect(body.code).toBe('invalid_path');
    expect(body.force_hint).toBeUndefined();
    expect(body.candidates).toBeUndefined();
    expect(body.project_root).toBe('relative');
  });

  it('candidates: [] 时空数组仍可序列化；force_hint 为非空 string', async () => {
    const b = makeTempDir();
    temps.push(b);
    const mod = await loadFreshModule();
    const e = await expectNotInCandidates(mod, 'config_get', { project_root: b });
    expect(e.candidates).toEqual([]);
    expect(typeof e.force_hint).toBe('string');
    expect((e.force_hint ?? '').length).toBeGreaterThan(0);
    expect(() => JSON.stringify(e)).not.toThrow();
  });

  it('模块不再导出 resolve/runWith/isError/ProjectRootResolveError（公共面仅 withResolved*）', async () => {
    const mod = await loadFreshModule();
    expect(mod).not.toHaveProperty('resolveProjectRootForTool');
    expect(mod).not.toHaveProperty('runWithCallScopedRoot');
    expect(mod).not.toHaveProperty('isProjectRootResolveError');
    expect(mod).not.toHaveProperty('ProjectRootResolveError');
    expect(mod).toHaveProperty('withResolvedProjectRoot');
  });
});
