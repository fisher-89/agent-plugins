# 设计: add-integration-root-param

> **变更**: add-integration-root-param
> **日期**: 2026-06-16
> **基于**: proposal.md, specs/test-path-resolver/spec.md, specs/phase-agents/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `testResolvePathsInputSchema` | 在现有 input schema 上增加可选字段 `integration_root` | `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` | `zod/v4` | TypeScript / Zod |
| `deriveIntegrationTestPath()` | 纯函数：按场景名与扩展名推导集成测试路径；支持可选 `integrationRoot` 前缀 | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | 无 | TypeScript |
| `resolveTestPaths()` | 从 `params.integrationRoot` 读取前缀并传给 `deriveIntegrationTestPath()`；**不**影响 `unit_tests` 推导 | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `deriveIntegrationTestPath()` | TypeScript |
| `runTestResolvePaths()` | 将 MCP 入参 `integration_root` 映射为 `integrationRoot` 并委托 `resolveTestPaths()` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `resolveTestPaths()`, `getProjectDir()` | TypeScript |
| `mcp.ts` 工具注册 | `test_resolve_paths` handler 已通过 `{ ...args }` 透传；schema 更新后自动暴露新字段 | `plugins/dev-team/bin/src/mcp.ts` | `runTestResolvePaths`, schema | TypeScript / MCP SDK |
| `test-design-planner` 代理 | 单元测试路径仍单次调用；存在集成场景时先 `test_detect_frameworks`，再按 plan 条目 `directory` 多次调用 `test_resolve_paths` 并合并 `integration_tests` | `plugins/dev-team/agents/test-design-planner.md` | MCP 工具 `test_resolve_paths`、`test_detect_frameworks` | Markdown prompt (Agent) |

### 组件图

```
test-design-planner (Agent)
  |-- Read proposal.md, design.md
  |-- Grep 源码
  |-- CallMcpTool: test_resolve_paths (modules, extension?)
  |       → unit_tests（一次，不传 integration_root）
  |
  |-- [若存在 integration_scenarios]
  |     CallMcpTool: test_detect_frameworks
  |     对每个 plan 条目:
  |       CallMcpTool: test_resolve_paths
  |         modules, integration_scenarios, extension,
  |         integration_root = plan[i].directory
  |       → 合并 integration_tests
  v
MCP Server (mcp.ts)
  |-- resolveProjectRoot(project_root?)
  |-- runTestResolvePaths({ ...args, integration_root? })
  v
commands/test-resolve-paths.ts
  |-- resolveTestPaths({ integrationRoot? })
  |     |-- modules[] → deriveUnitTestPath() [不变]
  |     |-- integrationScenarios[] → deriveIntegrationTestPath(scenario, ext, integrationRoot)
  v
JSON 响应
  |-- unit_tests:        { source, test_file }[]     [不受 integration_root 影响]
  |-- integration_tests: { scenario, test_file }[]   [可含子目录前缀]
  |-- errors:            { path, message }[]

（只读引用，不修改）test_detect_frameworks → plan[].directory
（不变）unit-test-executor / integration-test-executor 执行逻辑
```

---

## 数据流

### 流程描述

1. **`test-design-planner`** 从 design.md 变更范围与 Grep 结果汇总 `modules`，从 proposal/design 识别 `integration_scenarios`（若有）。
2. **单元测试路径**：调用一次 `test_resolve_paths`，传入 `modules` 与可选 `extension`；**不传** `integration_root`（colocated 单元测试始终相对 `project_root`，与框架工作目录无关）。
3. **集成测试路径**（仅当存在 `integration_scenarios` 时）：
   - 调用 `test_detect_frameworks` 获取 `plan` 数组，每条含 `directory`（相对或绝对工作目录，如 `"."` 或 `"plugins/dev-team/bin"`）。
   - 对**每个** plan 条目调用 `test_resolve_paths`，传入相同 `modules`、`integration_scenarios`、`extension`，并将该条目的 `directory` 作为 `integration_root`。
   - 合并各次返回的 `integration_tests` 写入 test-design.md `集成测试 > 用例` 表格；若同一 `test_file` 重复出现，planner 按框架/目录分组或去重展示。
4. **`runTestResolvePaths()`** 将 snake_case `integration_root` 映射为 camelCase `integrationRoot`，委托 `resolveTestPaths()`。
5. **`deriveIntegrationTestPath()`** 规范化 `integrationRoot`（POSIX `/`、去除尾部斜杠）；未提供或为 `"."` 时返回 `__tests__/<scenario>/<scenario>.test.<ext>`；否则返回 `<integrationRoot>/__tests__/...`。
6. 扩展名推断、排序、`errors` 收集逻辑与 `add-test-path-resolver-api` 完全一致；`integration_root` **仅**影响 `integration_tests`。

### 路径规则（更新后）

| 条件 | 集成测试路径示例（scenario=`api-flow`, ext=`ts`） |
|------|--------------------------------------------------|
| 未传 `integration_root` 或值为 `"."` | `__tests__/api-flow/api-flow.test.ts` |
| `integration_root: "plugins/dev-team/bin"` | `plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts` |
| `integration_root: "plugins/dev-team/bin/"`（尾部斜杠） | 同上（规范化后无重复斜杠） |

单元测试路径规则**不变**（与源文件 colocated，不受 `integration_root` 影响）。

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `ResolveTestPathsParams` | 现有字段 + `integrationRoot?: string` | `integrationRoot` 仅作用于 `integration_tests` 路径前缀 | 无 |
| `TestResolvePathsInput` | 现有字段 + `integration_root?: string` | MCP 层 snake_case；command 层映射为 `integrationRoot` | 无 |
| `testResolvePathsInputSchema` | 现有字段 + `integration_root: z.string().optional()` | 与 MCP 注册、command 入参一致 | 无 |
| `IntegrationTestEntry` | `scenario`, `test_file` | `test_file` 可为带子目录前缀的 POSIX 相对路径 | 无 |

---

## 路由/API 设计

本变更扩展现有 MCP 工具，无 HTTP 路由。

### MCP 工具: `test_resolve_paths`（增量字段）

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_resolve_paths` | 根据模块列表推导单元/集成测试路径；集成测试可指定 `__tests__/` 父目录 | 见下方 schema | 与变更前相同 | 无（本地 MCP） |

**输入 schema 增量**（`testResolvePathsInputSchema`）：

```typescript
{
  modules: z.array(z.string()).min(1),
  integration_scenarios: z.array(z.string()).optional(),
  extension: z.string().optional(),
  integration_root: z.string().optional(),  // 新增：__tests__/ 父目录，相对 project_root
  project_root: z.string().optional().nullable(),
}
```

**输出 schema**：不变。

**Handler 行为**（`mcp.ts` 现有实现，无需改 handler 签名）：

```typescript
async (args) => {
  const projectRoot = resolveProjectRoot(args.project_root);
  const result = runTestResolvePaths({ ...args, project_root: projectRoot });
  return jsonContent(testResolvePathsOutputSchema, result);
}
```

`integration_root` 通过 spread 自动透传；command 层负责 snake_case → camelCase 映射。

### 只读依赖: `test_detect_frameworks`

| 字段 | 用途 |
|------|------|
| `plan[].directory` | planner 将其作为 `integration_root` 传入 `test_resolve_paths`；与 `unit-test-executor` 执行 cwd 同源 |

本变更**不修改** `test_detect_frameworks` 的 schema 或实现。

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **新增可选参数 `integration_root`，表示 `__tests__/` 的父目录（相对 `project_root`）** | vitest/vite-plus 等框架的 `include: './__tests__/**/*.test.ts'` 相对于 plan 条目的 `directory` 解析；与 `per-directory-test-execution` 语义一致。 | **备选：在 `test_resolve_paths` 内部自动调用 `test_detect_frameworks` 并合并结果**。被拒绝，工具职责过重、难以测试；且单框架调用方无需额外检测开销。 |
| D2 | **多框架处理由 `test-design-planner` 编排，工具层保持无状态纯推导** | planner 已掌握 design.md 上下文，可按 plan 条目分组展示集成测试；多次调用结果可合并、去重。与 `add-test-path-resolver-api` 的「调用方控制输入精度」原则一致。 | **备选：`test_resolve_paths` 返回 `{ framework, directory, integration_tests }[]` 结构**。被拒绝，破坏现有 output schema，所有消费者需迁移。 |
| D3 | **未传或 `integration_root === "."` 时行为与变更前完全一致** | 单包项目、根目录 vitest 占绝大多数；避免 breaking change。 | **备选：默认 `integration_root` 为 cwd 相对路径**。被拒绝，MCP 工具无稳定 cwd 语义，且与 `project_root` 解析策略冲突。 |
| D4 | **规范化 `integrationRoot`：POSIX 分隔符 + 去除尾部 `/`** | 防止 `plugins/dev-team/bin/` 产生双斜杠；与现有 `toPosixRelativePath` 风格一致。 | **备选：原样拼接，由调用方保证格式**。被拒绝，planner/框架检测可能返回带尾斜杠路径，应在工具层防御。 |
| D5 | **`integration_root` 不影响 `unit_tests` 推导** | colocated 单元测试始终相对 `project_root` 与源文件同级；框架工作目录仅影响 `__tests__/` 树的位置。 | **备选：单元测试也前缀 `integration_root`**。被拒绝，与 test-gen colocated 约定及现有 executor 行为冲突。 |
| D6 | **单元测试路径仍单次 `test_resolve_paths` 调用；集成测试按 plan 条目多次调用** | 避免对无集成场景的项目增加 `test_detect_frameworks` 调用；单元路径与集成路径解耦。 | **备选：每次集成都重复解析 unit_tests 并丢弃**。被拒绝，冗余且易在合并时引入不一致。 |
| D7 | **`integration_root` 路径穿越：规范化后若含 `..` 段则写入 `errors`（可选增强）** | proposal 风险表提及；与 `isWithinProjectRoot` 对 `modules` 的防护对称。若实现成本过高，首版可仅做尾部斜杠规范化，穿越防护留待 follow-up。 | **备选：静默接受 `../outside/__tests__/...`**。被拒绝，可能生成 project_root 外路径，误导 test-gen。 |

---

## 依赖

### 运行时依赖

- 无新增 npm 依赖。变更限于现有 `test-resolve-paths.ts`、schema 与 agent prompt。

### 构建/测试依赖

- `vite-plus/test` — 单元测试（`plugins/dev-team/bin`）
- `zod/v4` — schema 单元测试

### 逻辑依赖（只读，不修改）

- `test_detect_frameworks` — planner 读取 `plan[].directory`
- `test_get_framework_config` — 不变
- `unit-test-executor` / `integration-test-executor` — 已在各 `directory` 下执行，本变更对齐路径约定
- `add-test-path-resolver-api` 实现的 `deriveUnitTestPath`、`inferExtension`、`resolveTestPaths` 主体逻辑

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `integration_root` 与 vitest `root`/`include` 仍不一致 | 集成测试路径无法被框架发现 | 低 | planner 直接使用 `test_detect_frameworks` plan 的 `directory`，与 executor 同源 |
| 多框架多次调用产生重复集成场景行 | test-design.md 表格冗余 | 中 | planner 按框架/目录分组展示，或去重相同 `test_file` |
| 调用方忘记传 `integration_root` | monorepo 子包仍生成根级 `__tests__/` | 中 | 更新 planner 强制流程；AC-9 静态检查；未传时保持旧行为不破坏单包项目 |
| `integration_root` 路径穿越 | 生成 project_root 外路径 | 低 | 规范化并拒绝含 `..` 的值（D7）；与 `modules` 路径校验一致 |
| MCP handler spread 遗漏新字段 | 参数未生效 | 低 | 现有 `{ ...args }` 已透传；AC-8 代码审查 + schema 测试 |

---

## 迁移步骤

无数据迁移。向后兼容增量部署：

1. 更新 `test-resolve-paths.schema.ts` 增加 `integration_root`
2. 更新 `test-resolve-paths.ts`：`deriveIntegrationTestPath` 签名、`ResolveTestPathsParams`、`runTestResolvePaths` 映射
3. 确认 `mcp.ts` handler 透传（通常无需改 handler 体，可更新 tool description）
4. 更新 `test-design-planner.md` Process
5. 补充单元测试与 schema 测试；可选 agent 静态检查
6. 升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号
7. 运行 `npm run build` 重新打包

现有仅使用 `modules` / `integration_scenarios` 的调用方无需变更。

---

## 待决问题

- **D7 实现范围**：首版是否在 `resolveTestPaths` 中对非法 `integration_root`（含 `..`）写入 `errors` 并跳过集成路径生成，还是仅做尾部斜杠规范化？建议首版实现轻量校验以覆盖 AC-5，与 spec 风险表一致。
- **多框架 test-design.md 表格结构**：合并 `integration_tests` 时是否在表格增加「框架 / 工作目录」列，还是按目录分节？由 planner 实现细节决定，不影响 MCP 契约。
- **`test_detect_frameworks` 返回绝对路径 `directory` 时**：当前 spec 假定相对 `project_root` 的 POSIX 路径；若 plan 条目为绝对路径，planner 是否应先转为相对路径再传入 `integration_root`？需与 `test-detect-frameworks` 实际输出对齐（本变更不修改 detect 工具）。
