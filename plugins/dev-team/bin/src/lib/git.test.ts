/**
 * 单元测试: lib/git.ts — getGitDiffFiles
 *
 * 使用真实 git 仓库进行集成测试，覆盖：
 * - 正常返回变更文件列表
 * - 无变更时返回空数组
 * - 未跟踪文件检测（intent-to-add）
 * - 非 git 仓库优雅降级
 * - 多文件 / 子目录 / 特殊字符文件名场景
 *
 * @see plugins/dev-team/bin/src/lib/git.ts
 */

import { randomBytes } from 'node:crypto';
import { rm, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { simpleGit } from 'simple-git';
import { describe, it, expect, afterAll } from 'vite-plus/test';

import { getGitDiffFiles } from './git';

// ---------------------------------------------------------------------------
// 测试辅助工具
// ---------------------------------------------------------------------------

/** 在系统临时目录下生成唯一的测试用目录路径 */
function tempDir(): string {
  return join(tmpdir(), `git-test-${randomBytes(8).toString('hex')}`);
}

/** 初始化一个空的 git 仓库并配置提交者身份 */
async function initRepo(dir: string): Promise<void> {
  await mkdir(dir, { recursive: true });
  const git = simpleGit(dir);
  await git.init();
  await git.addConfig('user.name', 'Test');
  await git.addConfig('user.email', 'test@test.com');
}

/** 将目录中所有文件 stage 并提交 */
async function commitAll(dir: string, message: string): Promise<void> {
  const git = simpleGit(dir);
  await git.add('.');
  await git.commit(message);
}

// ---------------------------------------------------------------------------
// 清理注册（所有测试结束后统一删除临时目录）
// ---------------------------------------------------------------------------

const cleanupDirs: string[] = [];

afterAll(async () => {
  for (const dir of cleanupDirs) {
    await rm(dir, { recursive: true, force: true }).catch(() => {});
  }
});

/** 登记一个临时目录，以便在测试结束后自动清理 */
function trackCleanup(dir: string): string {
  cleanupDirs.push(dir);
  return dir;
}

// ===========================================================================
// getGitDiffFiles — 正常路径
// ===========================================================================

describe('getGitDiffFiles — 正常路径', () => {
  it('应返回 git diff HEAD --name-only 的变更文件列表', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    await writeFile(join(dir, 'a.ts'), 'initial');
    await commitAll(dir, 'initial commit');
    await writeFile(join(dir, 'a.ts'), 'modified');

    const files = await getGitDiffFiles(dir);

    expect(files).toEqual(['a.ts']);
  });

  it('无变更时应返回空数组', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    await writeFile(join(dir, 'a.ts'), 'content');
    await commitAll(dir, 'initial commit');

    const files = await getGitDiffFiles(dir);

    expect(files).toEqual([]);
  });

  it('应检测未跟踪文件（通过 intent-to-add）', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    await writeFile(join(dir, 'existing.ts'), 'content');
    await commitAll(dir, 'initial commit');
    // 创建未跟踪文件
    await writeFile(join(dir, 'new.ts'), 'new file');

    const files = await getGitDiffFiles(dir);

    expect(files).toContain('new.ts');
  });

  it('函数执行后不应将未跟踪文件留在暂存区', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    await writeFile(join(dir, 'committed.ts'), 'content');
    await commitAll(dir, 'initial commit');
    await writeFile(join(dir, 'untracked.ts'), 'new');

    await getGitDiffFiles(dir);

    // 未跟踪文件不应被留在 git 暂存区
    const git = simpleGit(dir);
    const status = await git.status();
    expect(status.staged).toEqual([]);
    expect(status.not_added).toContain('untracked.ts');
  });
});

// ===========================================================================
// getGitDiffFiles — 错误处理（优雅降级）
// ===========================================================================

describe('getGitDiffFiles — 错误处理', () => {
  it('非 git 仓库时应返回空数组', async () => {
    const dir = trackCleanup(tempDir());
    await mkdir(dir, { recursive: true });

    const files = await getGitDiffFiles(dir);

    expect(files).toEqual([]);
  });

  it('目标目录不存在时 simpleGit 构造会同步抛出异常', async () => {
    const nonExistent = join(tmpdir(), `nonexistent-${randomBytes(8).toString('hex')}`);

    // simpleGit() 在 try 块外调用 → 目录不存在时同步抛出，不返回 Promise
    await expect(() => getGitDiffFiles(nonExistent)).rejects.toThrow();
  });
});

// ===========================================================================
// getGitDiffFiles — 多文件场景
// ===========================================================================

describe('getGitDiffFiles — 多文件场景', () => {
  it('应正确处理大量变更文件', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    const count = 50;
    const names: string[] = [];
    for (let i = 0; i < count; i++) {
      names.push(`src/module${i}.ts`);
    }
    await mkdir(join(dir, 'src'), { recursive: true });
    for (const name of names) {
      await writeFile(join(dir, name), 'initial');
    }
    await commitAll(dir, 'initial commit');
    for (const name of names) {
      await writeFile(join(dir, name), 'modified');
    }

    const files = await getGitDiffFiles(dir);

    expect(files).toHaveLength(count);
    // git diff --name-only 返回的是字典序（字符串排序），而非数字序
    const sortedLexicographically = [...names].sort((a, b) => a.localeCompare(b));
    expect(files).toEqual(sortedLexicographically);
  });

  it('应正确处理包含空格的文件名', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    await mkdir(join(dir, 'my docs'), { recursive: true });
    await writeFile(join(dir, 'my docs', 'readme.md'), 'content');
    await commitAll(dir, 'initial commit');
    await writeFile(join(dir, 'my docs', 'readme.md'), 'modified');

    const files = await getGitDiffFiles(dir);

    expect(files).toContain('my docs/readme.md');
  });

  it('应正确处理包含方括号的文件名', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    await mkdir(join(dir, 'src', '[id]'), { recursive: true });
    await writeFile(join(dir, 'src', '[id]', 'page.tsx'), 'content');
    await commitAll(dir, 'initial commit');
    await writeFile(join(dir, 'src', '[id]', 'page.tsx'), 'modified');

    const files = await getGitDiffFiles(dir);

    expect(files).toContain('src/[id]/page.tsx');
  });
});

// ===========================================================================
// getGitDiffFiles — 混合变更场景
// ===========================================================================

describe('getGitDiffFiles — 混合变更场景', () => {
  it('应同时检测已修改文件和未跟踪文件', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    await writeFile(join(dir, 'modified.ts'), 'initial');
    await commitAll(dir, 'initial commit');
    // 修改已有文件 + 创建未跟踪文件
    await writeFile(join(dir, 'modified.ts'), 'changed');
    await writeFile(join(dir, 'untracked.ts'), 'new');

    const files = await getGitDiffFiles(dir);

    expect(files).toContain('modified.ts');
    expect(files).toContain('untracked.ts');
  });

  it('子目录中的文件变更应正确检测', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    await mkdir(join(dir, 'deep', 'nested'), { recursive: true });
    await writeFile(join(dir, 'deep', 'nested', 'file.ts'), 'initial');
    await commitAll(dir, 'initial commit');
    await writeFile(join(dir, 'deep', 'nested', 'file.ts'), 'changed');

    const files = await getGitDiffFiles(dir);

    expect(files).toContain('deep/nested/file.ts');
  });

  it('多个未跟踪文件混合已有文件变更', async () => {
    const dir = trackCleanup(tempDir());
    await initRepo(dir);
    await writeFile(join(dir, 'keep.ts'), 'initial');
    await commitAll(dir, 'initial commit');
    // 修改已有 + 多个未跟踪
    await writeFile(join(dir, 'keep.ts'), 'changed');
    await writeFile(join(dir, 'new-a.ts'), 'a');
    await writeFile(join(dir, 'new-b.ts'), 'b');

    const files = await getGitDiffFiles(dir);

    expect(files).toContain('keep.ts');
    expect(files).toContain('new-a.ts');
    expect(files).toContain('new-b.ts');
  }, 10000);
});
