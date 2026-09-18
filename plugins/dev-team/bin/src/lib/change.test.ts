/**
 * 单元测试: resolveChangeDir 路径拼接
 *
 * 覆盖 call-scoped / CLAUDE_PROJECT_DIR 经 getProjectDir 的路径解析。
 *
 * @see openspec/changes/mcp-workspace-root/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

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
// resolveChangeDir — 无 MCP 缓存时回退
// ===========================================================================

describe('resolveChangeDir — 无 MCP 缓存时回退', () => {
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

  it('无 MCP 缓存、设置 CLAUDE_PROJECT_DIR 时 resolveChangeDir 基于 env 根目录拼接', async () => {
    tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'change-env-root-'));
    process.env.CLAUDE_PROJECT_DIR = tempRoot;

    const { resolveChangeDir } = await import('./change');

    expect(resolveChangeDir('my-change', tempRoot)).toBe(
      path.resolve(tempRoot, 'openspec', 'changes', 'my-change'),
    );
  });
});
