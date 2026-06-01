import { defineConfig } from 'vite-plus';

const OUTPUT_FILE_NAME = 'dev-team-mcp.cjs';

export default defineConfig({
  lint: {
    ignorePatterns: [OUTPUT_FILE_NAME, 'openspec-bundled.js'],
    options: {
      typeCheck: true,
      typeAware: true,
    },
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
});
