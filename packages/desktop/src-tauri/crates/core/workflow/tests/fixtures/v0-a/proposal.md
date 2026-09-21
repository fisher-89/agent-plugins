# 提案: use-fast-glob

> **变更**: use-fast-glob
> **日期**: 2026-06-18
> **状态**: 草稿

---

## 问题

`plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` 当前包含约 80 行手写的 glob 匹配逻辑（`globToRegex`、`matchGlob`）以及自定义的目录遍历函数 `collectFiles`，用于：

1. **文件到框架归属检测**：将输入文件路径与 `config.json` 中 `test.framework` / `test.overrides` 配置的 glob 模式进行首匹配
2. **自动扫描**：当 `test_detect_frameworks` 未传入 `files` 参数时，递归遍历项目目录并对每个文件逐一执行 glob 匹配

该手写实现存在以下问题：

- **维护成本高**：glob 语法（`**`、`{}`、`*`、`?`、路径分隔符归一化）需自行实现与测试，代码体量大且难以复用
- **历史缺陷**：此前 `**/` 零段路径匹配曾出现 bug（需用 `(?:.+/)?` 修复），说明手写正则转换容易遗漏边界情况
- **性能低效**：自动扫描先收集全部文件再逐个 regex 测试，而非利用成熟 glob 库的原生目录遍历与模式过滤
- **与生态脱节**：`dev-team/bin` 的 `pnpm-lock.yaml` 中已有 `tinyglobby`（vite 传递依赖），但业务代码未使用任何标准 glob 库

`deriveWorkingDirectory` 为纯字符串前缀推导，与 glob 匹配语义无关，不在本次替换范围内。

---

## 提案

在 `plugins/dev-team/bin` 中引入 [`fast-glob`](https://github.com/mrmlnc/fast-glob) npm 包，抽取共享 glob 工具模块，替换 `test-detect-frameworks.ts` 中的手写 glob-to-regex 逻辑：

1. **新增依赖**：在 `plugins/dev-team/bin/package.json` 的 `dependencies` 中添加 `fast-glob`（及 TypeScript 类型声明 `@types/fast-glob` 若需要）

2. **新增共享模块** `plugins/dev-team/bin/src/lib/glob.ts`：
   - `matchGlob(filePath, pattern): boolean` — 使用 `fast-glob` 生态（picomatch）判断单条路径是否匹配 glob 模式；路径分隔符统一归一化为 POSIX `/`
   - `scanProjectFiles(rootDir, patterns, options?): string[]` — 使用 `fast-glob.sync()` 在 `rootDir` 下扫描匹配 `patterns` 中任一模式的文件，返回绝对路径列表；内置与现有 `collectFiles` 一致的排除目录（`node_modules`、`.git`、`dist` 等）

3. **重构 `test-detect-frameworks.ts`**：
   - 删除私有函数 `globToRegex`、`matchGlob`、`collectFiles`
   - 文件匹配与自动扫描改为调用 `lib/glob.ts` 导出函数
   - 保持 `deriveWorkingDirectory`、`normalizeFrameworks`、`generateScript` 及 MCP 工具对外行为不变

4. **测试与版本**：更新/补充单元测试覆盖 glob 边界场景；按项目规则升级 `dev-team/.claude-plugin/plugin.json` 版本号

---

## 能力

### 新增能力

- **glob-matching** — 基于 `fast-glob` 的共享 glob 匹配与项目文件扫描模块（`lib/glob.ts`），为 CLI 命令提供可复用的 `matchGlob` 与 `scanProjectFiles` 函数

### 修改的能力

- **test-execution-diagnostics** — `test_detect_frameworks` 的文件匹配与自动扫描实现改为依赖 `glob-matching` 模块，移除手写 glob-to-regex 逻辑；对外 MCP 契约与检测语义（首匹配、框架默认 glob、plan 生成）保持不变

---

## 变更范围

### 实现以下特性

- `plugins/dev-team/bin/package.json`：添加 `fast-glob` 依赖
- `plugins/dev-team/bin/src/lib/glob.ts`：新建共享 glob 工具模块，导出 `matchGlob` 与 `scanProjectFiles`
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts`：删除 `globToRegex`/`matchGlob`/`collectFiles`，改用 `lib/glob.ts`
- `plugins/dev-team/bin/src/commands/test-detect-frameworks.test.ts`：补充 glob 边界测试；确保现有 AC-4 首匹配、自动扫描、plan 集成测试全部通过
- `dev-team/.claude-plugin/plugin.json`：递增插件版本号

### 不要修改

- `deriveWorkingDirectory` 函数及其推导规则（仍为纯字符串操作，不依赖 glob 库）
- `test_detect_frameworks` MCP 工具的输入/输出 schema 与对外行为语义
- `test-get-framework-config.ts`、`coverage-calculator.ts`、`coverage-parser.ts` 等其他模块
- `openspec/config.json` 的 `test.framework` / `test.overrides` 配置 schema
- `unit-test-executor.md` 等 agent 指南中的工作流步骤

---

## 验收标准

| ID    | 验收条件                                                                                                                                                  | 验证方法                                               |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| AC-1  | `plugins/dev-team/bin/package.json` 包含 `fast-glob` 作为 `dependencies` 条目                                                                             | 检查 `package.json` 与 `pnpm-lock.yaml`                |
| AC-2  | `lib/glob.ts` 导出 `matchGlob` 与 `scanProjectFiles`，且不包含手写 glob-to-regex 转换逻辑                                                                 | 代码审查；`globToRegex` 在 `dev-team/bin` 源码中不存在 |
| AC-3  | `matchGlob` 正确匹配项目使用的 glob 模式：`**/*.{test,spec}.{js,ts,jsx,tsx}`、`**/tests/**/*.rs`、`plugins/dev-team/bin`（无通配符精确路径）、`**/e2e/**` | 单元测试                                               |
| AC-4  | `matchGlob` 对 Windows 反斜杠路径与 POSIX 正斜杠路径产生一致匹配结果                                                                                      | 单元测试：同一路径分别以 `\` 和 `/` 传入               |
| AC-5  | `scanProjectFiles` 自动扫描排除 `node_modules`、`.git`、`dist`、`build`、`target`、`.vp`、`coverage`、`.nyc_output` 目录                                  | 单元测试或集成测试                                     |
| AC-6  | `test_detect_frameworks` 首匹配规则不变：文件同时匹配多个 glob 时返回 `frameworks` 数组中第一个匹配的框架                                                 | 现有 `test-detect-frameworks.test.ts` AC-4 测试通过    |
| AC-7  | `test_detect_frameworks` 无 `files` 参数时自动扫描行为与替换前等价（匹配文件集合一致）                                                                    | 对比测试或现有自动扫描场景测试通过                     |
| AC-8  | `test_detect_frameworks` 的 `detected`、`frameworks`、`plan` 输出结构与 schema 不变                                                                       | 现有 plan 集成测试与 Zod schema 校验通过               |
| AC-9  | `deriveWorkingDirectory` 行为不变                                                                                                                         | 现有 `deriveWorkingDirectory` 单元测试全部通过         |
| AC-10 | `plugins/dev-team/bin` 全部单元测试通过（`vp test`）                                                                                                      | CI / 本地 `pnpm test`                                  |
| AC-11 | 插件版本号已递增                                                                                                                                          | 检查 `dev-team/.claude-plugin/plugin.json`             |

---

## 风险

| 风险                                                                   | 影响                                           | 概率 | 缓解措施                                                                                                                           |
| ---------------------------------------------------------------------- | ---------------------------------------------- | ---- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `fast-glob` 与手写实现在 `{}`、`**` 等模式上的语义差异导致匹配结果变化 | 框架检测错误，测试在错误目录执行               | 中   | 以现有 `test-detect-frameworks.test.ts` 为回归基线；补充 `{}`、`**/`、无通配符精确路径等边界测试；实现前记录替换前后的匹配快照对比 |
| `fast-glob` 增加 `dev-team-mcp.cjs` 打包体积                           | CLI 启动略慢或 bundle 变大                     | 低   | `fast-glob` 体积较小；构建后检查 bundle 大小；必要时在 vite 配置中确认 tree-shaking                                                |
| 自动扫描改用 `fast-glob.sync` 后排除目录行为不一致                     | 扫描到 `node_modules` 内测试文件或遗漏有效文件 | 低   | `scanProjectFiles` 通过 `ignore` 选项复刻现有 `collectFiles` 排除列表；对比扫描结果集                                              |
| 仅替换匹配逻辑但遗漏 `collectFiles` 导致半迁移状态                     | 自动扫描仍低效或行为不一致                     | 低   | 提案明确要求同时替换 `collectFiles`；AC-5/AC-7 验证扫描行为                                                                        |
