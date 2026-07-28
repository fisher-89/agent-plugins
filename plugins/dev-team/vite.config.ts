import { writeFileSync } from 'node:fs';

import { defineConfig } from 'vite-plus';

const MCP_OUTPUT_FILE_NAME = 'bin/dev-team-mcp.cjs';
const CLI_OUTPUT_FILE_NAME = 'bin/dev-team-cli.cjs';
const HOOKS_OUTPUT_FILE_NAME = 'bin/dev-team-hooks.cjs';
const SCHEMA_OUTPUT_FILE_NAME = 'bin/dev-team-config.schema.json';
const NO_OXC_FILES = [
  MCP_OUTPUT_FILE_NAME,
  CLI_OUTPUT_FILE_NAME,
  HOOKS_OUTPUT_FILE_NAME,
  SCHEMA_OUTPUT_FILE_NAME,
  'bin/openspec-bundled.js',
  '*.md',
];

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
  pack: [
    {
      name: 'mcp',
      platform: 'node',
      entry: 'bin/src/mcp.ts',
      outputOptions: {
        file: MCP_OUTPUT_FILE_NAME,
        format: 'cjs',
        minify: true,
        sourcemap: true,
        cleanDir: false,
        // Keep a single CJS artifact even when mcp.ts lazy-imports archi modules.
        codeSplitting: false,
      },
      deps: {
        alwaysBundle: [/.*/],
      },
      dts: false,
      hooks: {
        'build:done': async () => {
          const { configSchema } = await import('./bin/src/schemas');
          const jsonSchemaContent = JSON.stringify(configSchema.toJSONSchema(), null, 2);
          writeFileSync(SCHEMA_OUTPUT_FILE_NAME, jsonSchemaContent);
        },
      },
    },
    {
      name: 'cli',
      platform: 'node',
      entry: 'bin/src/cli.ts',
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
      entry: 'bin/src/hooks.ts',
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
    include: ['bin/src/**/*.test.ts', 'bin/__tests__/**/*.test.ts'],
    silent: 'passed-only',
  },
});
