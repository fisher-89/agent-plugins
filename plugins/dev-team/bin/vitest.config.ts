import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    globals: true,
    include: ["src/**/*.test.ts", "../../../openspec/changes/**/tests/*.test.ts"],
  },
});
