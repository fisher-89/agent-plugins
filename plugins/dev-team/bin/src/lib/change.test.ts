/**
 * 单元测试: getChangeDir 路径拼接
 *
 * 覆盖 MCP 缓存已设置时间接受益于 getProjectDir 的路径解析（AC-1~AC-3）。
 *
 * @see openspec/changes/mcp-project-root-lock/test-design.md
 * @see openspec/changes/mcp-project-root-lock/design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';
import { pathToFileURL } from 'node:url';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// ===========================================================================
// getChangeDir — MCP 缓存已设置时路径拼接
// ===========================================================================

describe('getChangeDir — MCP 缓存已设置时路径拼接', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  let tempRoot: string | undefined;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
    vi.resetModules();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    if (tempRoot) {
      fs.rmSync(tempRoot, { recursive: true, force: true });
      tempRoot = undefined;
    }
  });

  it('MCP 缓存为 /workspace/project 时 getChangeDir 应拼接 openspec/changes/<name> (AC-1~AC-3)', async () => {
    tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'change-mcp-cache-'));
    const { initProjectRootFromMcp } = await import('./project-root');
    const { getChangeDir } = await import('./change');
    const server = {
      getClientCapabilities: () => ({ roots: { listChanged: false } }),
      listRoots: async () => ({ roots: [{ uri: pathToFileURL(tempRoot!).href }] }),
    };

    await initProjectRootFromMcp(server);

    expect(getChangeDir('my-change')).toBe(
      path.resolve(tempRoot, 'openspec', 'changes', 'my-change'),
    );
  });
});

// ===========================================================================
// getChangeDir — 无 MCP 缓存时回退
// ===========================================================================

describe('getChangeDir — 无 MCP 缓存时回退', () => {
  let savedEnv: ReturnType<typeof saveEnv>;
  let tempRoot: string | undefined;

  beforeEach(() => {
    savedEnv = saveEnv();
    clearProjectEnv();
    vi.resetModules();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
    if (tempRoot) {
      fs.rmSync(tempRoot, { recursive: true, force: true });
      tempRoot = undefined;
    }
  });

  it('无 MCP 缓存、设置 CLAUDE_PROJECT_DIR 时 getChangeDir 基于 env 根目录拼接', async () => {
    tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'change-env-root-'));
    process.env.CLAUDE_PROJECT_DIR = tempRoot;

    const { getChangeDir } = await import('./change');

    expect(getChangeDir('my-change')).toBe(
      path.resolve(tempRoot, 'openspec', 'changes', 'my-change'),
    );
  });
});
