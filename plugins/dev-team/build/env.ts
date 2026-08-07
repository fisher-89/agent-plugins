export type ProductEnvKey = 'claude' | 'cursor' | 'cursorHome';
export type HooksProfile = 'claudeNested' | 'cursorNative';
export type ProductLayout = 'plugin' | 'home-image';
export type PathReplacePhase = 'build' | 'install';

export interface ProductEnv {
  key: ProductEnvKey;
  outDir: string;
  contentRoot: string;
  runtimeRoot: string;
  mcpToolPrefix: string;
  namePrefix: string;
  agentRefPrefix: string;
  skillSlashPrefix: string;
  layout: ProductLayout;
  hooksProfile: HooksProfile;
  mcpOut: string;
  pathReplacePhase: PathReplacePhase;
}

export const PRODUCT_ENV_KEYS = [
  'claude',
  'cursor',
  'cursorHome',
] as const satisfies readonly ProductEnvKey[];

const ENV_TABLE: Record<ProductEnvKey, ProductEnv> = {
  claude: {
    key: 'claude',
    outDir: '../../claude-plugins/dev-team',
    contentRoot: '${CLAUDE_PLUGIN_ROOT}',
    runtimeRoot: '${CLAUDE_PLUGIN_ROOT}',
    mcpToolPrefix: 'mcp__plugin_dev-team_dev-team__',
    namePrefix: '',
    agentRefPrefix: 'dev-team:',
    skillSlashPrefix: '/dev-team:',
    layout: 'plugin',
    hooksProfile: 'claudeNested',
    mcpOut: '.mcp.json',
    pathReplacePhase: 'build',
  },
  cursor: {
    key: 'cursor',
    outDir: '../../cursor-plugins/dev-team',
    contentRoot: '.',
    runtimeRoot: '.',
    mcpToolPrefix: 'mcp__plugin_dev-team_dev-team__',
    namePrefix: '',
    agentRefPrefix: 'dev-team:',
    skillSlashPrefix: '/dev-team:',
    layout: 'plugin',
    hooksProfile: 'claudeNested',
    mcpOut: 'mcp.json',
    pathReplacePhase: 'build',
  },
  cursorHome: {
    key: 'cursorHome',
    outDir: '../../cursor-home-image/dev-team',
    contentRoot: '__DEV_TEAM_ROOT__',
    runtimeRoot: '__DEV_TEAM_RUNTIME_ROOT__',
    mcpToolPrefix: 'mcp__user-dev-team_mcp__',
    namePrefix: 'dev-team_',
    agentRefPrefix: 'dev-team_',
    skillSlashPrefix: '/dev-team_',
    layout: 'home-image',
    hooksProfile: 'cursorNative',
    mcpOut: 'mcp.dev-team.json',
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
