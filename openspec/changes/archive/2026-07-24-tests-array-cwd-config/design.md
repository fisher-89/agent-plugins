# 设计: tests-array-cwd-config

> **变更**: tests-array-cwd-config
> **日期**: 2026-07-22

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| config-defaults | 导出覆盖率 / 变异阈值 schema 常量，供 Zod `prefault` 引用 | `plugins/dev-team/bin/src/schemas/config/defaults.ts` | 无 | TypeScript |
| config-schema | 顶层 `tests[]` suite Zod schema；校验 `root` 禁通配符；suite 级 `coverage`/`mutation` 默认值 | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | zod/v4, `defaults.ts` | TypeScript |
| config-json-schema | JSON Schema 镜像 `tests[]` suite 形状 | `plugins/dev-team/bin/dev-team-config.schema.json` | JSON Schema draft-2020-12 | JSON |
| test-framework | `FrameworkConfig.config_flag`；jest/vitest/vite-plus 模板嵌入 `{config_args}` | `plugins/dev-team/bin/src/lib/test-framework.ts` | `schemas` (`TestFramework`) | TypeScript |
| test-detect-frameworks | 从 `tests[]` 构建 plan；`directory`=absCwd；展开 `{config_args}`；suite scope 匹配文件 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `test-framework`, `test-exclude`, `config`, `glob` | TypeScript |
| test-exclude | 按 `tests[].excludes`（相对 root）判断排除；不再读旧 `test.exclude` / overrides | `plugins/dev-team/bin/src/lib/test-exclude.ts` | `glob`, `OpenSpecConfig` | TypeScript |
| test-resolve-paths | 空 modules 按 suite `root` 扫描；错误文案引导配置 `tests` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `test-detect-frameworks`, `test-exclude` | TypeScript |
| test-report | 覆盖率 / 变异阈值按 suite 读取；分组结果改为 suite 维度（报告字段名可仍为 `overrides`） | `plugins/dev-team/bin/src/lib/test-report.ts` | `config`, `glob` | TypeScript |
| test-runner | 变异阶段 cwd=absCwd；mutate 源文件相对 absCwd；exclude 用 suite 语义 | `plugins/dev-team/bin/src/lib/test-runner.ts` | `test-exclude`, `stryker-config` | TypeScript |
| stryker-config | 以 absCwd 为 `rootPath` 生成临时配置；mutate 路径相对该根 | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | Node `fs`/`path` | TypeScript |
| project-config | 本仓库 `openspec/config.json` 迁入 `tests[]`（方案 G） | `openspec/config.json` | `dev-team-config.schema.json` | JSON |
| agent-prompt | 文案中旧 `test.*` 键改为 `tests` | `plugins/dev-team/agents/test-execution-executor.md` 等 | 无 | Markdown |
| plugin-manifest | 按项目规则升级插件版本 | `plugins/dev-team/.claude-plugin/plugin.json` | 无 | JSON |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、HTTP 端点，私有函数不列入。
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/schemas/config/defaults.ts` | 导出 `TEST_COVERAGE_LINE_DEFAULT` / `TEST_COVERAGE_BRANCH_DEFAULT` / `TEST_COVERAGE_FUNCTION_DEFAULT` / `TEST_MUTATION_SCORE_DEFAULT`，供 `config.schema.ts` 的 suite `prefault` 引用 |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 删除顶层 `test` 对象；新增 `tests` 数组 suite schema（必填 `root`+`framework`；可选 `cwd`/`config`/`includes`/`excludes`/`coverage`/`mutation`）；`root` 拒绝 `*`/`?`/`{`/`[`；常量改为从 `defaults.ts` 导入；`tests` 缺省 `prefault([])` | AC-1；硬 breaking，不双读旧 `test` |
| `plugins/dev-team/bin/dev-team-config.schema.json` | `properties.test` 替换为 `properties.tests`（array of suite）；suite `required: ["root","framework"]`；同步字段与默认值描述 | 与 Zod 镜像 |
| `plugins/dev-team/bin/src/lib/test-framework.ts` | `FrameworkConfig` 增加 `config_flag`；jest/vitest/vite-plus 设为 `"--config"` 且 `test_execution` 含 `{config_args}`；其余框架 `config_flag: null` | AC-3；禁止依赖整段 append |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 以 `config.tests` 构建 plan；移除 `deriveWorkingDirectory` / 旧 `normalizeFrameworks`；`directory`=absCwd 相对项目根；script 展开 `{config_args}`；无 `config_flag` 却声明 `config` 时抛错；`mutation_score` 取自 suite；文件匹配用 suite scope | AC-2/3/4/5 |
| `plugins/dev-team/bin/src/lib/test-exclude.ts` | `isFileExcluded` / `getExcludeGlobs` 改为基于 `tests[].excludes`（拼成 projectRoot 相对模式，仅作用于对应 `root` 树） | AC-4 |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 空 modules 扫描根取 suite `root`（覆盖 scope，不因 `cwd` 上移漏扫/误扫）；无 plan 时错误文案改为配置 `tests` | AC-4/6 |
| `plugins/dev-team/bin/src/lib/test-report.ts` | 删除全局 `test.coverage` + overrides 级联；阈值按 suite（匹配 `directory`+`framework` 或等价）读取；mutation 分组改 suite 维度；移除模块内手写平行默认级联（改用 parse 后 suite 值） | AC-5 |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 变异阶段继续以 `entry.directory`（absCwd）为 cwd；传入 `resolveStrykerConfig` 的源路径保证相对 absCwd；exclude 走新 suite 语义 | AC-2/4 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | 确认 / 必要时强化：`mutate` 条目相对 `rootPath`（即 absCwd）规范化；临时 `stryker.config.*` 落在该目录 | AC-2；第一版无独立 artifacts |
| `plugins/dev-team/bin/src/commands/test-execution.ts` | 无配置提示改为引导 `tests` | AC-6 |
| `plugins/dev-team/bin/src/mcp.ts` | `test_detect_frameworks` 工具描述改为 `tests` suite 映射 | AC-6 |
| `openspec/config.json` | `test.overrides` 迁为 `tests[]` 方案 G（含本仓库 vite-plus suite 的 `root`/`includes`/`excludes`/`config` 等） | AC-6 |
| `plugins/dev-team/agents/test-execution-executor.md` | 将 `test.coverage` 等旧键表述改为 suite / `tests[].coverage` | 文案同步 |
| `plugins/dev-team/.claude-plugin/plugin.json` | 升级 `version` | 项目规则：改插件代码后升版 |
| `plugins/dev-team/bin/src/schemas/index.ts` | 如导出 `TestSuite` / defaults 常量则在此 re-export | 供类型消费者使用 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `TEST_COVERAGE_LINE_DEFAULT` 等 | `plugins/dev-team/bin/src/schemas/config/defaults.ts` | 新增 | `const TEST_COVERAGE_LINE_DEFAULT: number`（及 BRANCH/FUNCTION/MUTATION_SCORE） | schema 默认阈值唯一来源 |
| `getFrameworkConfig` | `plugins/dev-team/bin/src/lib/test-framework.ts` | 修改 | `function getFrameworkConfig(framework: string): FrameworkConfig` | 返回值新增 `config_flag`；支持注入的框架模板含 `{config_args}` |
| `isFileExcluded` | `plugins/dev-team/bin/src/lib/test-exclude.ts` | 修改 | `function isFileExcluded(filePath: string, config: OpenSpecConfig): boolean` | 语义改为 suite-scoped `excludes`；签名不变 |
| `getExcludeGlobs` | `plugins/dev-team/bin/src/lib/test-exclude.ts` | 修改 | `function getExcludeGlobs(config: OpenSpecConfig): string[]` | 返回所有 suite 拼好的 project-relative exclude 模式并集 |
| `runTestDetectFrameworks` | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 修改 | `function runTestDetectFrameworks(options: TestDetectFrameworksOptions): TestDetectFrameworksResult` | 读 `config.tests`；plan.directory=absCwd；注入 config；suite scope 匹配 |
| `runTestResolvePaths` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 修改 | `function runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult` | 扫描根 / 错误文案适配 `tests` |
| `executePlanEntry` | `plugins/dev-team/bin/src/lib/test-runner.ts` | 修改 | `function executePlanEntry(entry: TestPlan, projectRoot: string, options?: { files?: string[]; timeout?: number; noMutation?: boolean; mutationDiffFiles?: string[] }): ExecutionResult` | 变异 cwd/路径/exclude 适配；签名保持 |
| `resolveStrykerConfig` | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | 修改 | `function resolveStrykerConfig(rootPath: string, sourceFiles: string[], framework: string): { configPath: string; tempDirPath: string }` | 保证 mutate 相对 `rootPath`（absCwd）；签名保持 |
| `generateSubReport` | `plugins/dev-team/bin/src/lib/test-report.ts` | 修改 | `function generateSubReport(framework: string, result: ExecutionResult, projectRoot: string, reportsDir: string, planDirectory: string): TestExecutionSubReport` | 阈值来自匹配 suite；签名可保持，内部改查 `config.tests` |
| `generateSummaryReport` | `plugins/dev-team/bin/src/lib/test-report.ts` | 修改 | `function generateSummaryReport(subReports: TestExecutionSubReport[], projectRoot: string, reportsDir: string): TestExecutionSummaryReport` | coverage/mutation 分组改 suite 维度 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `OpenSpecConfig` | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 修改 | 正式模型为 `tests: TestSuite[]`；不再包含 `test.framework` / `test.overrides` 等 |
| `OpenSpecConfigInput` | 同上 | 修改 | 输入侧对应 `tests` suite 形状 |
| `TestSuite` | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 新增 | `z.output` 推导的 suite 元素类型：`{ root, framework, cwd, config?, includes?, excludes?, coverage, mutation }`；建议 `export type TestSuite = OpenSpecConfig['tests'][number]` |
| `FrameworkConfig` | `plugins/dev-team/bin/src/lib/test-framework.ts` | 修改 | 增加 `config_flag: string \| null` |
| `TestPlan` | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | 修改（语义） | `directory` 定义为 absCwd 相对项目根；`mutation_score` 来自 suite（非全局）；结构字段名可不变 |

### 配置

| 配置键 | 所在文件 | 类型 | 值类型 | 默认值 | 说明 |
|--------|----------|------|--------|--------|------|
| `tests` | `openspec/config.json` | 新增（替换 `test`） | `TestSuite[]` | `[]` | 顶层测试 suite 数组；硬 breaking |
| `tests[].root` | 同上 | 新增 | `string` | （必填） | 相对 projectRoot；禁止通配符 |
| `tests[].framework` | 同上 | 新增 | `TestFramework` enum | （必填） | 测试框架 |
| `tests[].cwd` | 同上 | 新增 | `string` | `"."` | 相对 root 的执行目录 |
| `tests[].config` | 同上 | 新增 | `string` | 无 | 相对 root 的框架配置文件；需框架 `config_flag` |
| `tests[].includes` | 同上 | 新增 | `string[]` | 无（消费者用 `default_glob`） | 相对 root 的匹配 glob |
| `tests[].excludes` | 同上 | 新增 | `string[]` | 无 | 相对 root 的排除 glob |
| `tests[].coverage.lines` / `branches` / `functions` | 同上 | 新增 | `number` | 80 / 70 / 75 | schema 常量 |
| `tests[].mutation.score` | 同上 | 新增 | `number` | 70 | schema 常量 |
| `test`（整对象） | `openspec/config.json` | 删除 | — | — | 不再作为正式配置模型解析 |
| `version` | `plugins/dev-team/.claude-plugin/plugin.json` | 修改 | `string` | 当前 `2.9.4` → 下一 patch/minor | 插件代码变更后升版 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `TestSuite` | `root: string`；`framework: TestFramework`；`cwd: string`（默认 `"."`）；`config?: string`；`includes?: string[]`；`excludes?: string[]`；`coverage: { lines, branches, functions }`；`mutation: { score }` | 隶属于 `OpenSpecConfig.tests[]`；一对多 plan 条目（通常 1 suite → 1 plan） | `openspec/config.json`，经 `readConfig` / `configSchema` 解析 |
| 路径派生（运行时，不持久化） | `absRoot = projectRoot/root`；`absCwd = absRoot/(cwd??".")`；`absConfig = absRoot/config`（可选）；`scope = under(root) ∩ includesEffective ∩ ¬excludes`，其中 `includesEffective = includes ?? framework.default_glob` | 驱动 plan.directory、script `cd`、检测匹配、exclude、变异 mutate 相对路径 | 内存 / plan JSON 输出 |
| `TestPlan` | 既有字段；`directory` = absCwd 相对 projectRoot（POSIX）；`mutation_score` = suite.mutation.score；`script` 内 `{config_args}` 已展开 | 由 `runTestDetectFrameworks` 从 `tests[]` 生成 | 命令/MCP 输出，不写入 config |
| 产物落点（第一版） | `coverage/`、`.stryker-tmp/`、`stryker.config.*`、`reports/mutation/` | 相对 absCwd；不引入 `artifacts` 字段 | 文件系统（absCwd 下） |

路径约定示意：

```
projectRoot
└── root/                    ← suite.root（锚点，禁通配符）
    ├── cwd/ 或 ../          ← suite.cwd → absCwd（执行与产物）
    ├── config 文件          ← suite.config（相对 root；注入时再相对 absCwd）
    └── includes/excludes    ← 范围集合（相对 root）
```

本仓库迁移目标形态（与 proposal 示例对齐）：

```json
{
  "tests": [
    {
      "root": "plugins/dev-team/bin",
      "framework": "vite-plus",
      "cwd": ".",
      "config": "vite.config.ts",
      "includes": ["src/**/*.{ts,tsx}"],
      "excludes": ["./*", "src/schemas/**/*"]
    }
  ]
}
```

---

<!-- 本变更不涉及 HTTP API，省略路由/API 设计节 -->

---

## 依赖

### 运行时依赖

- 无新增运行时依赖。继续使用现有 `zod/v4`、`picomatch`（经 `lib/glob.ts`）、Node 内置 `fs`/`path`。

### 构建/测试依赖

- 无新增构建/测试依赖。（测试夹具与单测更新属独立测试阶段，本设计不展开。）

---

## 关键设计要点

### Plan 与 script

1. 对每个 suite：`absRoot` / `absCwd` / 可选 `absConfig` 按上表计算；`plan.directory = path.relative(projectRoot, absCwd)`（POSIX）。
2. `generateShellScript` / `generateCmdScript`：在写入 `test_execution` 前将 `{config_args}` 替换为 `` `${config_flag} ${posixRelative(absCwd, absConfig)}` `` 或空串；**禁止**对整段字符串末尾 append。
3. suite 有 `config` 且 `config_flag == null` → 抛出明确错误（plan 失败）。
4. 删除 `deriveWorkingDirectory`。

### Suite scope

- `inScope(file, suite)`：文件位于 `root` 下，且匹配 `includesEffective`，且不匹配该 suite 的 `excludes`。
- 多 suite 时数组顺序优先（第一个匹配决定 `detected[].framework`）。
- `isFileExcluded`：仅当文件落在某 suite 的 `root` 下且匹配该 suite 的 scoped exclude 时为 `true`。

### 报告阈值

- `readCoverageThresholds` / `computeOverrides` / `computeMutationOverrides` 改为遍历 `config.tests`，按 suite scope 匹配源文件并使用该 suite 的 `coverage` / `mutation.score`。
- 报告输出中若仍保留 `overrides` 字段名，语义为「按 suite 分组」，`glob` 可用 `suite.root`（或 includes 联合标识）填充，避免再读旧 `test.overrides`。

### 空 modules 扫描

- 扫描根优先使用各 suite 的 `root`，再应用 scope / `isFileExcluded`，避免 `cwd: ".."` 时误扫 root 外或漏扫 root 内。
- 省略 `includes` 时有效 includes 为框架 `default_glob`（与 AC-4 一致）；需要源码树范围时显式配置 `includes`（本仓库迁移即如此）。

---

## 待决问题

- 无（proposal 已拍板：硬 breaking、方案 G、`{config_args}` 占位、第一版无 `artifacts`、pytest `-c` / rust manifest 不纳入本 change。）
