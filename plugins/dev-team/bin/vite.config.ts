import { defineConfig } from "vite-plus";

export default defineConfig({
  resolve: {},
  lint: {
    ignorePatterns: ["dev-team-bundle.cjs", "openspec-bundled.js"],
  },
  pack: {
    entry: ["src/index.ts"],
    format: "cjs",
    outputOptions: {
      file: "dev-team-bundle.cjs",
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
