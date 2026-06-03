import { defineConfig } from 'vite-plus';

const OUTPUT_FILE_NAME = 'dev-team-mcp.cjs';

export default defineConfig({
  lint: {
    ignorePatterns: [OUTPUT_FILE_NAME, 'openspec-bundled.js'],
    options: {
      typeCheck: true,
      typeAware: true,
      reportUnusedDisableDirectives: 'error',
    },
    rules: {
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
      { files: ['*.test.ts', '*.spec.ts'], rules: { 'typescript/no-non-null-assertion': 'off' } },
    ],
  },
  fmt: {
    ignorePatterns: [OUTPUT_FILE_NAME, 'openspec-bundled.js'],
    singleQuote: true,
    sortImports: true,
  },
  pack: {
    entry: ['src/mcp.ts'],
    platform: 'node',
    format: 'cjs',
    outputOptions: {
      file: OUTPUT_FILE_NAME,
    },
    deps: {
      alwaysBundle: [/.*/],
    },
    dts: false,
    minify: true,
    sourcemap: true,
    clean: false,
  },
  test: {
    globals: true,
    include: ['src/**/*.test.ts', '../../../openspec/changes/*/tests/**/*.test.ts'],
  },
});
