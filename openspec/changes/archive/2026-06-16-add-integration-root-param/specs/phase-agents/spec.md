## MODIFIED Requirements

### Requirement: test-design-planner 调用 test_resolve_paths 获取测试文件路径

`plugins/dev-team/agents/test-design-planner.md` SHALL 在编写 test-design.md 之前调用 MCP 工具 `test_resolve_paths`，根据 design.md 与源码 Grep 结果确定的模块列表获取规定的测试文件路径。

Process 章节 SHALL 包含以下步骤（在读取 proposal.md、design.md 并 Grep 源码之后，写入 test-design.md 之前）：

1. 从 design.md 的变更范围与 Grep 结果汇总**精确模块列表**（文件路径或目录路径，相对于项目根目录）
2. 从 proposal.md / design.md 识别需要集成测试覆盖的**场景名**列表（若有）
3. 调用 `mcp__plugin_dev-team_dev-team__test_resolve_paths` 获取单元测试路径：传入 `modules` 与可选的 `extension`（单元测试路径不依赖 `integration_root`）
4. 当存在集成测试场景时，调用 `mcp__plugin_dev-team_dev-team__test_detect_frameworks` 获取各框架 plan 条目；对**每个** plan 条目调用 `mcp__plugin_dev-team_dev-team__test_resolve_paths`，传入相同的 `modules`、`integration_scenarios`、`extension`，并将该 plan 条目的 `directory` 作为 `integration_root`
5. 将返回的 `unit_tests` 映射到 test-design.md `验收范围` 与 `单元测试 > 用例` 表格的 `测试文件` 列（`source` → 被测模块，`test_file` → 测试文件路径）
6. 将各次 `test_resolve_paths` 调用返回的 `integration_tests` 合并映射到 `集成测试 > 用例` 表格的 `测试文件` 列（每条集成测试路径相对 `project_root`，且与对应框架的 `directory` 一致）
7. 若任一次调用的 `errors` 非空，在 test-design.md `不可测试项` 或相应章节记录无法解析的模块及原因

当项目仅含单一框架且 plan 条目 `directory` 为 `"."` 时，Planner MAY 仅调用一次 `test_resolve_paths`（不传 `integration_root` 或传 `"."`），行为与变更前等价。

Agent SHALL NOT 手工拼接或猜测测试文件路径；所有 `测试文件` 列的值 MUST 来自 `test_resolve_paths` 返回值（或明确标注为不可测试并说明原因）。

Agent 工具权限 SHALL 包含 `test_resolve_paths` 与 `test_detect_frameworks` MCP 工具。

#### Scenario: planner 使用 MCP 返回的单元测试路径填充 coverage map

- **WHEN** `test_resolve_paths` 对 `modules: ["src/config.ts"]` 返回 `unit_tests: [{source: "src/config.ts", test_file: "src/config.test.ts"}]`
- **THEN** test-design.md `验收范围` 表格中对应 AC 行的 `测试文件` 列为 `src/config.test.ts`
- **AND** `单元测试 > 用例` 表格中相关行的 `测试文件` 列同为 `src/config.test.ts`

#### Scenario: planner 使用 MCP 返回的集成测试路径

- **WHEN** `test_resolve_paths` 对 `integration_scenarios: ["api-flow"]` 且未传 `integration_root` 返回 `integration_tests: [{scenario: "api-flow", test_file: "__tests__/api-flow/api-flow.test.ts"}]`
- **THEN** test-design.md `集成测试 > 用例` 表格的 `测试文件` 列为 `__tests__/api-flow/api-flow.test.ts`

#### Scenario: planner 按框架 directory 传入 integration_root

- **WHEN** `test_detect_frameworks` 返回 plan 条目 `{framework: "vite-plus", directory: "plugins/dev-team/bin", ...}`
- **AND** planner 对该条目调用 `test_resolve_paths`，传入 `integration_scenarios: ["api-flow"]` 与 `integration_root: "plugins/dev-team/bin"`
- **THEN** test-design.md `集成测试 > 用例` 中对应行的 `测试文件` 列为 `plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts`

#### Scenario: planner 传入目录级模块列表

- **WHEN** design.md 变更范围为目录 `plugins/dev-team/bin/src/commands/`
- **THEN** planner 将该目录路径传入 `test_resolve_paths` 的 `modules` 参数
- **AND** 使用返回的多条 `unit_tests` 条目分别填充各源文件对应的测试文件路径

#### Scenario: planner 记录 errors 中的不可解析模块

- **WHEN** `test_resolve_paths` 返回 `errors: [{path: "src/legacy.md", message: "..."}]`
- **THEN** test-design.md 在 `不可测试项` 章节列出该路径及原因
- **AND** 不在 coverage map 中为该路径填写虚构的测试文件路径

#### Scenario: test-design-planner.md 包含 test_resolve_paths 与 integration_root 调用说明

- **WHEN** 读取 `plugins/dev-team/agents/test-design-planner.md`
- **THEN** Process 章节包含对 `mcp__plugin_dev-team_dev-team__test_resolve_paths` 的调用步骤
- **AND** 描述多框架场景下将 `test_detect_frameworks` plan 条目的 `directory` 作为 `integration_root` 传入
- **AND** 明确禁止手工拼接测试文件路径

## Module Contract

### Component: test-design-planner agent

| Property | Description |
|----------|-------------|
| **Module** | `plugins/dev-team/agents/test-design-planner.md` |
| **MCP tools** | `test_resolve_paths`、`test_detect_frameworks`（集成测试场景时）；现有 Read/Grep 工具不变 |
| **Input artifacts** | `proposal.md`, `design.md`, 源码 Grep 结果 |
| **Output artifact** | `openspec/changes/<change-name>/test-design.md` |
| **Behavior** | 单元测试路径一次 `test_resolve_paths`；集成测试按 plan 条目 `directory` 多次调用并合并 `integration_tests`；errors 记入不可测试项 |
