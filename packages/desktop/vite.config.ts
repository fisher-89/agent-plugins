import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite-plus';

// dev server 端口与 src-tauri/tauri.conf.json 的 build.devUrl 对齐。
// defineConfig 取自 vite-plus（config.json 登记的套件框架）：`vp dev` /
// `vp build` 与 `vp test` 共用本文件；test 节配置组件/hook 测试所需的 jsdom 环境。
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  envPrefix: ['VITE_', 'TAURI_'],
  build: {
    outDir: 'dist',
    target: 'es2022',
  },
  lint: {
    ignorePatterns: ['src-tauri/**'],
    options: {
      typeCheck: true,
      typeAware: true,
      reportUnusedDisableDirectives: 'error',
    },
    rules: {
      'max-lines-per-function': ['error', { max: 50, skipBlankLines: true, skipComments: true }],
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
        files: ['*.test.ts', '*.test.tsx'],
        rules: {
          'max-lines-per-function': 'off',
          'typescript/no-non-null-assertion': 'off',
          'typescript/no-unsafe-type-assertion': 'off',
        },
      },
    ],
  },
  fmt: {
    ignorePatterns: ['src-tauri/**'],
    singleQuote: true,
    sortImports: true,
  },
  test: {
    environment: 'jsdom',
    globals: true,
  },
});
