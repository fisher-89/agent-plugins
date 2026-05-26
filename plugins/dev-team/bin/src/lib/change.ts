import * as path from "path";

/**
 * Resolve the root directory for a change.
 * Returns the absolute path to `openspec/changes/<change-name>`.
 */
export function resolveChangeDir(changeName: string): string {
  // Bundle is at plugins/dev-team/bin/dev-team-bundle.cjs.
  // Project root is three levels up: bin -> dev-team -> plugins -> <project-root>
  const binDir = path.resolve(__dirname, "..");
  const pluginDir = path.resolve(binDir, "..");
  const projectRoot = path.resolve(pluginDir, "..");
  return path.resolve(projectRoot, "openspec", "changes", changeName);
}

/**
 * Return the phases subdirectory for a change.
 * Equivalent to `<changeDir>/phases`.
 */
export function getPhasesDir(changeName: string): string {
  return path.resolve(resolveChangeDir(changeName), "phases");
}
