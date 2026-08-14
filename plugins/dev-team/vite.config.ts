import { defineConfig, type UserConfig } from 'vite-plus';

import { assembleAll, generateConfigJsonSchema } from './build';

const NO_OXC_FILES = ['bin/openspec-bundled.js', 'bin/dev-team-config.schema.json', '*.md'];
const BIN_ENTRIES = ['mcp', 'cli', 'hooks'] as const;

type PackConfig = NonNullable<Exclude<UserConfig['pack'], Array<unknown>>>;

interface StagingPackSpec {
  name: string;
  entry: string;
  file: string;
  format: 'cjs' | 'esm';
  banner?: string;
}

const STAGING_PACKS: StagingPackSpec[] = [
  ...BIN_ENTRIES.map<StagingPackSpec>((id) => ({
    name: id,
    entry: `bin/src/${id}.ts`,
    file: `bin/${id}.cjs`,
    format: 'cjs',
  })),
  {
    name: 'home-install',
    entry: 'home-install.ts',
    file: 'install.mjs',
    format: 'esm',
    banner: '#!/usr/bin/env node\n',
  },
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
        files: ['*.test.ts'],
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
    include: ['bin/src/**/*.test.ts', 'bin/__tests__/**/*.test.ts', 'build/**/*.test.ts'],
    setupFiles: ['bin/__tests__/test-setup.ts'],
    silent: 'passed-only',
  },
});

function makeStagingPacks(): PackConfig[] {
  let pending = STAGING_PACKS.length;
  const onDone = async (): Promise<void> => {
    pending -= 1;
    if (pending > 0) return;
    generateConfigJsonSchema();
    await assembleAll();
  };

  return STAGING_PACKS.map((spec) => ({
    name: spec.name,
    platform: 'node',
    entry: spec.entry,
    outputOptions: {
      dir: '.pack-staging',
      entryFileNames: spec.file,
      format: spec.format,
      banner: spec.banner,
      minify: true,
      sourcemap: false,
      codeSplitting: false,
    },
    deps: {
      alwaysBundle: [/.*/],
      onlyBundle: false,
    },
    dts: false,
    hooks: {
      'build:done': onDone,
    },
  }));
}
