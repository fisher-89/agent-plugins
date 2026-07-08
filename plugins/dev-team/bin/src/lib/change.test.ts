/**
 * 单元测试: getChangeDir 路径拼接
 *
 * 覆盖 MCP 缓存已设置时间接受益于 getProjectDir 的路径解析（AC-1~AC-3）。
 *
 * @see openspec/changes/use-mcp-roots-list/test-design.md
 * @see openspec/changes/use-mcp-roots-list/design.md
 */

import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { getChangeDir } from './change';

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

// ===========================================================================
// getChangeDir — MCP 缓存已设置时路径拼接
// ===========================================================================

describe('getChangeDir — MCP 缓存已设置时路径拼接', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    delete process.env.CLAUDE_PROJECT_DIR;
    vi.resetModules();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('MCP 缓存为 /workspace/project 时 getChangeDir 应拼接 openspec/changes/<name> (AC-1~AC-3)', async () => {
    const { initProjectRootFromMcp } = await import('./project-root');
    const { getChangeDir } = await import('./change');
    const projectRoot = path.resolve('/workspace/project');
    const uri = `file://${projectRoot}`;
    const server = {
      getClientCapabilities: () => ({ roots: { listChanged: false } }),
      listRoots: async () => ({ roots: [{ uri }] }),
    };

    await initProjectRootFromMcp(server);

    expect(getChangeDir('my-change')).toBe(
      path.resolve(projectRoot, 'openspec', 'changes', 'my-change'),
    );
  });
});

// ===========================================================================
// getChangeDir — 无 MCP 缓存时回退
// ===========================================================================

describe('getChangeDir — 无 MCP 缓存时回退', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    vi.resetModules();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('无 MCP 缓存、设置 CLAUDE_PROJECT_DIR 时 getChangeDir 基于 env 根目录拼接', async () => {
    const envRoot = path.resolve('/env/project-root');
    process.env.CLAUDE_PROJECT_DIR = envRoot;

    expect(getChangeDir('my-change')).toBe(
      path.resolve(envRoot, 'openspec', 'changes', 'my-change'),
    );
  });
});
