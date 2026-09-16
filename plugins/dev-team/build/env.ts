type AgentType = 'claude' | 'cursor' | 'zcode';
export type ProductEnvKey = 'claude' | 'cursor' | 'cursorHome' | 'zcode';
type ProductLayout = 'plugin' | 'home-image';

export interface ProductEnv extends ModelEnv, ToolEnv {
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
  /** 插件配置文件路径，（文件通常叫plugin.json) */
  pluginFilePath: string;
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

const PLUGIN_NAME = 'dev-team';

export const PRODUCT_ENV_KEYS: ProductEnvKey[] = ['claude', 'cursor', 'cursorHome', 'zcode'];

const ENV_TABLE: Record<ProductEnvKey, ProductEnv> = {
  claude: {
    agent: 'claude',
    layout: 'plugin',
    pluginPrefix: `${PLUGIN_NAME}:`,
    namePrefix: '',
    mcpToolPrefix: `mcp__plugin_${PLUGIN_NAME}_dev-team__`,
    outDir: '../../claude-plugins/dev-team',
    pluginFilePath: '.claude-plugin/plugin.json',
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
  },
  cursor: {
    agent: 'cursor',
    layout: 'plugin',
    pluginPrefix: `${PLUGIN_NAME}:`,
    namePrefix: '',
    mcpToolPrefix: `mcp__plugin_${PLUGIN_NAME}_dev-team__`,
    outDir: '../../cursor-plugins/dev-team',
    pluginFilePath: '.cursor-plugin/plugin.json',
    hooksFilePath: 'hooks/hooks.json',
    mcpFilePath: 'mcp.json',
    pluginRoot: '.',
    modelHigh: 'grok-4.6',
    modelFast: 'composer-2.5',
    toolWrite: 'Write',
    toolEdit: 'StrReplace',
    toolBash: 'Shell',
    toolCmd: 'Shell',
    toolAskUser: 'AskQuestion',
  },
  cursorHome: {
    agent: 'cursor',
    layout: 'home-image',
    pluginPrefix: '',
    namePrefix: `${PLUGIN_NAME}_`,
    mcpToolPrefix: `mcp__user-${PLUGIN_NAME}_mcp__`,
    outDir: '../../cursor-home-image/dev-team',
    pluginFilePath: '',
    hooksFilePath: 'hooks.json',
    mcpFilePath: 'mcp.json',
    pluginRoot: '__INSTALL_PLUGIN_ROOT__',
    modelHigh: 'grok-4.6',
    modelFast: 'composer-2.5',
    toolWrite: 'Write',
    toolEdit: 'StrReplace',
    toolBash: 'Shell',
    toolCmd: 'Shell',
    toolAskUser: 'AskQuestion',
  },
  zcode: {
    agent: 'zcode',
    layout: 'plugin',
    pluginPrefix: `${PLUGIN_NAME}:`,
    namePrefix: '',
    mcpToolPrefix: `mcp__plugin_${PLUGIN_NAME}_dev-team__`,
    outDir: '../../zcode-plugins/dev-team',
    pluginFilePath: '.zcode-plugin/plugin.json',
    hooksFilePath: 'hooks/hooks.json',
    mcpFilePath: '.mcp.json',
    pluginRoot: '${CLAUDE_PLUGIN_ROOT}',
    modelHigh: 'glm-5.3',
    modelFast: 'glm-5.3-flash',
    toolWrite: 'Write',
    toolEdit: 'Edit',
    toolBash: 'Bash',
    toolCmd: '',
    toolAskUser: 'AskUserQuestion',
  }
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
