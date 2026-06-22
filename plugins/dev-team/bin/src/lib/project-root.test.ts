/**
 * 单元测试: MCP roots/list 项目根解析
 *
 * 覆盖 AC-1、AC-4~AC-6 及 fileUriToPath URI 转换场景。
 *
 * @see openspec/changes/use-mcp-roots-list/test-design.md
 * @see openspec/changes/use-mcp-roots-list/design.md
 */

import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { getProjectDir } from '../utils/constant';
import {
  fileUriToPath,
  getMcpCachedProjectRoot,
  initProjectRootFromMcp,
  type McpServerLike,
  refreshProjectRootFromMcp,
  resetMcpProjectRootCacheForTests,
} from './project-root';

// ---------------------------------------------------------------------------
// Simple mock matching the narrow McpServerLike interface
// ---------------------------------------------------------------------------

interface MockServerOptions {
  capabilities?: { roots?: { listChanged?: boolean } };
  listRootsResult?: { roots: { uri: string; name?: string }[] };
  listRootsError?: Error;
}

function createMockServer(options: MockServerOptions = {}) {
  let listRootsCalls = 0;

  const server: McpServerLike = {
    getClientCapabilities: () =>
      options.capabilities !== undefined ? options.capabilities : { roots: { listChanged: false } },
    listRoots: async () => {
      listRootsCalls += 1;
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

// ---------------------------------------------------------------------------
// Env 隔离 — getProjectDir 断言用
// ---------------------------------------------------------------------------

const ENV_KEYS = ['CLAUDE_PROJECT_DIR'] as const;

function saveEnv(): Record<(typeof ENV_KEYS)[number], string | undefined> {
  return {
    CLAUDE_PROJECT_DIR: process.env.CLAUDE_PROJECT_DIR,
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

function resetCache(): void {
  resetMcpProjectRootCacheForTests();
}

// ===========================================================================
// fileUriToPath — Windows / Unix URI 转换 (AC-1)
// ===========================================================================

describe('fileUriToPath — Windows URI 转换', () => {
  beforeEach(() => {
    resetCache();
  });

  it('file:///D:/Projects/wps-claude-plugin 应转为平台本地绝对路径 (AC-1)', () => {
    const result = fileUriToPath('file:///D:/Projects/wps-claude-plugin');
    expect(result).toBe(path.resolve('D:/Projects/wps-claude-plugin'));
  });

  it('尾随斜杠 file:///D:/Projects/wps-claude-plugin/ 应规范化正确', () => {
    const withoutSlash = fileUriToPath('file:///D:/Projects/wps-claude-plugin');
    const withSlash = fileUriToPath('file:///D:/Projects/wps-claude-plugin/');
    expect(withSlash).toBe(withoutSlash);
  });
});

describe('fileUriToPath — Unix URI 转换', () => {
  beforeEach(() => {
    resetCache();
  });

  it('file:///home/user/projects/my-app 应转为 /home/user/projects/my-app', () => {
    const result = fileUriToPath('file:///home/user/projects/my-app');
    expect(result).toBe(path.resolve('/home/user/projects/my-app'));
  });
});

describe('fileUriToPath — 无效输入', () => {
  beforeEach(() => {
    resetCache();
  });

  it('空字符串 "" 应抛错', () => {
    expect(() => fileUriToPath('')).toThrow();
  });

  it('非 file:// scheme（如 http://example.com）应抛错', () => {
    expect(() => fileUriToPath('http://example.com')).toThrow();
  });

  it('URI 含 URL 编码字符（如 %20）应解码为正确本地路径', () => {
    const result = fileUriToPath('file:///D:/Projects/my%20project');
    expect(result).toBe(path.resolve('D:/Projects/my project'));
  });
});

// ===========================================================================
// initProjectRootFromMcp — roots 可用时缓存首个 root (AC-1)
// ===========================================================================

describe('initProjectRootFromMcp — roots 可用时缓存首个 root', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    delete process.env.CLAUDE_PROJECT_DIR;
    resetCache();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('listRoots 返回单条 Windows file:// root 后 getProjectDir 应为本地绝对路径 (AC-1)', async () => {
    const uri = 'file:///D:/Projects/wps-claude-plugin';
    const expected = path.resolve('D:/Projects/wps-claude-plugin');
    const { server } = createMockServer({
      capabilities: { roots: { listChanged: false } },
      listRootsResult: { roots: [{ uri, name: 'wps-claude-plugin' }] },
    });

    await initProjectRootFromMcp(server);

    expect(getMcpCachedProjectRoot()).toBe(expected);
    expect(getProjectDir()).toBe(expected);
  });

  it('多条 root 时仅使用 roots[0].uri（D3 多 root 策略）', async () => {
    const firstUri = 'file:///D:/Projects/first-root';
    const secondUri = 'file:///D:/Projects/second-root';
    const { server } = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [{ uri: firstUri }, { uri: secondUri }],
      },
    });

    await initProjectRootFromMcp(server);

    expect(getMcpCachedProjectRoot()).toBe(path.resolve('D:/Projects/first-root'));
  });
});

// ===========================================================================
// initProjectRootFromMcp — 无 roots capability / listRoots 失败 (AC-4)
// ===========================================================================

describe('initProjectRootFromMcp — 无 roots capability', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    resetCache();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('getClientCapabilities()?.roots 为 undefined 时不调用 listRoots，缓存保持 null (AC-4)', async () => {
    const mock = createMockServer({ capabilities: {} });

    await initProjectRootFromMcp(mock.server);

    expect(getMcpCachedProjectRoot()).toBeNull();
    expect(mock.listRootsCalls).toBe(0);
  });
});

describe('initProjectRootFromMcp — listRoots 抛错', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    process.env.CLAUDE_PROJECT_DIR = '/fallback/from-env';
    resetCache();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('listRoots reject/throw 时缓存保持 null，不阻断初始化 (AC-4)', async () => {
    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    const { server } = createMockServer({
      capabilities: { roots: {} },
      listRootsError: new Error('listRoots failed'),
    });

    await expect(initProjectRootFromMcp(server)).resolves.toBeUndefined();

    expect(getMcpCachedProjectRoot()).toBeNull();
    expect(getProjectDir()).toBe('/fallback/from-env');
    stderrSpy.mockRestore();
  });
});

// ===========================================================================
// initProjectRootFromMcp — 空 roots 数组 (AC-5)
// ===========================================================================

describe('initProjectRootFromMcp — 空 roots 数组', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    process.env.CLAUDE_PROJECT_DIR = '/fallback/cursor';
    resetCache();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('listRoots 返回 { roots: [] } 时缓存保持 null (AC-5)', async () => {
    const { server } = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [] },
    });

    await initProjectRootFromMcp(server);

    expect(getMcpCachedProjectRoot()).toBeNull();
    expect(getProjectDir()).toBe('/fallback/cursor');
  });
});

// ===========================================================================
// refreshProjectRootFromMcp — 缓存更新 (AC-6, D6)
// ===========================================================================

describe('refreshProjectRootFromMcp — 缓存更新', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    delete process.env.CLAUDE_PROJECT_DIR;
    resetCache();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('refresh 后 listRoots 返回新 URI 时 getProjectDir 应返回新路径 (AC-6)', async () => {
    const oldUri = 'file:///D:/Projects/old-workspace';
    const newUri = 'file:///D:/Projects/new-workspace';

    const mock = createMockServer({
      capabilities: { roots: { listChanged: true } },
      listRootsResult: { roots: [{ uri: oldUri }] },
    });

    await initProjectRootFromMcp(mock.server);
    expect(getMcpCachedProjectRoot()).toBe(path.resolve('D:/Projects/old-workspace'));

    mock.server.listRoots = async () => ({ roots: [{ uri: newUri }] });
    await refreshProjectRootFromMcp(mock.server);

    expect(getProjectDir()).toBe(path.resolve('D:/Projects/new-workspace'));
  });
});

describe('refreshProjectRootFromMcp — 刷新失败保留旧缓存', () => {
  beforeEach(() => {
    resetCache();
  });

  it('已有有效缓存时 listRoots 失败，getProjectDir 仍返回旧路径 (AC-6 / D6)', async () => {
    const oldPath = path.resolve('D:/Projects/stale-but-valid');
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: `file:///${oldPath.replace(/\\/g, '/')}` }] },
    });

    await initProjectRootFromMcp(mock.server);
    expect(getMcpCachedProjectRoot()).toBe(oldPath);

    const stderrSpy = vi.spyOn(process.stderr, 'write').mockImplementation(() => true);
    mock.server.listRoots = async () => {
      throw new Error('refresh failed');
    };

    await expect(refreshProjectRootFromMcp(mock.server)).resolves.toBeUndefined();
    expect(getProjectDir()).toBe(oldPath);
    stderrSpy.mockRestore();
  });
});

describe('refreshProjectRootFromMcp — 刷新返回空 roots', () => {
  beforeEach(() => {
    resetCache();
  });

  it('已有有效缓存时 refresh 返回 { roots: [] }，保留旧缓存 (D6)', async () => {
    const oldPath = path.resolve('D:/Projects/keep-me');
    const mock = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: { roots: [{ uri: `file:///${oldPath.replace(/\\/g, '/')}` }] },
    });

    await initProjectRootFromMcp(mock.server);
    expect(getMcpCachedProjectRoot()).toBe(oldPath);

    mock.server.listRoots = async () => ({ roots: [] });

    await refreshProjectRootFromMcp(mock.server);
    expect(getProjectDir()).toBe(oldPath);
  });
});

// ===========================================================================
// resetMcpProjectRootCacheForTests — 测试隔离 (D8)
// ===========================================================================

describe('resetMcpProjectRootCacheForTests — 测试隔离', () => {
  it('reset 后 getMcpCachedProjectRoot 应为 null (D8)', async () => {
    const { server } = createMockServer({
      capabilities: { roots: {} },
      listRootsResult: {
        roots: [{ uri: 'file:///D:/Projects/wps-claude-plugin' }],
      },
    });

    await initProjectRootFromMcp(server);
    expect(getMcpCachedProjectRoot()).not.toBeNull();

    resetMcpProjectRootCacheForTests();
    expect(getMcpCachedProjectRoot()).toBeNull();
  });
});
