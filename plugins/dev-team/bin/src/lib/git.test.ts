/**
 * 单元测试: lib/git.ts — getGitDiffFiles
 *
 * 通过 mock simple-git 覆盖控制流、调用参数与返回值；不使用真实 git / 磁盘 I/O。
 *
 * @see plugins/dev-team/bin/src/lib/git.ts
 */

import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

const { mockGit, mockSimpleGit } = vi.hoisted(() => {
  const mockGit = {
    checkIsRepo: vi.fn(),
    status: vi.fn(),
    add: vi.fn(),
    diffSummary: vi.fn(),
    reset: vi.fn(),
  };
  const mockSimpleGit = vi.fn(() => mockGit);
  return { mockGit, mockSimpleGit };
});

vi.mock('simple-git', () => ({
  simpleGit: mockSimpleGit,
}));

const { getGitDiffFiles } = await import('./git');

const PROJECT_ROOT = '/fake/project';

function resetMocks(): void {
  mockSimpleGit.mockReset().mockImplementation(() => mockGit);
  mockGit.checkIsRepo.mockReset().mockResolvedValue(true);
  mockGit.status.mockReset().mockResolvedValue({ not_added: [] });
  mockGit.add.mockReset().mockResolvedValue(undefined);
  mockGit.diffSummary.mockReset().mockResolvedValue({ files: [] });
  mockGit.reset.mockReset().mockResolvedValue(undefined);
}

describe('getGitDiffFiles', () => {
  beforeEach(() => {
    resetMocks();
  });

  it('非 git 仓库时返回空数组，且不调用 status / diffSummary', async () => {
    mockGit.checkIsRepo.mockResolvedValue(false);
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    const files = await getGitDiffFiles(PROJECT_ROOT);

    expect(files).toEqual([]);
    expect(mockSimpleGit).toHaveBeenCalledWith(PROJECT_ROOT);
    expect(mockGit.checkIsRepo).toHaveBeenCalledOnce();
    expect(mockGit.status).not.toHaveBeenCalled();
    expect(mockGit.diffSummary).not.toHaveBeenCalled();
    expect(mockGit.add).not.toHaveBeenCalled();
    expect(mockGit.reset).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it('无 untracked 时应调用 diffSummary([HEAD, --name-only]) 并映射 file 路径', async () => {
    mockGit.diffSummary.mockResolvedValue({
      files: [{ file: 'a.ts' }, { file: 'b.ts' }],
    });

    const files = await getGitDiffFiles(PROJECT_ROOT);

    expect(files).toEqual(['a.ts', 'b.ts']);
    expect(mockGit.diffSummary).toHaveBeenCalledWith(['HEAD', '--name-only', '--relative']);
    expect(mockGit.add).not.toHaveBeenCalled();
    expect(mockGit.reset).not.toHaveBeenCalled();
  });

  it('无变更时应返回空数组', async () => {
    mockGit.diffSummary.mockResolvedValue({ files: [] });

    const files = await getGitDiffFiles(PROJECT_ROOT);

    expect(files).toEqual([]);
    expect(mockGit.add).not.toHaveBeenCalled();
    expect(mockGit.reset).not.toHaveBeenCalled();
  });

  it('有 not_added 时应 add -N、返回 diff 文件，并在 finally 中 reset', async () => {
    mockGit.status.mockResolvedValue({ not_added: ['new.ts', 'other.ts'] });
    mockGit.diffSummary.mockResolvedValue({
      files: [{ file: 'new.ts' }, { file: 'other.ts' }],
    });

    const files = await getGitDiffFiles(PROJECT_ROOT);

    expect(files).toEqual(['new.ts', 'other.ts']);
    expect(mockGit.add).toHaveBeenCalledWith(['-N', 'new.ts', 'other.ts']);
    expect(mockGit.diffSummary).toHaveBeenCalledWith(['HEAD', '--name-only', '--relative']);
    expect(mockGit.reset).toHaveBeenCalledWith(['new.ts', 'other.ts']);
    expect(mockGit.add.mock.invocationCallOrder[0]).toBeLessThan(
      mockGit.diffSummary.mock.invocationCallOrder[0],
    );
    expect(mockGit.diffSummary.mock.invocationCallOrder[0]).toBeLessThan(
      mockGit.reset.mock.invocationCallOrder[0],
    );
  });

  it('diffSummary 抛错时仍应对 untracked 执行 reset', async () => {
    mockGit.status.mockResolvedValue({ not_added: ['new.ts'] });
    mockGit.diffSummary.mockRejectedValue(new Error('diff failed'));

    await expect(getGitDiffFiles(PROJECT_ROOT)).rejects.toThrow('diff failed');
    expect(mockGit.add).toHaveBeenCalledWith(['-N', 'new.ts']);
    expect(mockGit.reset).toHaveBeenCalledWith(['new.ts']);
  });

  it('simpleGit 同步抛异常时应向外抛出', async () => {
    mockSimpleGit.mockImplementation(() => {
      throw new Error('ENOENT');
    });

    await expect(getGitDiffFiles(PROJECT_ROOT)).rejects.toThrow('ENOENT');
    expect(mockGit.checkIsRepo).not.toHaveBeenCalled();
  });

  it('应保持 diffSummary 返回的多文件顺序', async () => {
    const names = Array.from({ length: 10 }, (_, i) => `src/module${i}.ts`);
    mockGit.diffSummary.mockResolvedValue({
      files: names.map((file) => ({ file })),
    });

    const files = await getGitDiffFiles(PROJECT_ROOT);

    expect(files).toHaveLength(10);
    expect(files).toEqual(names);
  });

  it('应原样透传含空格与方括号的路径', async () => {
    mockGit.diffSummary.mockResolvedValue({
      files: [{ file: 'my docs/readme.md' }, { file: 'src/[id]/page.tsx' }],
    });

    const files = await getGitDiffFiles(PROJECT_ROOT);

    expect(files).toEqual(['my docs/readme.md', 'src/[id]/page.tsx']);
  });
});
