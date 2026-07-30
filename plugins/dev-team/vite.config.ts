import { writeFileSync, cpSync, mkdirSync } from 'node:fs';
import { dirname } from 'node:path';

import { defineConfig, type UserConfig } from 'vite-plus';

const PLUGIN_NAME = 'dev-team';
const SUPPORT_AGENTS = ['cursor' as const, 'claude' as const];
type AgentType = typeof SUPPORT_AGENTS extends Array<infer T> ? T : never;

const NO_OXC_FILES = ['bin/openspec-bundled.js', '*.md'];

export default defineConfig({
  lint: {
    ignorePatterns: NO_OXC_FILES,
    options: {
      typeCheck: true,
      typeAware: true,
      reportUnusedDisableDirectives: 'error',
    },
    rules: {
      'max-lines-per-function': ['error', { max: 50 }],
      'no-duplicate-imports': ['error'],
      'no-unused-vars': 'error',
      'import/no-duplicates': 'error',
      'typescript/consistent-type-imports': ['error', { fixStyle: 'inline-type-imports' }],
      'typescript/non-nullable-type-assertion-style': 'error',
      'typescript/no-explicit-any': 'error',
      'typescript/no-non-null-assertion': 'error',
      'typescript/no-unnecessary-type-assertion': 'error',
      'typescript/no-unsafe-type-assertion': 'error',
      'unicorn/no-abusive-eslint-disable': 'error',
    },
    overrides: [
      {
        files: ['*.test.ts', '*.spec.ts'],
        rules: {
          'max-lines-per-function': 'off',
          'typescript/no-non-null-assertion': 'off',
          'typescript/no-unsafe-type-assertion': 'off',
        },
      },
    ],
  },
  fmt: {
    ignorePatterns: NO_OXC_FILES,
    singleQuote: true,
    sortImports: true,
  },
  pack: [...makeBinPackConfigs()],
  test: {
    include: ['bin/src/**/*.test.ts', 'bin/__tests__/**/*.test.ts'],
    silent: 'passed-only',
  },
});

function makeBinPackConfigs(): Extract<UserConfig['pack'], Array<unknown>> {
  return SUPPORT_AGENTS.flatMap((agent) => {
    return [
      {
        ...toSingleFilePack(agent, 'mcp'),
        clean: [getOutputPathByAgent(agent)],
        hooks: {
          'build:done': async () => {
            await generateConfigJsonSchema(agent);
            copyAgentPluginFiles(agent);
            await generateAgentPluginConfig(agent);
          },
        },
      },
      toSingleFilePack(agent, 'cli'),
      toSingleFilePack(agent, 'hooks'),
    ];
  });
}

function toSingleFilePack(
  agent: AgentType,
  entry: string,
): NonNullable<Exclude<UserConfig['pack'], Array<unknown>>> {
  return {
    name: entry,
    platform: 'node',
    entry: `bin/src/${entry}.ts`,
    outputOptions: {
      file: getOutputPathByAgent(agent, `bin/${PLUGIN_NAME}-${entry}.cjs`),
      format: 'cjs',
      minify: true,
      sourcemap: true,
      cleanDir: false,
      codeSplitting: false,
    },
    deps: {
      alwaysBundle: [/.*/],
    },
    dts: false,
  };
}

async function generateConfigJsonSchema(agent: AgentType): Promise<void> {
  const { configSchema } = await import('./bin/src/schemas');
  const jsonSchemaContent = JSON.stringify(configSchema.toJSONSchema(), null, 2);
  outputToFile(agent, 'bin/dev-team-config.schema.json', jsonSchemaContent);
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

async function generateAgentPluginConfig(agent: AgentType): Promise<void> {
  const { version } = await import('./package.json');
  const pluginConfigContent = JSON.stringify(
    {
      name: PLUGIN_NAME,
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

function getOutputPathByAgent(agent: AgentType, filePath: string = ''): string {
  return `../../${agent}-plugins/${PLUGIN_NAME}/${filePath}`;
}
