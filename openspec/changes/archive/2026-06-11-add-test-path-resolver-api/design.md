# 设计: add-test-path-resolver-api

> **变更**: add-test-path-resolver-api
> **日期**: 2026-06-10
> **基于**: proposal.md, specs/test-path-resolver/spec.md, specs/phase-agents/spec.md

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| `test-resolve-paths.schema.ts` | 定义 `test_resolve_paths` MCP 工具的 Zod 输入/输出 schema；`modules` 最少 1 项 | `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` | `zod/v4` | TypeScript / Zod |
| `resolveTestPaths()` | 核心逻辑：展开目录、按语言规则推导单元/集成测试路径、收集错误；路径命名规则为确定性纯函数 | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `fs`, `path` | TypeScript |
| `runTestResolvePaths()` | MCP 命令入口：解析 `project_root` 后委托 `resolveTestPaths()` | `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` | `resolveTestPaths()`, `resolveProjectRoot` 逻辑 | TypeScript |
| `mcp.ts` 工具注册 | 注册 `test_resolve_paths` 工具，绑定 schema 与 handler | `plugins/dev-team/bin/src/mcp.ts` | `runTestResolvePaths`, schema | TypeScript / MCP SDK |
| `schemas/index.ts` | 重新导出 `testResolvePathsInputSchema` / `testResolvePathsOutputSchema` | `plugins/dev-team/bin/src/schemas/index.ts` | schema 文件 | TypeScript |
| `test-design-planner` 代理 | Process 中调用 `test_resolve_paths`，用返回值填充 test-design.md 的 `测试文件` 列；禁止手工拼接路径 | `plugins/dev-team/agents/test-design-planner.md` | MCP 工具 `test_resolve_paths` | Markdown prompt (Agent) |

### 组件图

```
test-design-planner (Agent)
  |-- Read proposal.md, design.md
  |-- Grep 源码
  |-- CallMcpTool: test_resolve_paths
  |       modules, integration_scenarios?, extension?
  v
MCP Server (mcp.ts)
  |-- resolveProjectRoot(project_root?)
  |-- runTestResolvePaths(args)
  v
commands/test-resolve-paths.ts
  |-- resolveTestPaths()
  |     |-- 逐条处理 modules[]
  |     |     |-- 目录 → expandDirectory() [fs 递归遍历]
  |     |     |-- 文件 → 校验 + deriveUnitTestPath() [纯函数]
  |     |-- integration_scenarios[] → deriveIntegrationTestPath() [纯函数]
  |     |-- inferExtension() [众数推断]
  |     |-- 排序去重 unit_tests / integration_tests / errors
  v
JSON 响应
  |-- unit_tests:   { source, test_file }[]
  |-- integration_tests: { scenario, test_file }[]
  |-- errors:       { path, message }[]

（不变）test_detect_frameworks / test_get_framework_config
（不变）test-gen-generator / test-gen-evaluator colocated 约定
```

---

## 数据流

### 流程描述

1. **`test-design-planner`** 从 design.md 变更范围与 Grep 结果汇总精确 `modules` 列表（文件或目录，相对项目根），并从 proposal/design 识别 `integration_scenarios`（若有）。
2. Agent 调用 MCP 工具 `test_resolve_paths`，传入 `modules`、可选 `integration_scenarios`、`extension`、`project_root`。
3. **`mcp.ts` handler** 通过 `resolveProjectRoot()` 解析绝对项目根，调用 `runTestResolvePaths()`。
4. **`resolveTestPaths()`** 对每条 `modules` 条目：
   - 将路径规范为相对 `project_root` 的 POSIX 风格（`/` 分隔符）
   - 校验路径在 `project_root` 内（防路径穿越）
   - 若路径不存在 → 写入 `errors`，继续下一条
   - 若为目录 → `expandDirectory()` 深度优先递归收集可测试源文件（跳过排除目录/文件）
   - 若为文件 → 校验扩展名与是否已是测试文件；无效则写入 `errors`
   - 对每条有效源文件调用 `deriveUnitTestPath()` 生成同级目录单元测试路径
5. 若提供 `integration_scenarios`，按优先级解析 `<ext>`（显式 `extension` → modules 扩展名众数 → 默认 `ts`），为每个场景生成 `__tests__/<scenario>/<scenario>.test.<ext>`。
6. 对 `unit_tests`（按 `source`）、`integration_tests`（按 `scenario`）、`errors`（按 `path`）字典序排序；`unit_tests` 按 `source` 去重。
7. Agent 将 `unit_tests` 映射到 test-design.md `验收范围` 与 `单元测试 > 用例` 的 `测试文件` 列；将 `integration_tests` 映射到 `集成测试 > 用例`；将 `errors` 记入 `不可测试项`。

### 路径推导规则（权威约定）

| 源文件扩展名 | 单元测试文件名 | 目录 |
|-------------|---------------|------|
| `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | `<basename>.test<ext>` | 与源文件同级 |
| `.py` | `test_<basename>.py` | 与源文件同级 |
| `.go` | `<basename>_test.go` | 与源文件同级 |
| `.rs` | `<basename>_test.rs` | 与源文件同级 |

| 集成测试 | 路径模式 |
|---------|---------|
| 场景 `api-flow`，扩展名 `ts` | `__tests__/api-flow/api-flow.test.ts` |

### 目录展开排除规则

**跳过目录名**：`node_modules`、`.git`、`dist`、`build`、`coverage`、`.nyc_output`、`target`

**跳过文件**：
- 扩展名不在可测试源文件集合内
- 已是测试文件：`*.test.*`、`test_*.py`、`*_test.go`、`*_test.rs`
- 文档/配置：`.md`、`.json`、`.yaml`、`.yml`、`.txt`、`.lock`

### 数据模型

| 模型 | 字段 | 关系 | 持久化 |
|------|------|------|--------|
| `ResolveTestPathsParams` | `projectRoot: string`<br>`modules: string[]`<br>`integrationScenarios?: string[]`<br>`extension?: string` | `modules` 每项可为文件或目录；目录展开后产生多条 `unit_tests` | 无 |
| `UnitTestEntry` | `source: string`<br>`test_file: string` | 一对一：每个源文件对应一个规定测试路径 | 无 |
| `IntegrationTestEntry` | `scenario: string`<br>`test_file: string` | 每个场景对应一个集成测试路径 | 无 |
| `ResolveError` | `path: string`<br>`message: string` | 解析失败的模块条目；不阻断其余条目 | 无 |
| `ResolveTestPathsResult` | `unit_tests: UnitTestEntry[]`<br>`integration_tests: IntegrationTestEntry[]`<br>`errors: ResolveError[]` | 三个数组始终存在（可为空数组） | 无 |

---

## 路由/API 设计

本变更仅新增 MCP 工具，无 HTTP 路由。

### MCP 工具: `test_resolve_paths`

| 方法 | 路径 | 描述 | 输入 | 输出 | 认证 |
|------|------|------|------|------|------|
| MCP 工具调用 | `test_resolve_paths` | 根据模块列表（文件或目录）推导规定的单元测试与集成测试文件路径 | `{ modules: string[] (min 1), integration_scenarios?: string[], extension?: string, project_root?: string }` | `{ unit_tests: {source, test_file}[], integration_tests: {scenario, test_file}[], errors: {path, message}[] }` | 无（本地 MCP） |

**输入 schema**（`testResolvePathsInputSchema`）：

```typescript
{
  modules: z.array(z.string()).min(1),
  integration_scenarios: z.array(z.string()).optional(),
  extension: z.string().optional(),       // 可带或不带前导 "."
  project_root: z.string().optional().nullable(),
}
```

**输出 schema**（`testResolvePathsOutputSchema`）：

```typescript
{
  unit_tests: z.array(z.object({ source: z.string(), test_file: z.string() })),
  integration_tests: z.array(z.object({ scenario: z.string(), test_file: z.string() })),
  errors: z.array(z.object({ path: z.string(), message: z.string() })),
}
```

**Handler 行为**：

```typescript
async (args) => {
  const projectRoot = resolveProjectRoot(args.project_root);
  const result = runTestResolvePaths({ ...args, projectRoot });
  return jsonContent(result);
}
```

**校验失败**：`modules` 为空数组时由 Zod `.min(1)` 在 MCP 层拒绝，不返回空成功结果（AC-10）。

---

## 决策

| ID | 决策 | 理由 | 备选方案 |
|----|------|------|----------|
| D1 | **在 TypeScript MCP 层实现路径解析，作为 test-design 阶段的权威来源** | 与现有 `test_detect_frameworks`、`test_get_framework_config` 同属 `test_` 域名工具族；Agent 通过 MCP 直接消费，无需跨语言调用。命名规则与 `test-gen-generator` colocated 表一致，消除 test-design 与 test-gen 路径语义断层。 | **备选：扩展 Python `test-scope.py` / `test-generator.py`**。被拒绝，Python 工具未被 MCP Agent 直接调用，且本次范围明确为 TypeScript MCP 层；Python 对齐留待后续变更。 |
| D2 | **`deriveUnitTestPath()` 为独立纯函数，`resolveTestPaths()` 负责编排与 fs 遍历** | 路径命名公式（`<name>.test.ts`、`test_<name>.py` 等）无外部依赖，可单独单元测试覆盖 AC-1~AC-5。目录展开需同步 `fs` 读取，不宜与命名逻辑混在同一不可测试的块中。 | **备选：全部内联在 `runTestResolvePaths` 中**。被拒绝，无法对多语言命名规则做细粒度测试，且违反单一职责。 |
| D3 | **错误收集模式：单条失败写入 `errors` 并继续处理** | design.md 常以目录粒度描述范围，部分子路径可能不存在或不可测试；planner 需要尽可能多地获得有效路径，同时将失败项记入 `不可测试项`。 | **备选：遇错即抛异常终止**。被拒绝，一条无效路径会导致整个 coverage map 无法生成，降低 planner 可用性。 |
| D4 | **集成测试扩展名：显式 `extension` > modules 扩展名众数 > 默认 `ts`** | 多语言 monorepo 中 modules 可能混合扩展名；planner 可根据 design.md 技术栈显式传入。无有效源文件时默认 `ts` 与项目主栈一致。 | **备选：始终要求显式 `extension`**。被拒绝，增加 planner 负担；多数单语言变更可从 modules 自动推断。 |
| D5 | **集成测试路径固定为 `__tests__/<scenario>/<scenario>.test.<ext>`** | 与 `phase-unit-test` Glob 模式及 dev-team 工作流隐含约定一致；场景名同时作为目录名与文件名前缀，便于定位。 | **备选：`tests/integration/<scenario>.test.<ext>`**。被拒绝，与现有 `unit-test-executor` / `phase-unit-test` 中 `__tests__/**` 约定不一致，需同步修改多个消费者。 |
| D6 | **本次不修改 Python 工具与 test-gen 代理** | 提案明确 out-of-scope；`test-gen-generator` 已按 colocated 约定写文件，本工具与之规则对齐即可。避免扩大变更范围。 | **备选：同步修改 `test-scope.py` 并统一调用链**。被拒绝，超出本次验收范围，增加回归风险。 |
| D7 | **`modules` 空数组在 Zod schema 层拒绝（`.min(1)`）** | 空 modules 无业务语义，返回空成功结果会误导 planner 认为无需测试覆盖。与 `test_detect_frameworks` 等工具的输入校验风格一致。 | **备选：在 `resolveTestPaths` 内返回空结果**。被拒绝，无法通过 MCP schema 契约强制调用方传入有效输入。 |

---

## 依赖

### 运行时依赖

- 无新增 npm 依赖。目录遍历使用 Node.js 内置 `fs` / `path`；schema 使用已有 `zod/v4`；MCP 注册使用已有 `@modelcontextprotocol/sdk`。

### 构建/测试依赖

- `vite-plus/test` — 单元测试框架（`plugins/dev-team/bin` 现有测试基础设施）
- `zod/v4` — input/output schema 定义与 schema 单元测试

### 逻辑依赖（只读，不修改）

- `resolveProjectRoot()` — `mcp.ts` 现有实现（`CLAUDE_PROJECT_DIR` 或 `process.cwd()`）
- `test-gen-generator.md` colocated 命名表 — 规则对齐参考
- `test-design.md.template` — `测试文件` 列占位符（模板本身不变，由 planner 填充）

---

## 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| TypeScript 与 Python 工具命名规则不一致 | 不同工作流阶段路径不一致 | 中 | 以 `test-resolve-paths.ts` 为权威；代码注释标注 Python 工具待后续对齐；AC-1~AC-5 覆盖主要语言 |
| 目录展开包含非变更文件 | test-design.md 列出过多测试文件 | 中 | planner 仅传入 design.md 精确模块列表；工具不做智能过滤，由调用方控制输入精度 |
| 集成测试扩展名推断错误 | 集成测试路径扩展名与项目实际不符 | 低 | 支持显式 `extension`；planner 根据 design.md 技术栈传入 |
| 与 test-gen-evaluator G1/G2 路径期望漂移 | evaluator 因路径格式 fail | 低 | 规则与 `test-gen-generator` colocated 表一致；Rust 使用 `_test.rs`（与 generator 文档 `<module>_test.rs` 存在历史差异，本变更以 spec/proposal 为准） |
| 路径穿越攻击 | 解析 `../../../etc/passwd` 类输入 | 低 | 所有路径 resolve 后校验位于 `project_root` 之下；越界写入 `errors` |

---

## 迁移步骤

无数据迁移。一次性增量部署：

1. 新增 `test-resolve-paths.schema.ts` 与 `test-resolve-paths.ts`
2. 在 `schemas/index.ts` 与 `mcp.ts` 注册导出
3. 更新 `test-design-planner.md` Process 步骤
4. 升级 `plugins/dev-team/.claude-plugin/plugin.json` 版本号
5. 运行 `npm run build` 重新打包 `dev-team-mcp.cjs`

现有 `test_detect_frameworks`、`test_get_framework_config` 调用方无需变更。

---

## 待决问题

- Rust 测试文件命名：generator 文档写 `<module>_test.rs`，本变更 spec 规定 `<module>_test.rs`。实现以 spec 为准；是否后续统一 generator 文档待产品确认。
- 目录展开是否应支持 `.java` 等 generator 可处理但本工具未列出的扩展名？本次严格限定 spec 中的七种扩展名集合；扩展语言需单独变更。
- Python 工具（`test-scope.py`、`test-generator.py`）何时与本 MCP 工具对齐，由后续变更决定。
