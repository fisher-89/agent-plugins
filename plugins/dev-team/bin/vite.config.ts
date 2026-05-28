import { defineConfig } from 'vite-plus';

export default defineConfig({
  resolve: {},
  lint: {
    ignorePatterns: ['dev-team-mcp.cjs', 'openspec-bundled.js'],
  },
  pack: {
    entry: ['src/mcp.ts'],
    platform: 'node',
    format: 'cjs',
    outputOptions: {
      file: 'dev-team-mcp.cjs',
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
