import { defineConfig, type UserConfig } from 'vite-plus';

import { assembleAll, generateConfigJsonSchema } from './build';

const NO_OXC_FILES = ['bin/openspec-bundled.js', 'bin/dev-team-config.schema.json', '*.md'];
const BIN_ENTRIES = ['mcp', 'cli', 'hooks'] as const;

type PackConfig = NonNullable<Exclude<UserConfig['pack'], Array<unknown>>>;

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
        files: ['*.test.ts', '*.test.mjs'],
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
  pack: makeStagingPacks(),
  test: {
    include: [
      'bin/src/**/*.test.ts',
      'bin/__tests__/**/*.test.ts',
      'build/**/*.test.ts',
      'build/**/*.test.mjs',
    ],
    setupFiles: ['bin/__tests__/test-setup.ts'],
    silent: 'passed-only',
  },
});

function makeStagingPacks(): PackConfig[] {
  let pending = BIN_ENTRIES.length;
  const onDone = async (): Promise<void> => {
    pending -= 1;
    if (pending > 0) return;
    generateConfigJsonSchema();
    await assembleAll();
  };

  return BIN_ENTRIES.map((id, index) => ({
    name: id,
    platform: 'node',
    entry: `bin/src/${id}.ts`,
    outputOptions: {
      file: `.pack-staging/bin/${id}.cjs`,
      format: 'cjs',
      minify: true,
      sourcemap: true,
      cleanDir: index === 0,
      codeSplitting: false,
    },
    deps: {
      alwaysBundle: [/.*/],
    },
    dts: false,
    hooks: {
      'build:done': onDone,
    },
  }));
}
