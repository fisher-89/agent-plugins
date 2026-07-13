// ---------------------------------------------------------------------------
// Git utilities
//
// Provides git operations via simple-git for use in test execution and
// other workflow commands.
// ---------------------------------------------------------------------------

import { simpleGit } from 'simple-git';

// ---------------------------------------------------------------------------
// getGitDiffFiles
// ---------------------------------------------------------------------------

/**
 * Get the list of files changed in the working tree relative to HEAD.
 *
 * Uses `git diff HEAD --name-only` to discover all files that differ from
 * the latest commit (includes both staged and unstaged changes).
 *
 * @param projectRoot - Absolute path to the project root (git worktree)
 * @returns Normalized (forward-slash) file paths relative to project root.
 *          Returns an empty array on error — callers should treat this
 *          as "no diff files" rather than failing.
 */
export async function getGitDiffFiles(projectRoot: string): Promise<string[]> {
  const git = simpleGit(projectRoot);
  let untrackedFiles: string[] = [];
  try {
    if (!(await git.checkIsRepo())) {
      console.error('错误: 当前目录不是一个 Git 仓库');
      return [];
    }

    const status = await git.status();
    untrackedFiles = status.not_added;

    if (untrackedFiles.length > 0) {
      await git.add(['-N', ...untrackedFiles]);
    }

    const { files } = await git.diffSummary(['HEAD', '--name-only']);

    return files.map(({ file }) => file);
  } finally {
    if (untrackedFiles.length > 0) {
      await git.reset(untrackedFiles);
    }
  }
}
