import { writeFileSync } from 'node:fs';

import { defineConfig } from 'vite-plus';

const MCP_OUTPUT_FILE_NAME = 'dev-team-mcp.cjs';
const CLI_OUTPUT_FILE_NAME = 'dev-team-cli.cjs';
const HOOKS_OUTPUT_FILE_NAME = 'dev-team-hooks.cjs';
const OUTPUT_FILE_NAMES = [
  MCP_OUTPUT_FILE_NAME,
  CLI_OUTPUT_FILE_NAME,
  HOOKS_OUTPUT_FILE_NAME,
  'dev-team-config.schema.json',
  'openspec-bundled.js',
];

export default defineConfig({
  lint: {
    ignorePatterns: OUTPUT_FILE_NAMES,
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
        },
      },
    ],
  },
  fmt: {
    ignorePatterns: OUTPUT_FILE_NAMES,
    singleQuote: true,
    sortImports: true,
  },
  pack: [
    {
      name: 'mcp',
      platform: 'node',
      entry: 'src/mcp.ts',
      outputOptions: {
        file: MCP_OUTPUT_FILE_NAME,
        format: 'cjs',
        minify: true,
        sourcemap: true,
        cleanDir: false,
      },
      deps: {
        alwaysBundle: [/.*/],
      },
      dts: false,
      hooks: {
        'build:done': async () => {
          const { configSchema } = await import('./src/schemas');
          const jsonSchemaContent = JSON.stringify(configSchema.toJSONSchema(), null, 2);
          writeFileSync('./dev-team-config.schema.json', jsonSchemaContent);
        },
      },
    },
    {
      name: 'cli',
      platform: 'node',
      entry: 'src/cli.ts',
      outputOptions: {
        file: CLI_OUTPUT_FILE_NAME,
        format: 'cjs',
        minify: true,
        sourcemap: true,
        cleanDir: false,
      },
      deps: {
        alwaysBundle: [/.*/],
      },
      dts: false,
    },
    {
      name: 'hooks',
      platform: 'node',
      entry: 'src/hooks.ts',
      outputOptions: {
        file: HOOKS_OUTPUT_FILE_NAME,
        format: 'cjs',
        minify: true,
        sourcemap: true,
        cleanDir: false,
      },
      deps: {
        alwaysBundle: [/.*/],
      },
      dts: false,
    },
  ],
  test: {
    include: ['src/**/*.test.ts', './__tests__/**/*.test.ts'],
  },
});
