import * as path from 'path';

/**
 * Return the change directory for a change.
 * Equivalent to `<projectRoot>/openspec/changes/<changeName>`.
 */
export function getChangeDir(changeName: string, projectRoot: string): string {
  return path.resolve(projectRoot, 'openspec', 'changes', changeName);
}
