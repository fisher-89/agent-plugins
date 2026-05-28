import * as path from "path";

/**
 * Resolve the root directory for a change.
 * Returns the absolute path to `openspec/changes/<change-name>`.
 */
export function resolveChangeDir(changeName: string): string {
  // Project root is three levels up: bin -> dev-team -> plugins -> <project-root>
  const binDir = path.resolve(__dirname, "..");
  const pluginDir = path.resolve(binDir, "..");
  const projectRoot = process.env.CLAUDE_PROJECT_DIR || path.resolve(pluginDir, "..");
  return path.resolve(projectRoot, "openspec", "changes", changeName);
}

/**
 * Return the change directory for a change.
 * Equivalent to `<projectRoot>/openspec/changes/<changeName>`.
 */
export function getChangeDir(changeName: string): string {
  return resolveChangeDir(changeName);
}
