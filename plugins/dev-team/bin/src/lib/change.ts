import * as path from 'path';

import { getProjectDir } from '../lib/project-root';

/**
 * Resolve the root directory for a change.
 * Returns the absolute path to `openspec/changes/<change-name>`.
 */
function resolveChangeDir(changeName: string): string {
  return path.resolve(getProjectDir(), 'openspec', 'changes', changeName);
}

/**
 * Return the change directory for a change.
 * Equivalent to `<projectRoot>/openspec/changes/<changeName>`.
 */
export function getChangeDir(changeName: string): string {
  return resolveChangeDir(changeName);
}
