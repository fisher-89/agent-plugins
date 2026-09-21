# 实施任务: use-fast-glob

---

## 阶段 1: 依赖与基础设施

- [x] 在 `plugins/dev-team/bin/package.json` 的 `dependencies` 中添加 `fast-glob`
- [x] 在 `plugins/dev-team/bin` 目录执行 `pnpm install`，更新 `pnpm-lock.yaml`

## 阶段 2: 共享 glob 模块实现

- [x] 新建 `plugins/dev-team/bin/src/lib/glob.ts`，定义 `DEFAULT_IGNORE_DIRS` 常量（`node_modules`、`.git`、`.claude`、`dist`、`build`、`target`、`.vp`、`coverage`、`.nyc_output`）
- [x] 实现内部辅助函数 `normalizeSlashes(value: string): string`，将 `\` 归一化为 `/`
- [x] 实现内部辅助函数 `hasWildcard(pattern: string): boolean`，检测 pattern 是否含 `*`、`?`、`{`、`[` 通配符
- [x] 实现内部辅助函数 `expandPatternForScan(pattern: string): string`，无通配符时返回 `pattern + '/**'`，有通配符时原样返回
- [x] 实现并导出 `matchGlob(filePath: string, pattern: string): boolean`：
  - 归一化 `filePath` 与 `pattern` 的路径分隔符
  - 无通配符 pattern：匹配 `filePath === pattern` 或 `filePath.startsWith(pattern + '/')`
  - 有通配符 pattern：使用 `picomatch.isMatch(filePath, pattern)` 返回结果
  - 纯函数，无文件系统 I/O
- [x] 定义并导出 `ScanProjectFilesOptions` 接口（含可选 `ignore?: string[]` 字段）
- [x] 实现并导出 `scanProjectFiles(rootDir: string, patterns: string[], options?: ScanProjectFilesOptions): string[]`：
  - `patterns` 为空时返回 `[]`
  - 将 patterns 经 `expandPatternForScan` 扩展后传入 `fast-glob.sync()`
  - 配置 `cwd: rootDir`、`absolute: true`、`onlyFiles: true`
  - 配置 `ignore` 为 `DEFAULT_IGNORE_DIRS` 对应的 `**/<dir>/**` 模式，合并 `options.ignore`
  - 结果去重后按字典序排序返回

## 阶段 3: glob 模块单元测试

- [x] 新建 `plugins/dev-team/bin/src/lib/glob.test.ts`
- [x] 测试 `matchGlob` 匹配 vitest 默认 glob：`**/*.{test,spec}.{js,ts,jsx,tsx}`（AC-3）
- [x] 测试 `matchGlob` 匹配 rust 默认 glob：`**/tests/**/*.rs`（AC-3）
- [x] 测试 `matchGlob` 无通配符目录前缀：`matchGlob("plugins/dev-team/bin/src/foo.test.ts", "plugins/dev-team/bin")` 返回 `true`（AC-3）
- [x] 测试 `matchGlob` 不匹配路径返回 `false`（AC-3）
- [x] 测试 `matchGlob` Windows 反斜杠与 POSIX 正斜杠路径产生一致结果（AC-4）
- [x] 测试 `matchGlob` 支持 `**/` 零段路径匹配（如 `**/e2e/**` 匹配 `e2e/test.ts`）
- [x] 测试 `matchGlob` 支持 `{}` 花括号备选列表
- [x] 测试 `scanProjectFiles` 在临时目录中扫描匹配多个 glob 模式的文件（AC-5）
- [x] 测试 `scanProjectFiles` 排除 `node_modules` 目录下的文件（AC-5）
- [x] 测试 `scanProjectFiles` 在 `patterns` 为空时返回 `[]`
- [x] 测试 `scanProjectFiles` 返回绝对路径、去重、字典序排序

## 阶段 4: 重构 test-detect-frameworks

- [x] 在 `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` 中删除私有函数 `globToRegex`、`matchGlob`、`collectFiles`（约第 45–178 行）
- [x] 在 `test-detect-frameworks.ts` 顶部添加 `import { matchGlob, scanProjectFiles } from '../lib/glob'`
- [x] 修改 `runTestDetectFrameworks` 自动扫描分支：将 `filesToCheck = collectFiles(projectRoot)` 替换为 `filesToCheck = scanProjectFiles(projectRoot, mappings.map(m => m.glob))`
- [x] 确认 `deriveWorkingDirectory`、`normalizeFrameworks`、`generateScript` 及 plan 生成逻辑未被修改
- [x] 确认 `dev-team/bin` 源码中不存在 `globToRegex` 标识符（AC-2）

## 阶段 5: 回归测试与版本

- [x] 运行 `plugins/dev-team/bin` 下 `pnpm test`（`vp test`），确保 `test-detect-frameworks.test.ts` 全部通过，包括首匹配（AC-6）、plan 集成（AC-8）、自动扫描场景（AC-7）、`deriveWorkingDirectory` 测试（AC-9）
- [x] 运行 `pnpm build`（`vp pack`），确认 `dev-team-mcp.cjs` 构建成功
- [x] 递增 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（`2.6.20` → `2.6.21`）（AC-11）
