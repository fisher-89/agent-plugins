import { getMcpCachedProjectRoot } from '../lib/project-root';

export function getProjectDir(): string {
  return (
    getMcpCachedProjectRoot() ||
    process.env.CLAUDE_PROJECT_DIR ||
    process.env.WORKSPACE_FOLDER_PATHS?.split(';').filter(Boolean)[0] ||
    process.cwd()
  );
}
