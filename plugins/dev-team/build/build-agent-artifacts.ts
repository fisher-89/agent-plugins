import { writeFileSync, cpSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

export type AgentType = 'claude' | 'cursor';

const envs: Record<AgentType, { pluginRoot: string }> = {
  claude: {
    pluginRoot: '${CLAUDE_PLUGIN_ROOT}',
  },
  cursor: {
    pluginRoot: '.',
  },
};

export async function buildAgentArtifacts(agent: AgentType): Promise<void> {
  copyAgentPluginFiles(agent);
  buildMcpConfig(agent);
  await buildAgentPluginConfig(agent);
}

export function getOutputPathByAgent(agent: AgentType, filePath: string = ''): string {
  return `../../${agent}-plugins/dev-team/${filePath}`;
}

function copyAgentPluginFiles(agent: AgentType): void {
  const staticFiles = [
    'agents/',
    'hooks/',
    'skills/',
    'templates/',
    'utils/',
    '.mcp.json',
    'bin/openspec',
    'bin/openspec-bundled.js',
    'bin/openspec.cmd',
  ];
  for (const staticFile of staticFiles) {
    cpSync(staticFile, getOutputPathByAgent(agent, staticFile), { recursive: true });
  }
}

function buildMcpConfig(agent: AgentType): void {
  const mcpConfigContent = JSON.stringify(
    {
      mcpServers: {
        'dev-team': {
          type: 'stdio',
          command: 'node',
          args: [`${envs[agent].pluginRoot}/bin/dev-team-mcp.cjs`],
        },
      },
    },
    null,
    2,
  );
  const mcpConfigFilePath: Record<AgentType, string> = {
    claude: '.mcp.json',
    cursor: 'mcp.json',
  };
  outputToFile(agent, mcpConfigFilePath[agent], mcpConfigContent);
}

async function buildAgentPluginConfig(agent: AgentType): Promise<void> {
  const { version } = await import('../package.json');
  const pluginConfigContent = JSON.stringify(
    {
      name: 'dev-team',
      description: 'A plugin for enhancing development workflow with OpenSpec integration',
      version,
      bin: './bin',
      openspecVersion: '1.2.0',
    },
    null,
    2,
  );
  const pluginConfigFilePath: Record<AgentType, string> = {
    claude: '.claude-plugin/plugin.json',
    cursor: '.cursor-plugin/plugin.json',
  };
  outputToFile(agent, pluginConfigFilePath[agent], pluginConfigContent);
}

function outputToFile(agent: AgentType, filePath: string, content: string): void {
  const outputFilePath = getOutputPathByAgent(agent, filePath);
  const outputDirPath = dirname(outputFilePath);
  mkdirSync(outputDirPath, { recursive: true });
  writeFileSync(outputFilePath, content, 'utf-8');
}
