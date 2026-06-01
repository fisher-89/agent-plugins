import * as path from 'path';

/**
 * Resolve the root directory for a change.
 * Returns the absolute path to `openspec/changes/<change-name>`.
 */
function resolveChangeDir(changeName: string): string {
  const projectRoot = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  if (!projectRoot) {
    throw new Error(
      `无法获取工程目录,process.env.CLAUDE_PROJECT_DIR=${process.env.CLAUDE_PROJECT_DIR}`,
    );
  }
  return path.resolve(projectRoot, 'openspec', 'changes', changeName);
}

/**
 * Return the change directory for a change.
 * Equivalent to `<projectRoot>/openspec/changes/<changeName>`.
 */
export function getChangeDir(changeName: string): string {
  return resolveChangeDir(changeName);
}
