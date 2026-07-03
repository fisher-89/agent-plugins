# 设计: unit-test-mutation-testing

> **变更**: unit-test-mutation-testing
> **日期**: 2026-07-01

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| Mutation 配置 Schema | 定义 `mutation` 阈值 schema 和 override 中的 `mutation` 字段 | `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | zod/v4 | TypeScript |
| 框架注册表扩展 | 在 `FrameworkConfig` 中添加 `mutation_framework` 字段，标记支持框架 | `plugins/dev-team/bin/src/lib/test-framework.ts` | — | TypeScript |
| 框架检测扩展 | 在 `PlanEntry` 中添加 mutation 相关字段，传递到执行层 | `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | `test-framework.ts`, `config.schema.ts` | TypeScript |
| 框架检测 Schema 扩展 | 在 `PlanEntry` 的 output schema 中添加 mutation 字段 | `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | zod/v4 | TypeScript |
| StrykerJS 配置生成器 | 检测项目是否已有 `stryker.config.*`，否则生成临时配置文件 | `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | `path`, `fs` | TypeScript |
| StrykerJS 报告解析器 | 解析 StrykerJS JSON 报告输出，提取 score 和变异体计数 | `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.ts` | — | TypeScript |
| 输出 Schema 扩展 | 定义 `MutationBlock`、`MutationMeasured` 等 schema | `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` | zod/v4 | TypeScript |
| 测试运行器扩展 | 在 `executePlanEntry()` 中添加 mutation 执行阶段 | `plugins/dev-team/bin/src/lib/test-runner.ts` | `mutation-parser.ts`, `stryker-config.ts` | TypeScript |
| 报告生成器扩展 | 在子报告和汇总报告中添加 mutation 块 | `plugins/dev-team/bin/src/lib/test-report.ts` | `config.schema.ts`, `unit-test-output.schema.ts` | TypeScript |
| CLI 扩展 | 添加 `--no-mutation` 选项 | `plugins/dev-team/bin/src/cli.ts` | cac | TypeScript |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
  公共函数仅列模块级导出函数、CLI 子命令、HTTP 端点，私有函数不列入。
  签名格式：Python → create_adr(title: str, status: str = "proposed") -> dict；TS → function parseImports(file: string): Import[]
-->

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `plugins/dev-team/bin/src/lib/test-parser/mutation-parser.ts` | 解析 StrykerJS JSON 报告输出，提取变异得分和各类变异体计数 |
| `plugins/dev-team/bin/src/lib/test-parser/stryker-config.ts` | 检测项目 StrykerJS 配置状态，生成临时配置文件 |

<!-- 无新增配置文件 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/schemas/config/config.schema.ts` | 添加 `mutation` 阈值 schema；在 override 对象中添加 `mutation` 字段 | 定义 `mutation.score` 可选数字（默认 80）；override 支持 `mutation: { score }` |
| `plugins/dev-team/bin/src/lib/test-framework.ts` | 在 `FrameworkConfig` 中添加 `mutation_framework: string \| null`；在 registry 中为 jest/vitest/vite-plus 设置 `"stryker-js"`，其余为 `null` | 标记哪些框架支持 mutation testing |
| `plugins/dev-team/bin/src/commands/test-detect-frameworks.ts` | 在 `PlanEntry` 中添加 `mutation_framework`、`mutation_config`、`mutation_score` 字段；在 `buildPlanFromMappings()` 中填充这些字段 | 将 mutation 配置从 registry 和 config 传递到执行层 |
| `plugins/dev-team/bin/src/schemas/test-detect-frameworks.schema.ts` | 在 `PlanEntry` output schema 中添加 `mutation_framework`、`mutation_config`、`mutation_score` 字段 | 对齐 `PlanEntry` 类型的 schema 变更 |
| `plugins/dev-team/bin/src/schemas/unit-test-output.schema.ts` | 添加 `MutationMeasured`、`MutationOverride`、`MutationBlock` schema；在 `UnitTestSubReport` 和 `UnitTestSummaryReport` 中添加 `mutation` 字段 | 定义 mutation 报告的完整结构 |
| `plugins/dev-team/bin/src/lib/test-runner.ts` | 在 `ExecutionResult` 中添加 `mutation` 字段；在 `executePlanEntry()` 中添加 mutation 执行阶段 | 覆盖率解析后执行 StrykerJS，结果存入 `ExecutionResult.mutation` |
| `plugins/dev-team/bin/src/lib/test-report.ts` | 在子报告和汇总报告生成中添加 mutation 块；添加 `computeMutationResult()` 和 `computeMutationOverrides()` | 读取 `ExecutionResult.mutation`，写入子报告和汇总报告的 `mutation` 字段 |
| `plugins/dev-team/bin/src/cli.ts` | 添加 `--no-mutation` CLI 选项；在 `UnitTestOptions` 中传递 `noMutation` 标志 | 允许用户跳过 mutation 阶段 |
| `plugins/dev-team/bin/src/commands/unit-test.ts` | 在 `UnitTestOptions` 中添加 `noMutation`；在 `runUnitTest()` 中传递给 `executePlanEntry` | 透传 `--no-mutation` 标志到执行层 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `parseMutationReport` | `mutation-parser.ts` | 新增 | `function parseMutationReport(reportPath: string): MutationReport \| null` | 读取并解析 StrykerJS JSON 报告文件 |
| `resolveStrykerConfig` | `stryker-config.ts` | 新增 | `function resolveStrykerConfig(projectRoot: string, sourceFiles: string[], testFiles: string[], framework: string): { configPath: string, cleanup: boolean }` | 检测项目是否已有 stryker.config.*，否则生成临时配置文件；返回配置文件路径和是否需要清理 |
| `executePlanEntry` | `test-runner.ts` | 修改 | `function executePlanEntry(entry: PlanEntry, projectRoot: string, options?: { files?: string[], timeout?: number, noMutation?: boolean }): ExecutionResult` | 新增 `noMutation` 选项参数 |
| `generateSubReport` | `test-report.ts` | 修改 | `function generateSubReport(framework: string, result: ExecutionResult, projectRoot: string, reportsDir: string): UnitTestSubReport` | 内部新增 mutation 块构建逻辑 |
| `generateSummaryReport` | `test-report.ts` | 修改 | `function generateSummaryReport(subReports: UnitTestSubReport[], projectRoot: string, reportsDir: string): UnitTestSummaryReport` | 内部新增 mutation 聚合逻辑 |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `FrameworkConfig.mutation_framework` | `test-framework.ts` | 修改 | `FrameworkConfig` 新增 `mutation_framework: string \| null` 字段 |
| `PlanEntry.mutation_framework` | `test-detect-frameworks.ts` | 修改 | `PlanEntry` 新增 `mutation_framework: string \| null` |
| `PlanEntry.mutation_config` | `test-detect-frameworks.ts` | 修改 | `PlanEntry` 新增 `mutation_config: { score: number } \| null` |
| `PlanEntry.mutation_score` | `test-detect-frameworks.ts` | 修改 | `PlanEntry` 新增 `mutation_score: number \| null` |
| `ExecutionResult.mutation` | `test-runner.ts` | 修改 | `ExecutionResult` 新增 `mutation: MutationBlock \| null` |
| `UnitTestOptions.noMutation` | `unit-test.ts` | 修改 | `UnitTestOptions` 新增 `noMutation?: boolean` |
| `MutationMeasured` | `unit-test-output.schema.ts` | 新增 | 变异体计数类型：`killed`, `survived`, `timeout`, `noCoverage`, `compileError`, `runtimeError`, `ignored`, `total`, `detected`, `undetected` |
| `MutationOverride` | `unit-test-output.schema.ts` | 新增 | 按 overrides 分组的 mutation 结果：`glob`, `score`, `threshold`, `pass`, `file_count`, `passed_count` |
| `MutationBlock` | `unit-test-output.schema.ts` | 新增 | Mutation 结论块：`pass`, `score`, `threshold`, `measured`, `by_framework`, `overrides` |
| `UnitTestSubReport.mutation` | `unit-test-output.schema.ts` | 修改 | `UnitTestSubReport` 新增 `mutation: MutationBlock \| null` |
| `UnitTestSummaryReport.mutation` | `unit-test-output.schema.ts` | 修改 | `UnitTestSummaryReport` 新增 `mutation: MutationBlock \| null` |

### 配置

| 配置键 | 所在文件 | 类型 | 默认值 | 说明 |
|--------|----------|------|--------|------|
| `test.mutation.score` | `openspec/config.json` | 新增 | `number` 可选 | `80` | 变异得分阈值（百分比），仅在 mutation 框架生效时使用 |
| `test.overrides[].mutation` | `openspec/config.json` | 新增 | `{ score: number }` 可选 | — | 按目录覆盖变异得分阈值，与现有的 `coverage` overrides 模式一致 |

---

## 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `MutationMeasured` | `killed`, `survived`, `timeout`, `noCoverage`, `compileError`, `runtimeError`, `ignored`, `total`, `detected`, `undetected` (均为 `number`) | 内嵌于 `MutationBlock` | 写入 JSON 报告文件 |
| `MutationBlock` | `pass: boolean`, `score: number`, `threshold: number`, `measured: MutationMeasured`, `by_framework: Record<string, { score: number, measured: MutationMeasured, source_files: string[] }>`, `overrides?: MutationOverride[]` | 内嵌于 `UnitTestSubReport` 和 `UnitTestSummaryReport` | 写入 JSON 报告文件 |
| `MutationOverride` | `glob: string`, `score: number`, `threshold: number`, `pass: boolean`, `file_count: number`, `passed_count: number` | 内嵌于 `MutationBlock.overrides` | 写入 JSON 报告文件 |

### 报告结构

```
reports/unit-test/<framework>.json (子报告，新增 mutation 字段)
reports/unit-test-execution.json    (汇总报告，新增 mutation 字段)
```

StrykerJS 中间产物：`reports/mutation/mutation.json`（执行后清理）

---

## 路由/API 设计

<!-- 本变更为 CLI 工具内部逻辑，不涉及 HTTP API，省略此节 -->

---

## 依赖

### 运行时依赖

- `@stryker-mutator/core` — StrykerJS 变异测试引擎核心，由用户在项目中安装
- `@stryker-mutator/jest-runner` — StrykerJS Jest runner 插件（框架为 jest 时）
- `@stryker-mutator/vitest-runner` — StrykerJS Vitest runner 插件（框架为 vitest/vite-plus 时）

### 构建/测试依赖

<!-- 无新增构建/测试依赖 -->

---

## 待决问题

- 无

---

## 实现说明

### 执行流程

```
executePlanEntry()
  ├── 执行测试命令 (现有)
  ├── 解析测试输出 (现有)
  ├── 解析覆盖率报告 (现有)
  ├── if (mutation_framework 非空 && !noMutation)
  │   ├── resolveStrykerConfig()  // 检测或生成临时配置
  │   ├── execSync("npx stryker run ...")
  │   ├── parseMutationReport()    // 解析 reports/mutation/mutation.json
  │   └── 清理临时配置文件
  └── 返回 ExecutionResult (含 mutation 字段)
```

### StrykerJS 配置模板

生成临时配置时的关键参数：
- `mutate` — 限定变异范围为被测试覆盖的源文件（sourceFiles）
- `testRunner` — 根据 `mutation_framework` 决定（"jest" 或 "vitest"）
- `plugins` — 对应 runner 插件
- `reporters` — 设置为 `["json"]`，输出到 `reports/mutation/mutation.json`
- `thresholds` — 从 `config.test.mutation.score` 读取
- `tempDirName` — 使用随机目录名（stryker-js 默认行为）
- `timeoutMS` — 设默认值 60000，防止单测试执行过长
- `maxTestRunnerReuse` — 设默认值 10，限制 runner 重用次数

### 变异得分阈值判定

```
pass = measured.score >= thresholds.score
```

- 全局阈值从 `config.test.mutation.score` 读取，默认 80
- override 阈值从 `config.test.overrides[].mutation.score` 读取，未设置时回退到全局阈值
- 汇总报告的 pass 需要全局 pass 且所有 override pass
- 对于 `mutation_framework` 为 null 的框架，子报告和汇总报告的 mutation 字段均为 null（不参与判定）

### 中间产物清理

- StrykerJS JSON 报告输出到 `reports/mutation/mutation.json`
- 临时配置文件写入项目根目录（stryker.config.json，使用 `{ random }` 命名避免冲突）
- 执行完成后清理：删除 `reports/mutation/` 目录和临时配置文件
