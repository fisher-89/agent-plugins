# 提案: add-test-path-resolver-api

> **变更**: add-test-path-resolver-api
> **日期**: 2026-06-10
> **状态**: 起草中

---

## 问题

当前 dev-team 工作流在测试设计阶段（`test-design-planner`）需要为 proposal/design 中涉及的每个模块填写 `测试文件` 列，但缺少统一的、确定性的路径推导机制：

1. **路径规则分散在多处**：单元测试共存（colocated）规则写在 `test-gen-generator` 代理定义和 Python 工具（`test-generator.py`、`test-scope.py`）中，集成测试路径约定（`__tests__/<场景>/`）仅隐含在 phase-unit-test 的 Glob 模式中，没有单一权威来源。
2. **Agent 手工推导易出错**：`test-design-planner` 需根据模块路径自行拼接 `*.test.ts` 或 `test_*.py` 等文件名，容易因语言/扩展名差异产生错误路径，导致 test-design.md 的 coverage map 与 test-gen 实际输出不一致。
3. **目录级模块输入无标准展开**：design.md 常以目录粒度描述变更范围（如 `plugins/dev-team/bin/src/commands/`），Agent 需自行遍历目录猜测受影响的源文件，缺乏与 test-gen 一致的展开逻辑。
4. **与现有 test MCP 工具未衔接**：已有 `test_detect_frameworks` 和 `test_get_framework_config` 解决框架检测与命令配置，但缺少「模块 → 测试文件路径」这一环，test-design 与 test-gen 之间仍存在路径语义断层。

需要在 dev-team MCP 中新增确定性 API，传入精确模块列表（文件或目录），返回符合项目约定的单元测试与集成测试文件路径，并由 `test-design-planner` 消费该 API 填充 test-design.md。

---

## 提案

在 dev-team MCP 服务器中新增 `test_resolve_paths` 工具，接受模块路径列表（文件或目录）及可选的集成测试场景名列表，返回按语言规则推导的测试文件路径。

### 路径规则（权威约定）

| 测试类型 | 规则 | 示例 |
|---------|------|------|
| 单元测试 | 与被测源文件**同级目录**，按扩展名命名 | `src/config.ts` → `src/config.test.ts`；`src/component/Button.tsx` → `src/component/Button.test.tsx`；`src/auth.py` → `src/test_auth.py`；`src/handler.go` → `src/handler_test.go`；`src/lib.rs` → `src/lib_test.rs` |
| 集成测试 | `<ROOT>/__tests__/<场景>/<场景>.test.<ext>` | 场景 `api-flow`、扩展名 `.ts` → `__tests__/api-flow/api-flow.test.ts` |

### 实现结构

遵循现有 test MCP 工具模式：

- `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` — 核心纯函数 `resolveTestPaths()` 与 `runTestResolvePaths()` 入口
- `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` — Zod input/output schema
- `plugins/dev-team/bin/src/mcp.ts` — 注册 `test_resolve_paths` 工具
- `plugins/dev-team/agents/test-design-planner.md` — Process 中新增调用 `test_resolve_paths` 步骤，用返回路径填充 test-design.md 的 `测试文件` 列

目录输入 SHALL 递归展开为源文件列表（排除已有测试文件、配置/文档文件及 `node_modules`、`.git`、`dist` 等目录），再对每份源文件推导单元测试路径。

集成测试路径的 `<ext>` SHALL 由 `extension` 可选参数指定；未指定时从模块列表中推断主扩展名（多数票），无法推断时默认 `.ts`。

---

## 能力

### 新增能力

- **test-path-resolver** — MCP 工具 `test_resolve_paths`：接收模块列表与集成场景，返回单元测试与集成测试的规定路径；核心逻辑为确定性纯函数，不访问网络

### 修改的能力

- **phase-agents** — `test-design-planner` 在编写 test-design.md 前调用 `test_resolve_paths` 获取测试文件列表，禁止手工拼接测试路径

---

## 变更范围

### 实现以下特性

1. **`test_resolve_paths` MCP 工具**：输入 `{ modules: string[], integration_scenarios?: string[], extension?: string, project_root?: string }`；输出 `{ unit_tests: {source, test_file}[], integration_tests: {scenario, test_file}[], errors: {path, message}[] }`
2. **单元测试路径推导**：对 `.ts/.tsx/.js/.jsx/.mjs/.cjs` 使用 `<name>.test<ext>`；对 `.py` 使用 `test_<name>.py`；对 `.go` 使用 `<name>_test.go`；对 `.rs` 使用 `<name>_test.rs`；测试文件与源文件同目录
3. **目录展开**：目录路径递归收集可测试源文件；跳过 `*.test.*`、`test_*`（非源）、`*_test.go`、`*_test.rs` 等已有测试文件
4. **集成测试路径推导**：每个 `integration_scenarios` 条目生成 `__tests__/<scenario>/<scenario>.test.<ext>`（相对 `project_root`）
5. **错误收集**：路径不存在、非源文件、路径越出项目根等情形写入 `errors` 数组，不中断其余条目的解析
6. **Agent 集成**：更新 `test-design-planner.md`，在 Grep 源码后调用 MCP 工具，将 `unit_tests` / `integration_tests` 映射到 test-design.md 的 `验收范围` 与 `用例` 表格 `测试文件` 列
7. **单元测试**：为 `resolveTestPaths()` 覆盖各语言命名、目录展开、集成场景、边界输入

### 不要修改

- `test_detect_frameworks` 与 `test_get_framework_config` 的行为与 schema
- `test-gen-generator`、`test-gen-evaluator` 代理定义（路径规则由本工具统一，generator 仍按既有 colocated 约定写文件）
- `phase-test-design` 技能编排（仍为 P→E 薄编排层）
- Python 工具 `test-scope.py`、`test-generator.py`（本次仅在 TypeScript MCP 层实现，后续可对齐）
- 集成/单元测试执行器（`unit-test-executor`、`integration-test-executor`）的执行逻辑
- OpenSpec 核心 workflow 阶段定义

---

## 验收标准

| ID | 验收条件 | 验证方法 |
|----|---------|----------|
| AC-1 | `test_resolve_paths` 对 `src/config.ts` 返回单元测试路径 `src/config.test.ts` | 单元测试 + MCP 工具调用 |
| AC-2 | `test_resolve_paths` 对 `src/component/Button.tsx` 返回 `src/component/Button.test.tsx` | 单元测试 |
| AC-3 | `test_resolve_paths` 对 `src/auth.py` 返回 `src/test_auth.py` | 单元测试 |
| AC-4 | `test_resolve_paths` 对 `src/handler.go` 返回 `src/handler_test.go` | 单元测试 |
| AC-5 | `test_resolve_paths` 对 `src/lib.rs` 返回 `src/lib_test.rs` | 单元测试 |
| AC-6 | 传入目录 `src/commands/` 时展开目录内所有源文件并返回对应单元测试路径列表 | 单元测试（含多文件 fixture） |
| AC-7 | `integration_scenarios: ["api-flow"]` 且 `extension: "ts"` 时返回 `__tests__/api-flow/api-flow.test.ts` | 单元测试 |
| AC-8 | 未传 `extension` 时从 `modules` 中源文件扩展名推断集成测试扩展名 | 单元测试 |
| AC-9 | 不存在的路径、非源文件路径写入 `errors` 且其余有效条目仍正常返回 | 单元测试 |
| AC-10 | `modules` 为空数组时返回校验错误（不返回空成功结果） | 单元测试 + Zod schema 校验 |
| AC-11 | `mcp.ts` 注册 `test_resolve_paths`，input/output schema 与 spec 一致 | 代码审查 + schema 测试 |
| AC-12 | `test-design-planner.md` Process 包含调用 `test_resolve_paths` 的步骤，并用返回路径填充 `测试文件` 列 | 静态检查测试 |
| AC-13 | 构建通过：`plugins/dev-team/bin` 下 `npm run build` 无错误 | CI / 本地构建 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| TypeScript 与 Python 工具命名规则不一致（如 Rust `_tests` vs `_test`） | 不同工作流阶段路径不一致 | 中 | spec 明确以 `test-resolve-paths.ts` 为权威；文档注释引用 Python 工具待后续对齐 |
| 目录展开包含非变更文件 | test-design.md 列出过多测试文件 | 中 | planner 仍只传入 design.md 精确模块列表；工具不做「智能过滤」，由调用方控制输入精度 |
| 集成测试扩展名推断错误 | 集成测试路径扩展名与项目实际不符 | 低 | 支持显式 `extension` 参数；planner 可根据 design.md 技术栈传入 |
| 与 test-gen-evaluator G1/G2 路径期望漂移 | evaluator 因路径格式 fail | 低 | 规则与 `test-gen-generator` colocated 表一致；AC 覆盖主要语言 |
