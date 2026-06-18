/**
 * 单元测试: getProjectDir 项目根解析优先级
 *
 * 覆盖 MCP 缓存优先于 env/cwd 回退链（AC-1、AC-4、AC-5、D5）。
 *
 * @see openspec/changes/use-mcp-roots-list/test-design.md
 * @see openspec/changes/use-mcp-roots-list/design.md
 */

import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vite-plus/test';

import { resetMcpProjectRootCacheForTests } from '../lib/project-root';
import { getProjectDir } from './constant';

// ---------------------------------------------------------------------------
// Helpers
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

function resetMcpCache(): void {
  resetMcpProjectRootCacheForTests();
}

interface MockMcpServer {
  getClientCapabilities: () => { roots?: { listChanged?: boolean } };
  listRoots: () => Promise<{ roots: Array<{ uri: string }> }>;
  setNotificationHandler: (method: string, handler: () => void | Promise<void>) => void;
}

function createMockServer(uri: string): MockMcpServer {
  return {
    getClientCapabilities: () => ({ roots: { listChanged: false } }),
    listRoots: async () => ({ roots: [{ uri }] }),
    setNotificationHandler: () => {},
  };
}

// ===========================================================================
// getProjectDir — MCP 缓存优先于 env/cwd (AC-1, D5)
// ===========================================================================

describe('getProjectDir — MCP 缓存优先于 env/cwd', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    process.env.CLAUDE_PROJECT_DIR = '/env/claude-should-lose';
    resetMcpCache();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('MCP 缓存已设置时即使 env 已配置仍返回缓存值 (AC-1 / D5)', async () => {
    const { initProjectRootFromMcp } = await import('../lib/project-root');
    const mcpRoot = path.resolve('D:/Projects/mcp-workspace');
    const uri = `file:///${mcpRoot.replace(/\\/g, '/')}`;
    const server = createMockServer(uri);

    await initProjectRootFromMcp(server as never);

    expect(getProjectDir()).toBe(mcpRoot);
    expect(getProjectDir()).not.toBe('/env/claude-should-lose');
    expect(getProjectDir()).not.toBe('/env/cursor-should-lose');
  });
});

// ===========================================================================
// getProjectDir — 无 MCP 缓存时 env/cwd 回退链 (AC-4, AC-5)
// ===========================================================================

describe('getProjectDir — 无 MCP 缓存时 env/cwd 回退链', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    resetMcpCache();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('缓存为 null 且 CLAUDE_PROJECT_DIR 已设置时返回该 env 值 (AC-4)', () => {
    process.env.CLAUDE_PROJECT_DIR = '/from/claude-project-dir';

    expect(getProjectDir()).toBe('/from/claude-project-dir');
  });

  it('无 MCP 缓存且两 env 均未设置时返回 process.cwd() (AC-4 / AC-5)', () => {
    delete process.env.CLAUDE_PROJECT_DIR;

    expect(getProjectDir()).toBe(process.cwd());
  });
});

// ===========================================================================
// getProjectDir — env 空字符串边界
// ===========================================================================

describe('getProjectDir — env 空字符串', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    resetMcpCache();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('CLAUDE_PROJECT_DIR="" 视为未设置，继续检查 process.cwd()', () => {
    process.env.CLAUDE_PROJECT_DIR = '';

    expect(getProjectDir()).toBe(process.cwd());
  });
});
