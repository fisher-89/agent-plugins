import { getMcpCachedProjectRoot } from '../lib/project-root';

export function getProjectDir(): string {
  return (
    getMcpCachedProjectRoot() ??
    (process.env.CLAUDE_PROJECT_DIR || process.env.PROJECT_DIR || process.cwd())
  );
}
