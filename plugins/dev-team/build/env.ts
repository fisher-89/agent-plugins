type AgentType = 'claude' | 'cursor';
export type ProductEnvKey = 'claude' | 'cursor' | 'cursorHome';
type ProductLayout = 'plugin' | 'home-image';

export interface ProductEnv extends ModelEnv, ToolEnv, SubAgentEnv {
  /** 适配agent */
  agent: AgentType;
  /** 产物形态 */
  layout: ProductLayout;
  /** 插件前缀 */
  pluginPrefix: string;
  /** 文件或工具名称的前缀，避免名称重复 */
  namePrefix: string;
  /** agent唤起mcp工具的前缀 */
  mcpToolPrefix: string;
  /** 产物目录 */
  outDir: string;
  /** hooks文件相对路径 */
  hooksFilePath: string;
  /** mcp文件相对路径 */
  mcpFilePath: string;
  /** 插件根目录 / cursor home目录 */
  pluginRoot: string;
}

interface ModelEnv {
  modelHigh: string;
  modelFast: string;
}

interface ToolEnv {
  toolWrite: string;
  toolEdit: string;
  toolBash: string;
  toolCmd: string;
  toolAskUser: string;
}

interface SubAgentEnv {
  agentGeneralPurpose: string;
}

export const PRODUCT_ENV_KEYS: ProductEnvKey[] = ['claude', 'cursor', 'cursorHome'];

const ENV_TABLE: Record<ProductEnvKey, ProductEnv> = {
  claude: {
    agent: 'claude',
    layout: 'plugin',
    pluginPrefix: 'dev-team:',
    namePrefix: '',
    mcpToolPrefix: 'mcp__plugin_dev-team_dev-team__',
    outDir: '../../claude-plugins/dev-team',
    hooksFilePath: 'hooks/hooks.json',
    mcpFilePath: '.mcp.json',
    pluginRoot: '${CLAUDE_PLUGIN_ROOT}',
    modelHigh: 'opus',
    modelFast: 'sonnet',
    toolWrite: 'Write',
    toolEdit: 'Edit',
    toolBash: 'Bash',
    toolCmd: 'PowerShell',
    toolAskUser: 'AskUserQuestion',
    agentGeneralPurpose: 'general-purpose',
  },
  cursor: {
    agent: 'cursor',
    layout: 'plugin',
    pluginPrefix: 'dev-team:',
    namePrefix: '',
    mcpToolPrefix: 'mcp__plugin_dev-team_dev-team__',
    outDir: '../../cursor-plugins/dev-team',
    hooksFilePath: 'hooks/hooks.json',
    mcpFilePath: 'mcp.json',
    pluginRoot: '.',
    modelHigh: 'inherit',
    modelFast: 'fast',
    toolWrite: 'Write',
    toolEdit: 'StrReplace',
    toolBash: 'Shell',
    toolCmd: 'Shell',
    toolAskUser: 'AskQuestion',
    agentGeneralPurpose: 'generalPurpose',
  },
  cursorHome: {
    agent: 'cursor',
    layout: 'home-image',
    pluginPrefix: '',
    namePrefix: 'dev-team_',
    mcpToolPrefix: 'mcp__user-dev-team_mcp__',
    outDir: '../../cursor-home-image/dev-team',
    hooksFilePath: 'hooks.json',
    mcpFilePath: 'mcp.json',
    pluginRoot: '__INSTALL_PLUGIN_ROOT__',
    modelHigh: 'inherit',
    modelFast: 'fast',
    toolWrite: 'Write',
    toolEdit: 'StrReplace',
    toolBash: 'Shell',
    toolCmd: 'Shell',
    toolAskUser: 'AskQuestion',
    agentGeneralPurpose: 'generalPurpose',
  },
};

export function getEnv(key: ProductEnvKey): ProductEnv {
  const env = ENV_TABLE[key];
  if (!env) {
    throw new Error(`Unknown product env key: ${String(key)}`);
  }
  return env;
}

/** Reject null/undefined/empty env objects missing required ProductEnv fields. */
export function requireProductEnv(env: ProductEnv): void {
  if (env == null || typeof env !== 'object' || !('agent' in env) || env.agent == null) {
    throw new Error('env is required');
  }
}
