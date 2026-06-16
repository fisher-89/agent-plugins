export function getProjectDir(): string {
  return process.env.CLAUDE_PROJECT_DIR || process.env.CURSOR_PROJECT_DIR || process.cwd();
}
