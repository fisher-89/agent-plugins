/**
 * 单元测试: MCP 项目根锁定（env → roots 恰 1 → WORKSPACE 恰 1）
 *
 * @see openspec/changes/mcp-project-root-lock/test-design.md
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
  capabilities?: { roots?: { listChanged?: boolean } };
  listRootsResult?: { roots: { uri: string; name?: string }[] };
  listRootsError?: Error;
  /** When true, listRoots never resolves (exercises LIST_ROOTS_TIMEOUT_MS). */
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

async function loadFreshModule() {
  vi.resetModules();
  return import('./project-root');
}

// ===========================================================================
// initProjectRootFromMcp — 启动锁定优先级
// ===========================================================================

describe('initProjectRootFromMcp — 启动锁定优先级', () => {
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

  it('CLAUDE_PROJECT_DIR 为已存在绝对路径时锁定该路径，且 listRootsCalls === 0；即使同时设置可用 WORKSPACE 与可返回的 roots，缓存仍严格等于 CLAUDE 路径 (AC-1)', async () => {
    const claudeDir = makeTempDir();
    const workspaceDir = makeTempDir();
    const rootsDir = makeTempDir();
    temps.push(claudeDir, workspaceDir, rootsDir);
    process.env.CLAUDE_PROJECT_DIR = claudeDir;
    process.env.WORKSPACE_FOLDER_PATHS = workspaceDir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: fileUriFor(rootsDir) }] },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(claudeDir);
    expect(getMcpCachedProjectRoot()).not.toBe(workspaceDir);
    expect(getMcpCachedProjectRoot()).not.toBe(rootsDir);
    expect(mock.listRootsCalls).toBe(0);
  });

  it('无可用 CLAUDE 时 listRoots 返回恰好 1 个可用 file:// 根则锁定为该路径；同时设置可用 WORKSPACE 时缓存仍等于 roots 路径而非 WORKSPACE (AC-1)', async () => {
    const rootsDir = makeTempDir();
    const workspaceDir = makeTempDir();
    temps.push(rootsDir, workspaceDir);
    process.env.WORKSPACE_FOLDER_PATHS = workspaceDir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: fileUriFor(rootsDir), name: 'only' }] },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(rootsDir);
    expect(getMcpCachedProjectRoot()).not.toBe(workspaceDir);
  });

  it('env/roots 均不可用时 WORKSPACE_FOLDER_PATHS 为单个绝对存在路径（; 分隔，含尾部分隔符）则锁定且 getMcpCachedProjectRoot() 严格等于该路径 (AC-1)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = `${dir};`;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
  });

  it('WORKSPACE_FOLDER_PATHS 为单个绝对存在路径（, 分隔）则锁定且严格等于该路径 (AC-1)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = `${dir},`;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
  });

  it('roots 条目为裸绝对本地路径（非 file://）且可用时锁定为该路径 (AC-1)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: dir }] },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
  });

  it('roots 含多个条目但仅 1 个可转换为可用绝对路径（其余为空 URI / http:// / 相对路径 / 不存在路径）时锁定该唯一可用根，不得因总数 >1 失败 (AC-1)', async () => {
    const usable = makeTempDir();
    temps.push(usable);
    const missingAbs = path.join(os.tmpdir(), `missing-root-${Date.now()}`);
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [
          { uri: '' },
          { uri: 'http://example.com/repo' },
          { uri: 'relative/not-absolute' },
          { uri: fileUriFor(missingAbs) },
          { uri: fileUriFor(usable) },
        ],
      },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(usable);
  });

  it('CLAUDE_PROJECT_DIR 为 ${workspaceFolder} 字面量时拒绝该通道；缓存等于后续 WORKSPACE 路径；requireLocked 不得再抛（已锁定）(AC-1)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = '${workspaceFolder}';
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot, requireLockedProjectRoot } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
    expect(() => requireLockedProjectRoot()).not.toThrow();
    expect(requireLockedProjectRoot()).toBe(dir);
  });

  it('CLAUDE_PROJECT_DIR 为相对路径时不锁定该通道，继续 WORKSPACE；缓存不等于相对字符串本身 (AC-1)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = 'relative/path';
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
    expect(getMcpCachedProjectRoot()).not.toBe('relative/path');
  });

  it('CLAUDE_PROJECT_DIR 为绝对但不存在路径时不锁定该通道 (AC-1)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = path.join(os.tmpdir(), `missing-${Date.now()}-no-such-dir`);
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
  });

  it('listRoots 抛错 / -32601 时该通道失败并写 stderr，继续 WORKSPACE；init Promise resolve（不抛）且缓存等于 WORKSPACE；stderr 含精确前缀 MCP roots/list failed: (AC-1/AC-2)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsError: new Error('Method not found: -32601'),
    });

    await expect(initProjectRootFromMcp(mock.server)).resolves.toBeUndefined();

    expect(getMcpCachedProjectRoot()).toBe(dir);
    const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(stderrText).toContain('MCP roots/list failed:');
    stderrSpy.mockRestore();
  });
});

// ===========================================================================
// initProjectRootFromMcp — listRoots 超时
// ===========================================================================

describe('initProjectRootFromMcp — listRoots 超时', () => {
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
    vi.useRealTimers();
  });

  it('listRoots 永不 resolve 时，advance fake timers ≥ 2000ms 后 init resolve；缓存等于后续 WORKSPACE；stderr 含 timed out 与 2000；listRootsCalls === 1；spy clearTimeout 被调用 ≥1 次 (AC-1/AC-2)', async () => {
    vi.useFakeTimers();
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const clearTimeoutSpy = vi.spyOn(globalThis, 'clearTimeout');
    try {
      const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
      const mock = createMockServer({
        capabilities: { roots: {} },
        listRootsHang: true,
      });

      const initPromise = initProjectRootFromMcp(mock.server);
      await vi.advanceTimersByTimeAsync(2000);
      await expect(initPromise).resolves.toBeUndefined();

      expect(getMcpCachedProjectRoot()).toBe(dir);
      expect(mock.listRootsCalls).toBe(1);
      const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
      expect(stderrText).toMatch(/timed out/i);
      expect(stderrText).toContain('2000');
      expect(clearTimeoutSpy.mock.calls.length).toBeGreaterThanOrEqual(1);
    } finally {
      clearTimeoutSpy.mockRestore();
      stderrSpy.mockRestore();
    }
  });

  it('advance timers 仅 1999ms 时 init 仍 pending；再 advance 到 2000ms 才完成并锁定 WORKSPACE（证明超时阈值分支而非任意短超时）(AC-1/AC-2)', async () => {
    vi.useFakeTimers();
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    try {
      const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
      const mock = createMockServer({
        capabilities: { roots: {} },
        listRootsHang: true,
      });

      let settled = false;
      const initPromise = initProjectRootFromMcp(mock.server).then(() => {
        settled = true;
      });

      await vi.advanceTimersByTimeAsync(1999);
      // 让微任务有机会跑完；超时尚未触发时 settled 仍为 false
      await Promise.resolve();
      expect(settled).toBe(false);
      expect(getMcpCachedProjectRoot()).toBeNull();

      await vi.advanceTimersByTimeAsync(1);
      await initPromise;

      expect(settled).toBe(true);
      expect(getMcpCachedProjectRoot()).toBe(dir);
      expect(mock.listRootsCalls).toBe(1);
    } finally {
      stderrSpy.mockRestore();
    }
  });
});

describe('initProjectRootFromMcp — 启动锁定优先级（边界续）', () => {
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

  it('CLAUDE_PROJECT_DIR 为 undefined（未设置 / delete process.env）时跳过 env 通道', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    delete process.env.CLAUDE_PROJECT_DIR;
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
  });

  it('CLAUDE_PROJECT_DIR 为空字符串 "" 时跳过 env 通道', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = '';
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
  });

  it('CLAUDE_PROJECT_DIR 为超长绝对路径（>1000 chars）且不存在时缓存保持 null', async () => {
    const missing = path.join(os.tmpdir(), `${'x'.repeat(1001)}`);
    process.env.CLAUDE_PROJECT_DIR = missing;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
  });

  it('CLAUDE_PROJECT_DIR 含空格 / emoji 且路径存在时缓存严格等于该路径', async () => {
    const parent = makeTempDir('project-root-special-');
    temps.push(parent);
    const special = path.join(parent, 'my project 🚀');
    fs.mkdirSync(special);
    process.env.CLAUDE_PROJECT_DIR = special;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(special);
  });

  it('URI 含 %20 时解码后锁定路径与本地目录 path.resolve 一致', async () => {
    const parent = makeTempDir('project-root-space-');
    temps.push(parent);
    const spaced = path.join(parent, 'my project');
    fs.mkdirSync(spaced);
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: fileUriFor(spaced) }] },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(path.resolve(spaced));
  });

  it('客户端未声明 roots capability（getClientCapabilities() 无 roots）时不调用 listRoots（listRootsCalls === 0），继续 WORKSPACE', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: {},
      listRootsResult: { roots: [{ uri: fileUriFor(makeTempDir()) }] },
    });

    await initProjectRootFromMcp(mock.server);

    expect(mock.listRootsCalls).toBe(0);
    expect(getMcpCachedProjectRoot()).toBe(dir);
  });
});

// ===========================================================================
// initProjectRootFromMcp — 严格失败
// ===========================================================================

describe('initProjectRootFromMcp — 严格失败', () => {
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

  it('listRoots 返回 2+ 可用根时缓存为 null，且不等于任一 roots[i]；stderr 精确含 refusing to guess 与 usable 数量数字 (AC-2)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [{ uri: fileUriFor(a) }, { uri: fileUriFor(b) }],
      },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
    expect(getMcpCachedProjectRoot()).not.toBe(a);
    expect(getMcpCachedProjectRoot()).not.toBe(b);
    const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(stderrText).toContain('refusing to guess');
    expect(stderrText).toContain('2');
    stderrSpy.mockRestore();
  });

  it('WORKSPACE_FOLDER_PATHS 解析后 >1 个可用路径时缓存为 null，且不等于 split 后的 [0] (AC-2)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.WORKSPACE_FOLDER_PATHS = `${a},${b}`;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
    expect(getMcpCachedProjectRoot()).not.toBe(a);
  });

  it('三通道均失败时缓存为 null，且 getMcpCachedProjectRoot() !== process.cwd() (AC-2)', async () => {
    const cwd = process.cwd();
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
    expect(getMcpCachedProjectRoot()).not.toBe(cwd);
  });

  it('listRoots 返回 { roots: [] } 时缓存保持 null；stderr 含精确串 MCP roots/list returned no usable file roots (AC-2)', async () => {
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [] },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
    const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(stderrText).toContain('MCP roots/list returned no usable file roots');
    stderrSpy.mockRestore();
  });

  it('WORKSPACE_FOLDER_PATHS 仅相对路径段时失败原因含精确前缀 WORKSPACE_FOLDER_PATHS has no usable absolute paths:（非 literal）(AC-2)', async () => {
    process.env.WORKSPACE_FOLDER_PATHS = 'relative/a,relative/b';
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) throw err;
      expect(err.code).not.toBe('literal_env');
      expect(err.message).toContain(
        'WORKSPACE_FOLDER_PATHS has no usable absolute paths: relative/a,relative/b',
      );
    }
  });

  it('WORKSPACE_FOLDER_PATHS 为 undefined（未设置）时该通道失败', async () => {
    delete process.env.WORKSPACE_FOLDER_PATHS;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
  });

  it('WORKSPACE_FOLDER_PATHS 为空字符串 / 仅分隔符时缓存保持 null', async () => {
    process.env.WORKSPACE_FOLDER_PATHS = ',,;;';
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
  });

  it('WORKSPACE_FOLDER_PATHS 含空白 trim 后恰好 1 个可用路径时锁定', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = `  ${dir}  ,  `;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
  });

  it('WORKSPACE_FOLDER_PATHS 为超长字符串（>1000 chars，含或不含分隔符）时不误锁定为唯一可用根', async () => {
    process.env.WORKSPACE_FOLDER_PATHS = `${'a'.repeat(1001)},${'b'.repeat(1001)}`;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
  });

  it('WORKSPACE_FOLDER_PATHS 含特殊字符（\\n / emoji；可夹杂在路径或分隔片段中）时按解析与存在性规则处理，不得静默取 [0]', async () => {
    const parent = makeTempDir('project-root-ws-special-');
    temps.push(parent);
    const a = path.join(parent, 'alpha 🚀');
    const b = path.join(parent, 'beta 🚀');
    fs.mkdirSync(a);
    fs.mkdirSync(b);
    // 夹杂空白/换行分隔片段与 emoji 路径名；两个可用根时不得取 [0]
    process.env.WORKSPACE_FOLDER_PATHS = `\n${a}\n,${b},🚀`;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
  });
});

// ===========================================================================
// initProjectRootFromMcp — 进程内不可变
// ===========================================================================

describe('initProjectRootFromMcp — 进程内不可变', () => {
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

  it('首次锁定后二次 init 即使 env/roots 指向另一存在目录，缓存仍严格等于首次路径 (AC-3)', async () => {
    const first = makeTempDir();
    const second = makeTempDir();
    temps.push(first, second);
    process.env.CLAUDE_PROJECT_DIR = first;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: fileUriFor(second) }] },
    });

    await initProjectRootFromMcp(mock.server);
    expect(getMcpCachedProjectRoot()).toBe(first);

    process.env.CLAUDE_PROJECT_DIR = second;
    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(first);
    expect(getMcpCachedProjectRoot()).not.toBe(second);
  });

  it('已锁定后二次 init 时 listRootsCalls 保持 0（no-op 短路）(AC-3)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: fileUriFor(dir) }] },
    });

    await initProjectRootFromMcp(mock.server);
    expect(getMcpCachedProjectRoot()).toBe(dir);

    delete process.env.CLAUDE_PROJECT_DIR;
    await initProjectRootFromMcp(mock.server);

    expect(mock.listRootsCalls).toBe(0);
    expect(getMcpCachedProjectRoot()).toBe(dir);
  });

  it('首次失败（缓存 null）后二次 init 在新 CLAUDE 可用时可写入；写入后第三次改 env 不再改写 (AC-3)', async () => {
    const first = makeTempDir();
    const second = makeTempDir();
    temps.push(first, second);
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);
    expect(getMcpCachedProjectRoot()).toBeNull();

    process.env.CLAUDE_PROJECT_DIR = first;
    await initProjectRootFromMcp(mock.server);
    expect(getMcpCachedProjectRoot()).toBe(first);

    process.env.CLAUDE_PROJECT_DIR = second;
    await initProjectRootFromMcp(mock.server);
    expect(getMcpCachedProjectRoot()).toBe(first);
    expect(getMcpCachedProjectRoot()).not.toBe(second);
  });
});

// ===========================================================================
// requireLockedProjectRoot
// ===========================================================================

describe('requireLockedProjectRoot', () => {
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

  it('缓存已锁定时返回值 === 缓存路径，且 path.isAbsolute 为 true (AC-2/AC-5)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const { initProjectRootFromMcp, requireLockedProjectRoot, getMcpCachedProjectRoot } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    const locked = requireLockedProjectRoot();
    expect(locked).toBe(getMcpCachedProjectRoot());
    expect(path.isAbsolute(locked)).toBe(true);
  });

  it('缓存为 null 且无特定失败原因时：code === not_locked；message 完整字节级等于固定两段模板 (AC-2)', async () => {
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(() => requireLockedProjectRoot()).toThrow(ProjectRootLockError);
    try {
      requireLockedProjectRoot();
      expect.fail('应抛出 ProjectRootLockError');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.name).toBe('ProjectRootLockError');
      expect(err.code).toBe('not_locked');
      // 三通道失败后 lockFailureReason 已被各通道写入；message 必须含固定前缀与指导文案
      expect(err.message.startsWith('Project root is not locked:')).toBe(true);
      expect(err.message).toContain(
        'Set CLAUDE_PROJECT_DIR to an existing absolute path, or provide exactly one MCP root / WORKSPACE_FOLDER_PATHS entry.',
      );
    }
  });

  it('WORKSPACE 双可用根失败后：code === multi_root；message 含 multi-root ambiguity 或 usable paths；不得为 not_locked (AC-2)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.WORKSPACE_FOLDER_PATHS = `${a};${b}`;
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出 ProjectRootLockError');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.code).toBe('multi_root');
      expect(err.code).not.toBe('not_locked');
      expect(err.message).toMatch(/multi-root ambiguity|usable paths/i);
    }
  });

  it('失败原因含 >1（无空格）时 code === multi_root（证明 >\\s*1 的无空白分支）(AC-2)', async () => {
    // 路径中嵌入 ">1"，且不含 multi / more than one，专杀 >\\s*1 被删或改为要求空白的突变
    process.env.CLAUDE_PROJECT_DIR = '/nonexistent-root-marker->1-end';
    delete process.env.WORKSPACE_FOLDER_PATHS;
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出 ProjectRootLockError');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.code).toBe('multi_root');
      expect(err.message).toContain('>1');
      expect(err.message).not.toMatch(/> 1/);
    }
  });

  it('失败原因含 > 1（带空格）时 code === multi_root（证明 >\\s*1 的 \\s* 量词）(AC-2)', async () => {
    process.env.CLAUDE_PROJECT_DIR = '/nonexistent-root-marker-> 1-end';
    delete process.env.WORKSPACE_FOLDER_PATHS;
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出 ProjectRootLockError');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.code).toBe('multi_root');
      expect(err.message).toContain('> 1');
    }
  });

  it('roots 返回 2+ 可用根后：code === multi_root，message 含 roots/list 或 multi 语义 (AC-2)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [{ uri: fileUriFor(a) }, { uri: fileUriFor(b) }],
      },
    });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出 ProjectRootLockError');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.code).toBe('multi_root');
      expect(err.message).toMatch(/roots\/list|multi/i);
    } finally {
      stderrSpy.mockRestore();
    }
  });

  it('仅 WORKSPACE_FOLDER_PATHS=${workspaceFolder}（无其它通道）失败后：code === literal_env，message 含 ${ 或 literal (AC-2)', async () => {
    process.env.WORKSPACE_FOLDER_PATHS = '${workspaceFolder}';
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出 ProjectRootLockError');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.code).toBe('literal_env');
      expect(err.message).toMatch(/\$\{|literal/i);
    }
  });

  it("仅 CLAUDE_PROJECT_DIR='${workspaceFolder}' 且无 WORKSPACE/roots 时：code === literal_env（证明 env 字面量失败原因被保留到 require）(AC-2)", async () => {
    process.env.CLAUDE_PROJECT_DIR = '${workspaceFolder}';
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出 ProjectRootLockError');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.code).toBe('literal_env');
      expect(err.message).toMatch(/\$\{|literal/i);
    }
  });

  it('仅相对路径 CLAUDE：code === invalid_path；message 含完整片段 is not an absolute path（保留 an ）(AC-2)', async () => {
    process.env.CLAUDE_PROJECT_DIR = 'relative/path';
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出 ProjectRootLockError');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.code).toBe('invalid_path');
      expect(err.message).toContain('is not an absolute path');
    }
  });

  it('仅绝对不存在 CLAUDE：code === invalid_path；message 含 does not exist 与路径原文 (AC-2)', async () => {
    const missing = path.join(os.tmpdir(), `missing-require-${Date.now()}`);
    process.env.CLAUDE_PROJECT_DIR = missing;
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出 ProjectRootLockError');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.code).toBe('invalid_path');
      expect(err.message).toContain('does not exist');
      expect(err.message).toContain(missing);
    }
  });

  it('抛错后再次调用仍抛同一 code 与同一完整 message（失败原因不被清空）', async () => {
    process.env.WORKSPACE_FOLDER_PATHS = '${workspaceFolder}';
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    let firstCode: string | undefined;
    let firstMessage: string | undefined;
    try {
      requireLockedProjectRoot();
    } catch (err) {
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      firstCode = err.code;
      firstMessage = err.message;
    }
    expect(firstCode).toBe('literal_env');

    try {
      requireLockedProjectRoot();
      expect.fail('应再次抛出');
    } catch (err) {
      if (!(err instanceof ProjectRootLockError)) {
        throw err;
      }
      expect(err.code).toBe(firstCode);
      expect(err.message).toBe(firstMessage);
    }
  });
});

// ===========================================================================
// initProjectRootFromMcp — LITERAL_ENV_PATTERN 正则判别
// ===========================================================================

describe('initProjectRootFromMcp — LITERAL_ENV_PATTERN 正则判别', () => {
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

  it("CLAUDE_PROJECT_DIR='${ab}'（花括号内 ≥2 字符）被拒绝为字面量；后续 WORKSPACE 单根可锁定 (AC-1)", async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = '${ab}';
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot, requireLockedProjectRoot } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(dir);
    expect(() => requireLockedProjectRoot()).not.toThrow();
  });

  it("CLAUDE_PROJECT_DIR='${}'（空花括号）不得走 contains unexpanded literal 通道（证明 /\\$\\{[^}]+\\}/ 的 + 量词）(AC-1/AC-2)", async () => {
    // LITERAL_ENV_PATTERN 不匹配 '${}'；失败原因应为 not an absolute path（非 unexpanded literal）
    process.env.CLAUDE_PROJECT_DIR = '${}';
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) throw err;
      expect(err.message).toContain('is not an absolute path');
      expect(err.message).not.toContain('contains unexpanded literal');
    }
  });

  it("WORKSPACE_FOLDER_PATHS='${workspaceFolder}' 单独失败时 code === literal_env，且 message 字节级含 WORKSPACE_FOLDER_PATHS contains unexpanded literal: 与原始 raw 串 (AC-2)", async () => {
    const raw = '${workspaceFolder}';
    process.env.WORKSPACE_FOLDER_PATHS = raw;
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) throw err;
      expect(err.code).toBe('literal_env');
      expect(err.message).toContain(`WORKSPACE_FOLDER_PATHS contains unexpanded literal: ${raw}`);
      expect(err.message).toContain(raw);
    }
  });
});

// ===========================================================================
// initProjectRootFromMcp — fileUri / rootEntry 边界
// ===========================================================================

describe('initProjectRootFromMcp — fileUri / rootEntry 边界', () => {
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

  it('roots 仅含 http://example.com/x 时不得锁定；stderr 含 no usable file roots；缓存 null (AC-2)', async () => {
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: 'http://example.com/x' }] },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
    const stderrText = stderrSpy.mock.calls.map((c) => String(c[0])).join('');
    expect(stderrText).toContain('no usable file roots');
    stderrSpy.mockRestore();
  });

  it('roots 仅含空字符串 URI "" 时不得锁定 (AC-2)', async () => {
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: '' }] },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
  });

  it('roots 含非法/不可用 URI 与 1 个可用绝对路径时锁定该可用路径（跳过坏条目）(AC-1)', async () => {
    const usable = makeTempDir();
    temps.push(usable);
    const missingAbs = path.join(os.tmpdir(), `missing-root-${Date.now()}`);
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [
          { uri: 'http://example.com/x' },
          { uri: '' },
          { uri: 'relative/not/abs' },
          { uri: missingAbs },
          { uri: usable },
        ],
      },
    });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBe(usable);
  });
});

// ===========================================================================
// initProjectRootFromMcp — setLockFailure 保留
// ===========================================================================

describe('initProjectRootFromMcp — setLockFailure 保留', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it("先设 CLAUDE_PROJECT_DIR='${workspaceFolder}'（literal），再经无 roots capability + WORKSPACE unset：requireLocked.code 仍为 literal_env (AC-2)", async () => {
    process.env.CLAUDE_PROJECT_DIR = '${workspaceFolder}';
    delete process.env.WORKSPACE_FOLDER_PATHS;
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) throw err;
      expect(err.code).toBe('literal_env');
    }
  });

  it('先设相对路径 CLAUDE（invalid_path），再经 WORKSPACE unset：code 仍为 invalid_path，message 仍含 not an absolute path (AC-2)', async () => {
    process.env.CLAUDE_PROJECT_DIR = 'relative/only';
    delete process.env.WORKSPACE_FOLDER_PATHS;
    const { initProjectRootFromMcp, requireLockedProjectRoot, ProjectRootLockError } =
      await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    try {
      requireLockedProjectRoot();
      expect.fail('应抛出');
    } catch (err) {
      expect(err).toBeInstanceOf(ProjectRootLockError);
      if (!(err instanceof ProjectRootLockError)) throw err;
      expect(err.code).toBe('invalid_path');
      expect(err.message).toContain('not an absolute path');
    }
  });
});

// ===========================================================================
// getProjectDir — 模块归并后公共 API
// ===========================================================================

describe('getProjectDir — 模块归并后公共 API', () => {
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

  it('缓存已锁定时返回缓存路径，优先于 env (AC-10)', async () => {
    const locked = makeTempDir();
    const other = makeTempDir();
    temps.push(locked, other);
    process.env.CLAUDE_PROJECT_DIR = locked;
    const { initProjectRootFromMcp, getProjectDir } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);
    process.env.CLAUDE_PROJECT_DIR = other;

    expect(getProjectDir()).toBe(locked);
  });

  it('无缓存且可用 CLAUDE_PROJECT_DIR 时返回该路径（CLI 语义）(AC-7/AC-10)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const { getProjectDir, getMcpCachedProjectRoot } = await loadFreshModule();

    expect(getMcpCachedProjectRoot()).toBeNull();
    expect(getProjectDir()).toBe(dir);
  });

  it('无缓存、无 CLAUDE，且 WORKSPACE 恰好 1 个可用路径时返回该路径 (AC-10)', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const { getProjectDir } = await loadFreshModule();

    expect(getProjectDir()).toBe(dir);
  });

  it('无缓存且 env 均不可用时返回 process.cwd()（仅 CLI 辅助；MCP 不得依赖）(AC-7)', async () => {
    const { getProjectDir } = await loadFreshModule();

    expect(getProjectDir()).toBe(process.cwd());
  });

  it('CLAUDE_PROJECT_DIR 为 undefined（未设置）时继续 WORKSPACE/cwd', async () => {
    delete process.env.CLAUDE_PROJECT_DIR;
    const dir = makeTempDir();
    temps.push(dir);
    process.env.WORKSPACE_FOLDER_PATHS = dir;
    const { getProjectDir } = await loadFreshModule();

    expect(getProjectDir()).toBe(dir);
  });

  it('CLAUDE_PROJECT_DIR="" 视为未设置，继续 WORKSPACE/cwd', async () => {
    process.env.CLAUDE_PROJECT_DIR = '';
    const { getProjectDir } = await loadFreshModule();

    expect(getProjectDir()).toBe(process.cwd());
  });

  it('CLAUDE_PROJECT_DIR 为 ${...} 字面量时跳过，返回值不得等于该字面量', async () => {
    process.env.CLAUDE_PROJECT_DIR = '${workspaceFolder}';
    const { getProjectDir } = await loadFreshModule();

    expect(getProjectDir()).toBe(process.cwd());
    expect(getProjectDir()).not.toBe('${workspaceFolder}');
  });

  it('WORKSPACE 双可用路径时 CLI getProjectDir 回退 cwd（不得取 [0]），且与 requireLocked 抛错语义区分 (AC-7/AC-2)', async () => {
    const a = makeTempDir();
    const b = makeTempDir();
    temps.push(a, b);
    process.env.WORKSPACE_FOLDER_PATHS = `${a},${b}`;
    const {
      initProjectRootFromMcp,
      getProjectDir,
      requireLockedProjectRoot,
      ProjectRootLockError,
    } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getProjectDir()).toBe(process.cwd());
    expect(getProjectDir()).not.toBe(a);
    expect(() => requireLockedProjectRoot()).toThrow(ProjectRootLockError);
  });
});

// ===========================================================================
// getMcpCachedProjectRoot
// ===========================================================================

describe('getMcpCachedProjectRoot', () => {
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

  it('锁定后返回非 null 绝对路径', async () => {
    const dir = makeTempDir();
    temps.push(dir);
    process.env.CLAUDE_PROJECT_DIR = dir;
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    const cached = getMcpCachedProjectRoot();
    expect(cached).not.toBeNull();
    expect(path.isAbsolute(cached!)).toBe(true);
    expect(cached).toBe(dir);
  });

  it('三通道均失败后调用不抛异常，返回 null（不得伪装为 process.cwd()）', async () => {
    const { initProjectRootFromMcp, getMcpCachedProjectRoot } = await loadFreshModule();
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(() => getMcpCachedProjectRoot()).not.toThrow();
    expect(getMcpCachedProjectRoot()).toBeNull();
    expect(getMcpCachedProjectRoot()).not.toBe(process.cwd());
  });

  it('未 init / 缓存未写入时返回 null', async () => {
    const { getMcpCachedProjectRoot } = await loadFreshModule();

    expect(getMcpCachedProjectRoot()).toBeNull();
  });
});

// ===========================================================================
// isProjectRootLockError — 跨模块副本鸭类型识别
// ===========================================================================

describe('isProjectRootLockError — 跨模块副本鸭类型识别', () => {
  it('真实 ProjectRootLockError 实例应被识别', async () => {
    const { ProjectRootLockError, isProjectRootLockError } = await loadFreshModule();
    const err = new ProjectRootLockError('not_locked', 'locked? no');

    expect(isProjectRootLockError(err)).toBe(true);
  });

  it('同名 Error + code 字符串（模块副本）应被识别', async () => {
    const { isProjectRootLockError } = await loadFreshModule();
    const err = new Error('Project root is not locked: duck copy');
    err.name = 'ProjectRootLockError';
    Object.defineProperty(err, 'code', { value: 'not_locked' });

    expect(isProjectRootLockError(err)).toBe(true);
  });

  it('普通 Error / 仅改 name 无 code / null / 字符串 / 非 Error 对象 → 均返回 false', async () => {
    const { isProjectRootLockError } = await loadFreshModule();
    const plain = new Error('boom');
    const namedOnly = new Error('x');
    namedOnly.name = 'ProjectRootLockError';

    expect(isProjectRootLockError(plain)).toBe(false);
    expect(isProjectRootLockError(namedOnly)).toBe(false);
    expect(isProjectRootLockError(null)).toBe(false);
    expect(isProjectRootLockError('not_locked')).toBe(false);
    expect(isProjectRootLockError({ name: 'ProjectRootLockError', code: 'not_locked' })).toBe(
      false,
    );
  });

  it("name === 'ProjectRootLockError' 但 code 为非 string（如 number）→ false（证明 typeof code === 'string'）", async () => {
    const { isProjectRootLockError } = await loadFreshModule();
    const err = new Error('x');
    err.name = 'ProjectRootLockError';
    Object.defineProperty(err, 'code', { value: 42 });

    expect(isProjectRootLockError(err)).toBe(false);
  });
});

describe('getProjectDir — 模块归并后公共 API（AC-10 文件删除）', () => {
  it('utils/constant.ts 已删除；源码无 from utils/constant 残留依赖', () => {
    const constantPath = path.resolve(__dirname, '../utils/constant.ts');
    expect(fs.existsSync(constantPath)).toBe(false);
    const srcRoot = path.resolve(__dirname, '..');
    const walk = (dir: string): string[] => {
      const out: string[] = [];
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory() && entry.name !== 'node_modules') {
          out.push(...walk(full));
        } else if (
          entry.isFile() &&
          /\.(ts|js|cjs|mjs)$/.test(entry.name) &&
          !entry.name.includes('.test.')
        ) {
          out.push(full);
        }
      }
      return out;
    };
    for (const file of walk(srcRoot)) {
      const text = fs.readFileSync(file, 'utf-8');
      expect(text).not.toMatch(/from ['"].*utils\/constant['"]/);
      expect(text).not.toMatch(/from ['"]\.\/constant['"]/);
    }
  });
});
