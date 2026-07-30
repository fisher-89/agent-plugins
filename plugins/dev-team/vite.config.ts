import { defineConfig, type UserConfig } from 'vite-plus';

import {
  type AgentType,
  buildAgentArtifacts,
  getOutputPathByAgent,
  generateConfigJsonSchema,
} from './build';

const SUPPORT_AGENTS: AgentType[] = ['cursor', 'claude'];
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
  const packs = SUPPORT_AGENTS.flatMap((agent) => {
    return [
      {
        ...toSingleFilePack(agent, 'mcp'),
        clean: [getOutputPathByAgent(agent)],
        hooks: {
          'build:done': async () => {
            await buildAgentArtifacts(agent);
          },
        },
      },
      toSingleFilePack(agent, 'cli'),
      toSingleFilePack(agent, 'hooks'),
    ];
  });

  packs.at(-1)!.hooks = {
    'build:done': async () => {
      await generateConfigJsonSchema();
    },
  };

  return packs;
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
      file: getOutputPathByAgent(agent, `bin/dev-team-${entry}.cjs`),
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
