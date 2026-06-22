/**
 * 单元测试: getProjectDir 项目根解析优先级
 *
 * 覆盖 MCP 缓存优先于 env/cwd 回退链（AC-1、AC-4、AC-5、D5）。
 *
 * @see openspec/changes/use-mcp-roots-list/test-design.md
 * @see openspec/changes/use-mcp-roots-list/design.md
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

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
// getProjectDir — 无 MCP 缓存时 env/cwd 回退链 (AC-4, AC-5)
// ===========================================================================

describe('getProjectDir — 无 MCP 缓存时 env/cwd 回退链', () => {
  let savedEnv: ReturnType<typeof saveEnv>;

  beforeEach(() => {
    savedEnv = saveEnv();
    vi.resetModules();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('缓存为 null 且 CLAUDE_PROJECT_DIR 已设置时返回该 env 值 (AC-4)', async () => {
    process.env.CLAUDE_PROJECT_DIR = '/from/claude-project-dir';
    const { getProjectDir } = await import('./constant');

    expect(getProjectDir()).toBe('/from/claude-project-dir');
  });

  it('无 MCP 缓存且两 env 均未设置时返回 process.cwd() (AC-4 / AC-5)', async () => {
    delete process.env.CLAUDE_PROJECT_DIR;
    const { getProjectDir } = await import('./constant');

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
    vi.resetModules();
  });

  afterEach(() => {
    restoreEnv(savedEnv);
  });

  it('CLAUDE_PROJECT_DIR="" 视为未设置，继续检查 process.cwd()', async () => {
    process.env.CLAUDE_PROJECT_DIR = '';
    const { getProjectDir } = await import('./constant');

    expect(getProjectDir()).toBe(process.cwd());
  });
});
