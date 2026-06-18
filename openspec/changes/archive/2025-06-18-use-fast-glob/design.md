# 设计: use-fast-glob

> **变更**: use-fast-glob
> **日期**: 2026-06-18
> **基于**: proposal.md, specs/glob-matching/spec.md, specs/test-execution-diagnostics/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `glob.ts` 共享模块 | 提供 `matchGlob`（单路径匹配）与 `scanProjectFiles`（项目文件扫描）两个可复用纯函数/I/O 函数 | `plugins/dev-team/bin/src/lib/glob.ts` | `fast-glob`、`picomatch`（fast-glob 传递依赖） | TypeScript |
| `matchGlob()` | 将路径分隔符归一化后，使用 picomatch 判断单条路径是否匹配 glob 模式；对无通配符模式扩展为目录前缀语义 | `plugins/dev-team/bin/src/lib/glob.ts` | `picomatch` | TypeScript |
| `scanProjectFiles()` | 使用 `fast-glob.sync()` 在 `rootDir` 下枚举匹配任一 pattern 的文件，内置排除目录，返回排序去重后的绝对路径列表 | `plugins/dev-team/bin/src/lib/glob.ts` | `fast-glob` | TypeScript |
| `runTestDetectFrameworks()` 重构 | 删除私有 `globToRegex`/`matchGlob`/`collectFiles`；文件匹配与自动扫描改为调用 `lib/glob.ts`；保留 `deriveWorkingDirectory`、`normalizeFrameworks`、`generateScript` 不变 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `lib/glob.ts`、`readConfig`、`runTestGetFrameworkConfig` | TypeScript |
| `glob.test.ts` 单元测试 | 覆盖 glob 边界场景（`**`、`{}`、无通配符目录前缀、Windows 路径归一化、排除目录） | `plugins/dev-team/bin/src/lib/glob.test.ts` | `glob.ts`、`vite-plus/test` | TypeScript / Vitest |
| 插件版本递增 | 按项目规则在代码变更后升级插件版本号 | `plugins/dev-team/.claude-plugin/plugin.json` | 无 | JSON |

### 组件图

```
test_detect_frameworks (MCP tool)
  └── runTestDetectFrameworks()
        ├── readConfig() / normalizeFrameworks()     [不变]
        ├── deriveWorkingDirectory()                 [不变]
        ├── generateScript()                         [不变]
        ├── runTestGetFrameworkConfig()              [不变]
        │
        ├── [files 已提供] 逐文件首匹配
        │     └── matchGlob(file, mapping.glob)      ← lib/glob.ts
        │
        └── [files 省略] 自动扫描
              └── scanProjectFiles(root, mappingGlobs) ← lib/glob.ts
                    └── fast-glob.sync(patterns, { ignore, absolute, onlyFiles })

lib/glob.ts
  ├── matchGlob(filePath, pattern)
  │     ├── normalizeSlashes(filePath, pattern)
  │     ├── [无通配符] 目录前缀匹配 (path === pattern || path.startsWith(pattern + '/'))
  │     └── [有通配符] picomatch.isMatch(path, pattern)
  │
  └── scanProjectFiles(rootDir, patterns, options?)
        ├── [patterns 为空] → []
        ├── expandPatternsForScan()  // 无通配符模式追加 /**
        └── fast-glob.sync(expanded, { cwd, absolute, onlyFiles, ignore })
```

---

## 数据流

### 流程描述

#### 1. 单文件匹配路径（`files` 参数已提供）

1. `runTestDetectFrameworks` 将相对路径解析为绝对路径（逻辑不变）。
2. 对每个文件，按 `normalizeFrameworks` 返回的 `{glob, framework}` 数组顺序遍历。
3. 调用 `matchGlob(file, mapping.glob)` 判断是否匹配。
4. 首个匹配成功的 mapping 决定框架归属；无匹配则标记 `"unknown"`（逻辑不变）。

#### 2. 自动扫描路径（`files` 参数省略）

**替换前**：
1. `collectFiles(projectRoot)` 递归收集项目下全部文件（排除 `node_modules` 等目录）。
2. 对每个文件逐一执行 `matchGlob` 与所有 mapping 比较。

**替换后**：
1. 从 mappings 提取所有 glob 字符串组成 `mappingGlobs`。
2. 调用 `scanProjectFiles(projectRoot, mappingGlobs)` 直接获取匹配任一 glob 的文件绝对路径列表。
3. 对返回的每个文件执行首匹配规则（逻辑不变）。

**语义差异说明**：自动扫描不再将不匹配任何 glob 的文件纳入 `detected` 数组（替换前这些文件会以 `"unknown"` 出现）。这与 AC-7「匹配文件集合一致」一致——框架归属正确的文件集合保持不变，同时消除全量遍历的性能开销。

#### 3. `matchGlob` 内部流程

1. 将 `filePath` 与 `pattern` 中的 `\` 归一化为 `/`。
2. 检测 pattern 是否含通配符（`*`、`?`、`{`、`[`）。
3. **无通配符**：执行目录前缀匹配——`filePath === pattern` 或 `filePath.startsWith(pattern + '/')`。
4. **有通配符**：调用 `picomatch.isMatch(filePath, pattern)` 返回结果。

#### 4. `scanProjectFiles` 内部流程

1. 若 `patterns` 为空数组，立即返回 `[]`。
2. 将每个 pattern 通过 `expandPatternForScan()` 扩展：无通配符时追加 `/**`，确保目录级 glob（如 `plugins/dev-team/bin`）能扫描到子目录文件。
3. 调用 `fast-glob.sync(expandedPatterns, options)`：
   - `cwd: rootDir`
   - `absolute: true`
   - `onlyFiles: true`
   - `ignore: DEFAULT_IGNORE_PATTERNS`（见下方常量）
4. 对结果去重（`Set`）后按字典序排序返回。

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `FrameworkMapping`（不变） | `glob: string`、`framework: TestFrameworks` | 由 `normalizeFrameworks()` 从 `config.test.framework` + `config.test.overrides` 生成 | 不持久化（运行时从 config.json 读取） |
| `DetectedFile`（不变） | `file: string`、`framework: string` | 每个输入/扫描文件对应一条检测记录 | 不持久化 |
| `PlanEntry`（不变） | `directory`、`framework`、`coverage_cmd`、`coverage_format`、`coverage_output`、`coverage_artifacts`、`coverage_cleanup`、`script` | 由 mappings 顺序生成，与 glob 匹配无关 | 不持久化 |
| `ScanProjectFilesOptions`（新增） | `ignore?: string[]` — 可选额外排除 glob 模式 | 扩展 `scanProjectFiles` 默认排除列表 | 不持久化 |
| `DEFAULT_IGNORE_DIRS`（新增常量） | `node_modules`、`.git`、`.claude`、`dist`、`build`、`target`、`.vp`、`coverage`、`.nyc_output` | 转换为 fast-glob `ignore` 模式 `**/<dir>/**` | 模块级常量 |

---

## 路由/API 设计

### MCP 工具: `test_detect_frameworks`（实现替换，契约不变）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_detect_frameworks` | 检测文件所属测试框架并生成执行 plan | `{ files?: string[], project_root?: string }` | `{ detected: {file, framework}[], frameworks: string[], plan: PlanEntry[] }` | 无（本地 MCP） |

**不变项**：
- 输入/输出 Zod schema（`test-detect-frameworks.schema.ts`）不修改
- 首匹配规则、框架默认 glob、`plan` 生成顺序与内容不变
- `deriveWorkingDirectory` 推导规则不变

**变更项（内部实现）**：
- 文件匹配：`matchGlob` 从 `lib/glob.ts` 导入（picomatch 实现）
- 自动扫描：`scanProjectFiles(projectRoot, mappingGlobs)` 替代 `collectFiles` + 逐文件 regex

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **引入 `fast-glob` 作为生产依赖，抽取 `lib/glob.ts` 共享模块** | 消除约 80 行手写 glob-to-regex 代码；`fast-glob` 生态成熟，边界情况（`**/` 零段路径、`{}` 花括号）经过广泛验证；模块可被未来 CLI 命令复用 | **备选：继续使用手写 `globToRegex`**。被拒绝，维护成本高且已有 `**/` 历史 bug。**备选：使用已有的 `tinyglobby`（vite 传递依赖）**。被拒绝，`tinyglobby` 未在 `package.json` 中显式声明，API 面向 vite 构建场景，且 proposal 已选定 `fast-glob` |
| D2 | **`matchGlob` 使用 `picomatch.isMatch`，不直接调用 `fast-glob` 做单路径匹配** | `fast-glob` 官方文档推荐单路径匹配使用 picomatch；picomatch 随 `fast-glob` 安装，无需额外依赖；纯函数、无 I/O、性能优于 `fg.sync` 包装 | **备选：`fg.sync(pattern, { cwd: dirname(file) })` 判断结果是否含 file**。被拒绝，每次匹配触发文件系统扫描，语义脆弱且性能差 |
| D3 | **无通配符 pattern 采用目录前缀语义** | 项目配置中 `plugins/dev-team/bin` 表示「该目录及其子目录下的文件」，spec 明确要求 `matchGlob(".../foo.test.ts", "plugins/dev-team/bin")` 返回 `true`；手写 `globToRegex` 的纯字面量 `^pattern$` 无法满足此语义 | **备选：要求配置方始终写 `plugins/dev-team/bin/**`**。被拒绝，破坏现有配置兼容性与 spec 验收场景。**备选：无通配符仅精确匹配**。被拒绝，与 per-directory test execution 用例冲突 |
| D4 | **`scanProjectFiles` 扫描前将无通配符 pattern 扩展为 `pattern/**`** | `fast-glob.sync('plugins/dev-team/bin')` 仅匹配目录节点本身（非文件），追加 `/**` 才能枚举子目录文件；与 D3 的匹配语义保持一致 | **备选：扫描与匹配使用不同语义**。被拒绝，导致 `scanProjectFiles` 结果与 `matchGlob` 不一致 |
| D5 | **自动扫描改为 `scanProjectFiles` 仅返回匹配 glob 的文件** | 利用 fast-glob 原生模式过滤，避免全量 `collectFiles` + 逐文件 regex；AC-7 要求「匹配文件集合一致」而非「detected 全集一致」 | **备选：保留 `collectFiles` 全量收集 + `matchGlob` 过滤**。被拒绝，未解决性能问题，与 spec 中「SHALL NOT 使用自定义递归目录遍历」矛盾 |
| D6 | **排除目录通过 fast-glob `ignore: ['**/<dir>/**']` 实现** | 与现有 `collectFiles` 的 `excludedDirs` Set 语义等价（任意层级目录名匹配即跳过）；fast-glob 原生支持 `ignore` 选项 | **备选：扫描后手动过滤路径**。被拒绝，冗余且可能在超大 `node_modules` 目录上浪费 I/O。**备选：使用 `!` negation patterns**。被拒绝，`ignore` 选项更直观且是 fast-glob 推荐方式 |
| D7 | **`deriveWorkingDirectory` 保留在 `test-detect-frameworks.ts`，不迁入 `glob.ts`** | 该函数为纯字符串前缀推导，与 glob 匹配语义无关；proposal 明确不在替换范围内；避免模块职责膨胀 | **备选：合并到 `glob.ts` 作为 glob 辅助函数**。被拒绝，函数不依赖 glob 库，放在一起增加耦合 |
| D8 | **不添加 `@types/fast-glob`** | `fast-glob` v3+ 自带 TypeScript 类型声明；减少 devDependencies 维护 | **备选：显式添加 `@types/fast-glob`**。被拒绝，包已内置类型，重复声明可能版本冲突 |

---

## 依赖

### 运行时依赖

- `fast-glob` — `scanProjectFiles` 使用 `fg.sync()` 进行目录遍历与模式过滤；生产依赖，打包进 `dev-team-mcp.cjs`（`vite.config.ts` 中 `alwaysBundle: [/.*/]` 已配置全量打包）

### 传递依赖（无需显式声明）

- `picomatch` — `matchGlob` 使用 `picomatch.isMatch()` 进行单路径 glob 匹配；随 `fast-glob` 安装

### 构建/测试依赖

- `vite-plus/test` — 已有测试框架，用于 `glob.test.ts` 与现有 `test-detect-frameworks.test.ts` 回归
- `vite-plus` pack — 构建 `dev-team-mcp.cjs`，需验证 `fast-glob` 打包后 bundle 体积可接受

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `picomatch` 与手写 `globToRegex` 在 `{}`、`**` 等模式上语义差异 | 框架检测错误，测试在错误目录执行 | 中 | 以现有 `test-detect-frameworks.test.ts` 为回归基线；新增 `glob.test.ts` 覆盖 AC-3/AC-4 边界场景；实现前对比关键 pattern 的匹配快照 |
| 无通配符目录前缀语义与 picomatch 默认行为不一致 | `plugins/dev-team/bin` 类配置匹配失败 | 中 | D3/D4 显式实现前缀匹配与扫描扩展；单元测试覆盖无通配符场景 |
| 自动扫描语义变更（不再返回 `"unknown"` 非匹配文件） | 依赖 `detected` 含全量文件的下游逻辑行为变化 | 低 | spec 与 AC-7 明确仅要求匹配文件集合一致；现有测试不依赖 auto-scan 返回 unknown 非测试文件 |
| `fast-glob` 增加 `dev-team-mcp.cjs` 打包体积 | CLI 启动略慢 | 低 | `fast-glob` 体积较小；构建后对比 bundle 大小；`alwaysBundle` 已存在，无额外配置变更 |
| `scanProjectFiles` 排除目录行为与 `collectFiles` 不一致 | 扫描到 `node_modules` 内测试文件或遗漏有效文件 | 低 | `DEFAULT_IGNORE_DIRS` 与现有 `excludedDirs` 完全对齐；`glob.test.ts` 验证 `node_modules` 排除 |
| 半迁移状态（仅替换 `matchGlob` 未替换 `collectFiles`） | 自动扫描仍低效或行为不一致 | 低 | 提案明确要求同时替换；AC-5/AC-7 验证扫描行为；tasks 分阶段强制先完成 `glob.ts` 再重构命令 |

---

## 迁移步骤

1. 在 `plugins/dev-team/bin/package.json` 添加 `fast-glob` 依赖，执行 `pnpm install` 更新 lockfile。
2. 新建 `plugins/dev-team/bin/src/lib/glob.ts`，实现 `matchGlob` 与 `scanProjectFiles`。
3. 新建 `plugins/dev-team/bin/src/lib/glob.test.ts`，覆盖 glob 边界场景。
4. 重构 `test-detect-frameworks.ts`：删除 `globToRegex`、`matchGlob`、`collectFiles`；导入 `lib/glob.ts`；自动扫描改为 `scanProjectFiles`。
5. 运行 `pnpm test`（`vp test`）确保全部单元测试通过。
6. 运行 `pnpm build`（`vp pack`）验证打包成功且 bundle 体积可接受。
7. 递增 `plugins/dev-team/.claude-plugin/plugin.json` 版本号（`2.6.20` → `2.6.21`）。

无配置迁移、无 schema 变更、无 MCP 契约变更。

---

## 待决问题

- 未来是否有其他 CLI 命令（如 `coverage-calculator`）需要 glob 匹配？若有，可直接复用 `lib/glob.ts`，无需重复引入 glob 库。
- `scanProjectFiles` 的 `ScanProjectFilesOptions` 是否在首个消费者之外需要更多选项（如 `followSymlinks`）？当前无需求，保持最小接口。
