# 设计: 移除 test_resolve_paths 集成测试路径解析功能

> **变更**: remove-integration-test-path-resolution
> **日期**: 2026-07-15

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| test-resolve-paths 命令 | MCP 工具入口与核心路径解析逻辑：接收模块列表，推导单元测试文件路径 | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | schemas, config, test-exclude, test-detect-frameworks | TypeScript |
| test-resolve-paths schema | Zod 定义 MCP 工具输入/输出 schema，含字段描述与类型校验 | `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` | zod/v4 | TypeScript |
| MCP 注册层 (mcp.ts) | 将各命令模块注册为 MCP 工具，声明 name/description/inputSchema/outputSchema | `plugins/dev-team/bin/src/mcp.ts` | @modelcontextprotocol/sdk, schemas, commands | TypeScript |
| test-design-planner agent | Subagent prompt，指导测试设计流程，含 test_resolve_paths 调用步骤 | `plugins/dev-team/agents/test-design-planner.md` | 无 | Markdown |

---

## 变更清单

<!--
  以文件为入口逐层展开，实现阶段以此清单为边界。
  子表按需填写，不涉及的子表整段省略并以 HTML 注释标注原因。
-->

### 新增文件

<!-- 如无新增文件，省略此子节 -->

本变更不涉及新增文件。

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 移除 `IntegrationTestEntry` 接口；移除 `normalizeIntegrationRoot`、`isValidIntegrationRoot`、`deriveIntegrationTestPath`、`normalizeExtension`、`inferExtension`、`resolveIntegrationTests` 六个函数；从 `ResolveTestPathsParams` 移除 `integrationScenarios`、`extension`、`integrationRoot` 字段；从 `ResolveTestPathsResult` 移除 `integration_tests` 字段；从 `TestResolvePathsInput` 移除 `integration_scenarios`、`extension`、`integration_root` 字段；从 `resolveTestPaths` 移除对 `resolveIntegrationTests` 的调用和 `integration_tests` 返回值；简化 `runTestResolvePaths` 中参数传递 | 核心变更文件，移除全部集成测试路径解析逻辑 |
| `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` | 输入 schema 移除 `integration_scenarios`、`extension`、`integration_root` 字段；输出 schema 移除 `integration_tests` 字段；更新英文注释（不再提及集成测试） | 移除集成测试相关的 schema 定义 |
| `plugins/dev-team/bin/src/mcp.ts` | 更新 `registerTestResolvePathsTool` 的 description 文本，移除有关集成测试路径的说明 | description 不再提及 "integration" 或 "__tests__/<scenario>/" |
| `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` | 移除 `-- 集成测试路径`、`-- integration_root 向后兼容`、`-- integration_root 为点号`、`-- integration_root 子目录前缀`、`-- integration_root 与 unit_tests 隔离`、`-- integration_root 路径穿越`、`-- integration_root snake_case 映射` 共七个 describe 块；更新 `-- 端到端编排` 中引用 `integration_scenarios`/`integration_tests` 的测试用例；移除端到端编排中包含集成测试参数的用例 | 移除集成测试相关的所有测试用例 |
| `plugins/dev-team/agents/test-design-planner.md` | Step 7：不再识别"场景名列表"；Step 8：移除 `integration_root`/`integration_scenarios` 说明；Step 9：删除整个"集成测试路径"段落；Step 11：删除"将合并后的 integration_tests 映射到集成测试表格"；Step 12：仅提及单元测试 errors；Constraints 第 4-5 条：放宽为仅单元测试路径来自 `test_resolve_paths`，集成测试路径由 subagent 自主决定 | Subagent 不再调用 `test_resolve_paths` 获取集成测试路径，改为自行在 plan entry 目录下创建 `__tests__/` |
| `plugins/dev-team/bin/src/mcp.test.ts` | 确认工具注册个数验证保持 10 不变 | 无需实际代码修改，仅确认移除 integration_tests 后 `mcp.ts` 注册的工具数校验值仍为 10 |

### 公共函数 / API

| 标识符 | 所在文件 | 类型 | 签名 | 说明 |
|--------|----------|------|------|------|
| `runTestResolvePaths` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 修改 | `function runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult` | 移除 `integration_scenarios`、`extension`、`integration_root` 参数传递，返回类型不再包含 `integration_tests` |

### 类型定义

| 类型名 | 所在文件 | 类型 | 说明 |
|--------|----------|------|------|
| `IntegrationTestEntry` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 删除 | 接口 `{ scenario: string; test_file: string }`，集成测试路径条目 |
| `ResolveTestPathsParams` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 修改 | 移除 `integrationScenarios?: string[]`、`extension?: string`、`integrationRoot?: string` 三个字段 |
| `ResolveTestPathsResult` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 修改 | 移除 `integration_tests: IntegrationTestEntry[]` 字段，仅保留 `unit_tests` 与 `errors` |
| `TestResolvePathsInput` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 修改 | 移除 `integration_scenarios?: string[]`、`extension?: string`、`integration_root?: string` 三个字段 |

### 配置

<!-- 如无配置变更，省略此子节 -->

本变更不涉及配置变更。

---

## 数据模型

本变更不新增数据模型，仅移除已有模型。

**移除的模型：**

| 模型 | 字段 | 持久化 | 说明 |
|------|------|--------|------|
| `IntegrationTestEntry` | `scenario: string`， `test_file: string` | 否（TypeScript 接口） | 集成测试路径条目，每个条目记录场景名和推导出的测试文件路径 |

**修改的模型：**

| 模型 | 变更前字段 | 变更后字段 | 持久化 | 说明 |
|------|-----------|-----------|--------|------|
| `ResolveTestPathsResult` | `unit_tests: UnitTestEntry[]`，`integration_tests: IntegrationTestEntry[]`，`errors: ResolveError[]` | `unit_tests: UnitTestEntry[]`，`errors: ResolveError[]` | 否（TypeScript 类型） | 移除 `integration_tests` 输出字段 |

---

## 路由/API 设计

<!-- 本变更为 MCP 工具参数和输出变更，不涉及 HTTP API，仅通过 MCP schema 定义。此处略 -->

本变更不涉及 HTTP 路由/API，相关变更已在 变更清单 - Schema 节中覆盖。

---

## 依赖

### 运行时依赖

- 无新增依赖，无移除的运行时依赖。

### 构建/测试依赖

- 无变更。

---

## 待决问题

1. **`extension` 参数一并移除**：`extension` 参数在现有代码中仅被 `inferExtension()` 消费，而 `inferExtension()` 属于集成测试路径解析函数并被移除。移除后 `extension` 在 schema 和接口中成为死代码。设计决定一并移除，但 AC-1 未明确列出此项。确认是否接受该衍生变更。

2. **`mcp.test.ts` 无实际变更**：proposal 将该文件列为变更文件，但工具注册个数保持 10 不变（无工具移除），且现有测试未对 `test_resolve_paths` 的 description 做断言。若严格遵循 proposal，该文件无需修改。确认是否保持原样。

3. **Subagent prompt 变更后是否影响存量 workflow**：`test-design-planner.md` 移除集成测试路径推导步骤后，存量进行中的 workflow 的 test-design 阶段若重新执行，将不再生成 `integration_scenarios` 参数传入 `test_resolve_paths`。因参数被移除后 Zod 静默忽略多余字段，不会导致运行时错误。
