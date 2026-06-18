import { getMcpCachedProjectRoot } from '../lib/project-root';

const mcpProjectDir = getMcpCachedProjectRoot();
const claudeProjectDir = process.env.CLAUDE_PROJECT_DIR;
const cursorProjectDir = process.env.WORKSPACE_FOLDER_PATHS?.split(';').filter(Boolean)[0];
const cwd = process.cwd();

export function getProjectDir(): string {
  return mcpProjectDir || claudeProjectDir || cursorProjectDir || cwd;
}
