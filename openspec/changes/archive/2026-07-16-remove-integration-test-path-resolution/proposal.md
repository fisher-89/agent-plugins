# 提案: 移除 test_resolve_paths 集成测试路径解析功能

> **变更**: remove-integration-test-path-resolution
> **日期**: 2026-07-15
> **状态**: 草案

---

## 问题

`test_resolve_paths` MCP 工具当前承担了两项职责：单元测试路径推导和集成测试路径推导。集成测试路径解析通过 `integration_scenarios` 和 `integration_root` 两个参数实现，根据场景名和扩展名在 `__tests__/<scenario>/` 目录下生成路径。

实际使用中发现以下问题：

1. **职责耦合**：集成测试路径生成是 Plan 层级的决策（subagent 需自行决定在哪个目录下创建 `__tests__` 子目录），不应由路径解析工具代劳。路径解析工具应仅专注于"给定源文件，推导同级单元测试文件"这一职责。

2. **场景局限性**：当前集成测试路径格式 `__tests__/<scenario>/<scenario>.test.<ext>` 过于僵化，无法覆盖 subagent 实际需要的灵活路径结构。subagent 更倾向于在具体的 plan entry 目录下创建 `__tests__/` 子目录并自行管理测试文件。

3. **无实际调用方**：当前所有使用 `test_resolve_paths` 的场景都只使用 `unit_tests` 部分，`integration_tests` 输出未被任何上游消费。

下游 subagent（test-gen-generator, implementation-generator 等）应当自行在 plan entry 的 `__tests__` 目录下创建集成测试文件，而非依赖中心化的路径推导逻辑。

---

## 提案

从 `test_resolve_paths` MCP 工具及其底层函数中移除与集成测试路径解析相关的全部代码：

1. **移除输入参数**：
   - 移除 `integration_scenarios`（`string[]`，可选）
   - 移除 `integration_root`（`string`，可选）
   - 移除 `extension`（`string`，可选）——该参数仅被集成测试路径解析使用

2. **移除输出字段**：
   - 移除 `integration_tests`（输出对象中不再包含该字段）

3. **移除相关类型、函数与逻辑**：
   - 移除 `IntegrationTestEntry` 接口
   - 移除 `deriveIntegrationTestPath()` 函数
   - 移除 `normalizeIntegrationRoot()` 函数
   - 移除 `isValidIntegrationRoot()` 函数
   - 移除 `inferExtension()` 函数
   - 移除 `normalizeExtension()` 函数
   - 移除 `resolveIntegrationTests()` 函数

4. **保持单元测试路径推导完全不变**：`modules` 参数（含 `"git-change"` 模式）、config-driven 扫描、test config 过滤、exclude 过滤等单元测试路径解析逻辑不受影响。

5. **Subagent 自行管理集成测试**：subagent 需在各自的 plan entry 目录下创建 `__tests__/` 子目录并放置测试文件，不再依赖中心化路径推导。

---

## 能力

### 修改的能力

- **test-path-resolver** — 移除集成测试路径解析相关参数（`integration_scenarios`、`integration_root`、`extension`）、输出字段（`integration_tests`）及全部内部实现函数

---

## 变更范围

### 实现文件

- `plugins/dev-team/bin/src/commands/test-resolve-paths.ts` — 移除集成测试路径解析相关的类型、函数、导入、调用逻辑
- `plugins/dev-team/bin/src/schemas/test-resolve-paths.schema.ts` — 移除 `integration_scenarios`、`integration_root`、`extension` 输入字段；移除 `integration_tests` 输出字段
- `plugins/dev-team/bin/src/mcp.ts` — 更新 `registerTestResolvePathsTool` 的 description 文本（不再提及集成测试功能）

### Subagent Prompt 文件

- `plugins/dev-team/agents/test-design-planner.md` — 重写集成测试路径相关步骤，subagent 不再调用 `test_resolve_paths` 获取 `integration_tests`，改为自行在 plan entry 目录下创建 `__tests__/<scenario>/` 目录并管理测试文件

  具体变更点：
  - **Step 7**：不再识别"场景名列表"（用于传给 `integration_scenarios`），改为识别需要集成测试覆盖的场景并直接确定 `__tests__` 放置目录
  - **Step 8**：移除"不传 `integration_root`"的说明，`test_resolve_paths` 调用仅保留 `modules` 参数用于单元测试路径
  - **Step 9**：删除整个"集成测试路径"段落（原 lines 20-24），不再对每个 plan entry 调用 `test_resolve_paths` 传 `integration_scenarios`/`integration_root`。替换为：subagent 在 plan entry 的 `directory` 下创建 `__tests__/<scenario>/` 目录，自行命名测试文件
  - **Step 11**（原 line 26）：删除"将合并后的 `integration_tests` 映射到集成测试表格"——不再有 `integration_tests` 输出
  - **Step 12**（原 line 27）：仅提及单元测试 `errors`
  - **Constraints 第 4 条**（line 40）：放宽为仅单元测试路径 MUST 来自 `test_resolve_paths`，集成测试路径由 subagent 自主决定
  - **Constraints 第 5 条**（line 41）：`test_resolve_paths` 仅用于单元测试路径推导

### 测试文件

- `plugins/dev-team/bin/src/commands/test-resolve-paths.test.ts` — 移除 `runTestResolvePaths -- 集成测试路径`、`runTestResolvePaths -- integration_root 向后兼容`、`runTestResolvePaths -- integration_root 为点号`、`runTestResolvePaths -- integration_root 子目录前缀`、`runTestResolvePaths -- integration_root 与 unit_tests 隔离`、`runTestResolvePaths -- integration_root 路径穿越`、`runTestResolvePaths -- integration_root snake_case 映射` 等 describe 块及其内部用例
- `plugins/dev-team/bin/src/mcp.test.ts` — 工具注册个数验证由 10 改为 9（移除 registerTestResolvePathsTool 后总注册数减少 1）

### 不要修改

- 单元测试路径推导的核心逻辑（`resolveTestPaths`、`processNonEmptyModules`、`processEmptyModules`、`resolveEffectiveModules`、`collectFiles`、`deriveUnitTestPath` 等）
- Test config 过滤和 exclude 过滤逻辑
- `modules` 参数及 `"git-change"` 模式
- `openspec/config.json` 中与集成测试相关的配置定义
- `test-design-planner.md` 中单元测试路径相关步骤（Step 8 的 `modules` 调用保留，仅移除 integration 部分）
- 其他 subagent prompt（`implementation-generator.md` 的 `__tests__/` 黑名单、`test-execution-evaluator.md` 的测试文件检测模式）均不变

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 输入 schema 移除 `integration_scenarios`、`integration_root`、`extension` | `testResolvePathsInputSchema` 的键集合不包含 `integration_scenarios`、`integration_root`、`extension` |
| AC-2 | 输出 schema 移除 `integration_tests` | `testResolvePathsOutputSchema` 的键集合不包含 `integration_tests` |
| AC-3 | 移除 `IntegrationTestEntry` 接口 | 编译通过，代码中无 `IntegrationTestEntry` 引用 |
| AC-4 | 移除集成测试路径相关函数 | `deriveIntegrationTestPath`、`normalizeIntegrationRoot`、`isValidIntegrationRoot`、`inferExtension`、`normalizeExtension`、`resolveIntegrationTests` 均被移除 |
| AC-5 | MCP 工具 description 不再提及集成测试 | `registerTestResolvePathsTool` 的 description 文本不含 "integration" 或 "__tests__/<scenario>" |
| AC-6 | 单元测试路径推导不受影响 | 传入 `modules: ["src/config.ts"]` 时 `unit_tests` 返回 `[{source: "src/config.ts", test_file: "src/config.test.ts"}]` |
| AC-7 | 向 `runTestResolvePaths` 传入 `integration_scenarios` 时不会报错也不会生成集成测试路径（参数被 Zod 静默忽略） | 传入 `integration_scenarios: ["api-flow"]` 时输出无 `integration_tests` 字段，且不因该参数产生任何错误 |
| AC-8 | 测试文件中集成测试相关 describe 块被移除 | 测试套件中不含 `-- 集成测试路径`、`-- integration_root` 相关 describe |
| AC-9 | `test-design-planner.md` 不再引用 `integration_scenarios`/`integration_root` | 文件中不含 `integration_scenarios`、`integration_root`、`integration_tests`（MCP 返回值），集成测试路径步骤改为 subagent 自行创建 `__tests__/` 目录 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| 有外部消费者依赖 `integration_tests` 输出 | 下游工具或脚本收到不完整数据 | 低 | 代码搜索确认当前无上游消费者使用 `integration_tests` 输出；通过编译类型检查发现越界访问 |
| subunit 工具调用方仍传入 `integration_scenarios` 等参数 | 多余参数被 Zod 静默忽略，不产生错误 | 中 | Zod schema 移除了该字段，传入不影响执行（多余字段被 Zod 静默忽略）；输出中不再有 `integration_tests` |
| 覆盖了 `runTestResolvePaths` 的接口签名但遗漏了内部残留 | 编译残留会导致 CI 类型检查失败 | 低 | 确保删除后 TypeScript 编译无错误，且 grep 确认无残留引用 |

---

## 过程

### 决策

| 问题 | 决策 | 理由 | 备选方案 |
|------|------|------|----------|
| 集成测试路径是否应从 `test_resolve_paths` 中移除？ | 是，整体移除 | 职责耦合：路径解析工具仅负责"给定源文件，推导同级单元测试文件"，集成测试路径是 plan 层的决策。实际无上游消费者，且路径格式过于僵化 | 保留但降级为独立工具；保留但标记废弃 |
| 移除的集成测试路径由谁负责？ | 由 subagent 自行创建 | Subagent 在 plan entry 目录下自行创建 `__tests__/<scenario>/`，路径结构更灵活，与 plan 结构自然对齐 | 引入新的独立集成测试路径解析工具；保留现有逻辑 |
| `extension` 参数是否一并移除？ | 是，一并移除 | `extension` 仅被集成测试路径解析的 `inferExtension` 使用，单元测试路径推导基于文件扩展名自动判断 | 保留 `extension` 作为未来扩展预留 |

### 待决问题

- 无。本变更为纯移除，不引入新功能，不遗留未决决策。

---
