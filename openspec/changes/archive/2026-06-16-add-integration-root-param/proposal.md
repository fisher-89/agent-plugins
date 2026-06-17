# 提案: add-integration-root-param

> **变更**: add-integration-root-param
> **日期**: 2026-06-16
> **状态**: 起草中

---

## 问题

`test_resolve_paths` MCP 工具当前将集成测试路径生成为相对于 `project_root` 的 `__tests__/<scenario>/<scenario>.test.<ext>`。然而，测试框架（如 vitest）的实际工作目录来自 `test_detect_frameworks` 返回的 plan 条目中的 `directory` 字段，其 `include` 配置期望 `__tests__/` 相对于**该工作目录**，而非项目根目录。

典型场景：在 monorepo 中 vitest 从 `plugins/dev-team/bin/` 运行，配置为 `./__tests__/**/*.test.ts`，则集成测试应位于 `plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts`，而不是 `<project_root>/__tests__/api-flow/api-flow.test.ts`。

后果：

1. **test-design.md 路径错误**：`test-design-planner` 填写的集成测试 `测试文件` 列与 vitest 实际扫描目录不一致，导致 test-gen 写入位置与框架执行器期望不符。
2. **多框架项目无法区分根目录**：`test_detect_frameworks` 可为不同框架返回不同 `directory`（如根目录 vitest 与 `plugins/dev-team/bin` 下的 vite-plus），但 `test_resolve_paths` 无法按框架工作目录分别推导集成测试路径。
3. **与 per-directory 测试执行语义断裂**：`unit-test-executor` 已在各 plan 条目的 `directory` 下执行命令，路径解析器仍假设集成测试一律在项目根 `__tests__/`，形成执行与路径约定的不一致。

---

## 提案

为 `test_resolve_paths` 新增可选参数 `integration_root`，表示 `__tests__/` 的父目录（相对于 `project_root`）。当该参数提供且不为 `"."` 时，集成测试路径前缀为 `<integration_root>/__tests__/...`；未提供或为 `"."` 时保持现有行为（向后兼容）。

### 路径规则（更新后）

| 条件 | 集成测试路径 |
|------|-------------|
| 未传 `integration_root` 或 `integration_root` 为 `"."` | `__tests__/<scenario>/<scenario>.test.<ext>` |
| `integration_root` 为 `"plugins/dev-team/bin"` | `plugins/dev-team/bin/__tests__/<scenario>/<scenario>.test.<ext>` |

### 实现结构

- `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` — 输入 schema 增加 `integration_root?: string`
- `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` — `ResolveTestPathsParams` / `deriveIntegrationTestPath` 接受 `integrationRoot`；`resolveTestPaths` 与 `runTestResolvePaths` 透传
- `plugins/dev-team/bin/src/mcp.ts` — handler 将 `integration_root` 传入 `runTestResolvePaths`
- `plugins/dev-team/agents/test-design-planner.md` — 多框架时先调用 `test_detect_frameworks`，再对每个 plan 条目调用 `test_resolve_paths` 并传入该条目的 `directory` 作为 `integration_root`

### 设计决策

- **参数命名**：`integration_root`（`__tests__/` 的父目录，相对 `project_root`）
- **多框架处理**：由调用方（`test-design-planner`）对每个 plan 条目分别调用 `test_resolve_paths`，工具内部不做框架自动检测
- **向后兼容**：未传或 `"."` 时行为与变更前完全一致

---

## 能力

### 新增能力

（无）

### 修改的能力

- **test-path-resolver** — `test_resolve_paths` 与 `deriveIntegrationTestPath` 支持可选 `integration_root`，按框架工作目录前缀集成测试路径
- **phase-agents** — `test-design-planner` 在多框架场景下结合 `test_detect_frameworks` 的 `directory` 调用 `test_resolve_paths`

---

## 变更范围

### 实现以下特性

1. **Schema**：`testResolvePathsInputSchema` 增加可选字段 `integration_root`（字符串，描述为 `__tests__/` 父目录，相对 `project_root`）
2. **`deriveIntegrationTestPath`**：签名增加可选 `integrationRoot`；非 `"."` 时在 `__tests__/...` 前加 `<integrationRoot>/`；路径使用 POSIX `/` 分隔符
3. **`resolveTestPaths` / `runTestResolvePaths`**：从入参读取 `integration_root` 并传给集成路径推导
4. **MCP 注册**：`mcp.ts` 中 `test_resolve_paths` handler 透传 `integration_root`
5. **Agent 集成**：更新 `test-design-planner.md` Process——存在集成测试场景时先 `test_detect_frameworks`，对每个 plan 条目以 `directory` 为 `integration_root` 调用 `test_resolve_paths`，合并各次调用的 `integration_tests` 写入 test-design.md
6. **单元测试**：覆盖 `integration_root` 为子目录、`"."`、未传时的路径推导及向后兼容

### 不要修改

- `test_detect_frameworks` 与 `test_get_framework_config` 的行为与 schema（仅作为 planner 只读引用 `directory`）
- 单元测试（colocated）路径推导规则
- `test-gen-generator`、`unit-test-executor`、`integration-test-executor` 的执行逻辑
- Python 工具 `test-scope.py`、`test-generator.py`
- 集成测试扩展名推断逻辑（`extension` 参数与 modules 众数推断）

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | 未传 `integration_root` 时，`integration_scenarios: ["api-flow"]` 仍返回 `__tests__/api-flow/api-flow.test.ts` | 单元测试 |
| AC-2 | `integration_root: "."` 时行为与 AC-1 相同 | 单元测试 |
| AC-3 | `integration_root: "plugins/dev-team/bin"` 时返回 `plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts` | 单元测试 |
| AC-4 | `deriveIntegrationTestPath("api-flow", "ts", "plugins/dev-team/bin")` 返回带前缀的 POSIX 路径 | 单元测试 |
| AC-5 | `integration_root` 含尾部斜杠时规范化后路径正确（无重复斜杠） | 单元测试 |
| AC-6 | `integration_root` 不影响 `unit_tests` 推导结果 | 单元测试 |
| AC-7 | `testResolvePathsInputSchema` 接受可选 `integration_root` 字段 | schema 单元测试 |
| AC-8 | `mcp.ts` 注册的 `test_resolve_paths` input schema 包含 `integration_root` | 代码审查 |
| AC-9 | `test-design-planner.md` Process 描述多框架下按 plan 条目 `directory` 传入 `integration_root` | 静态检查 |
| AC-10 | `plugins/dev-team/bin` 下 `npm run build` 无错误 | 本地构建 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| `integration_root` 与 vitest `root`/`include` 配置仍不一致 | 集成测试路径仍无法被框架发现 | 低 | planner 直接使用 `test_detect_frameworks` plan 的 `directory`，与 executor 同源 |
| 多框架多次调用产生重复集成场景行 | test-design.md 表格冗余 | 中 | planner 按框架/目录分组展示，或去重相同 `test_file` |
| 调用方忘记传 `integration_root` | monorepo 子包仍生成根级 `__tests__/` 路径 | 中 | 更新 planner 强制流程；AC-9 静态检查；未传时保持旧行为不破坏单包项目 |
| `integration_root` 路径穿越 | 生成 project_root 外路径 | 低 | 可选：规范化并拒绝含 `..` 的值；spec 要求路径仍相对 `project_root` 且不含 `..` 段 |
