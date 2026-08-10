export type AgentType = 'claude' | 'cursor';
export type ProductEnvKey = 'claude' | 'cursor' | 'cursorHome';
export type ProductLayout = 'plugin' | 'home-image';
export type PathReplacePhase = 'build' | 'install';

export interface ProductEnv {
  /** 适配agent */
  agent: AgentType;
  /** 产物名称 */
  key: ProductEnvKey;
  /** 产物目录 */
  outDir: string;
  /** agent唤起mcp工具的前缀 */
  mcpToolPrefix: string;
  /** 文件或工具名称的前缀，避免名称重复 */
  namePrefix: string;
  /** 插件前缀 */
  pluginPrefix: string;
  /** 产物形态 */
  layout: ProductLayout;
  /** hooks文件相对路径 */
  hooksFilePath: string;
  /** mcp文件相对路径 */
  mcpFilePath: string;
  pathReplacePhase: PathReplacePhase;
  contentRoot: string;
  runtimeRoot: string;
}

export const PRODUCT_ENV_KEYS = [
  'claude',
  'cursor',
  'cursorHome',
] as const satisfies readonly ProductEnvKey[];

const ENV_TABLE: Record<ProductEnvKey, ProductEnv> = {
  claude: {
    agent: 'claude',
    key: 'claude',
    outDir: '../../claude-plugins/dev-team',
    contentRoot: '${CLAUDE_PLUGIN_ROOT}',
    runtimeRoot: '${CLAUDE_PLUGIN_ROOT}',
    mcpToolPrefix: 'mcp__plugin_dev-team_dev-team__',
    namePrefix: '',
    pluginPrefix: 'dev-team:',
    layout: 'plugin',
    hooksFilePath: 'hooks/hooks.json',
    mcpFilePath: '.mcp.json',
    pathReplacePhase: 'build',
  },
  cursor: {
    agent: 'cursor',
    key: 'cursor',
    outDir: '../../cursor-plugins/dev-team',
    contentRoot: '.',
    runtimeRoot: '.',
    mcpToolPrefix: 'mcp__plugin_dev-team_dev-team__',
    namePrefix: '',
    pluginPrefix: 'dev-team:',
    layout: 'plugin',
    hooksFilePath: 'hooks/hooks.json',
    mcpFilePath: 'mcp.json',
    pathReplacePhase: 'build',
  },
  cursorHome: {
    agent: 'cursor',
    key: 'cursorHome',
    outDir: '../../cursor-home-image/dev-team',
    contentRoot: '__DEV_TEAM_ROOT__',
    runtimeRoot: '__DEV_TEAM_RUNTIME_ROOT__',
    mcpToolPrefix: 'mcp__user-dev-team_mcp__',
    namePrefix: 'dev-team_',
    pluginPrefix: '',
    layout: 'home-image',
    hooksFilePath: 'hooks.json',
    mcpFilePath: 'mcp.json',
    pathReplacePhase: 'install',
  },
};

export function getEnv(key: ProductEnvKey): ProductEnv {
  const env = ENV_TABLE[key];
  if (!env) {
    throw new Error(`Unknown product env key: ${String(key)}`);
  }
  return env;
}
