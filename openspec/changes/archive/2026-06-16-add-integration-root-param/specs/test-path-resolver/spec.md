## MODIFIED Requirements

### Requirement: resolveTestPaths 推导集成测试路径

当提供 `integration_scenarios` 字符串数组时，`resolveTestPaths` SHALL 为每个场景名 `scenario` 生成集成测试路径。

基础路径模式 SHALL 为：

```
__tests__/<scenario>/<scenario>.test.<ext>
```

其中 `<ext>` 为不含点号的扩展名字符串（如 `ts`、`py`）。

当调用方提供 `integration_root` 且其值不为 `"."` 时，集成测试路径 SHALL 为：

```
<integration_root>/__tests__/<scenario>/<scenario>.test.<ext>
```

`integration_root` SHALL 表示 `__tests__/` 目录的父路径，相对于 `project_root`，使用 POSIX `/` 分隔符。实现 SHALL 去除 `integration_root` 尾部斜杠后再拼接。当 `integration_root` 未提供或为 `"."` 时，SHALL 保持不含前缀的基础路径模式（向后兼容）。

扩展名解析优先级 SHALL 为：
1. 若调用方提供 `extension` 参数，使用该值（去除前导 `.`）
2. 否则从 `modules` 展开后的源文件扩展名统计众数（`.ts` 计为 `ts`，`.py` 计为 `py` 等）
3. 若无法推断（无有效源文件），默认 `ts`

`integration_tests` 结果 SHALL 按 `scenario` 字典序排序。

`integration_root` SHALL NOT 影响 `unit_tests` 推导逻辑。

#### Scenario: 显式 extension 生成集成测试路径

- **WHEN** `resolveTestPaths` 收到 `integration_scenarios: ["api-flow"]` 且 `extension: "ts"`
- **AND** 未传 `integration_root` 或 `integration_root` 为 `"."`
- **THEN** `integration_tests` 包含 `{scenario: "api-flow", test_file: "__tests__/api-flow/api-flow.test.ts"}`

#### Scenario: 从 modules 推断集成测试扩展名

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/auth.py"]` 且 `integration_scenarios: ["db-roundtrip"]`（未传 `extension`）
- **AND** 未传 `integration_root` 或 `integration_root` 为 `"."`
- **THEN** `integration_tests` 包含 `{scenario: "db-roundtrip", test_file: "__tests__/db-roundtrip/db-roundtrip.test.py"}`

#### Scenario: 无有效源文件时集成测试默认 ts 扩展名

- **WHEN** `resolveTestPaths` 收到 `modules: ["README.md"]`（非源文件，写入 `errors`）
- **AND** `integration_scenarios: ["smoke"]` 且未传 `extension`
- **AND** 未传 `integration_root` 或 `integration_root` 为 `"."`
- **THEN** `integration_tests` 包含 `{scenario: "smoke", test_file: "__tests__/smoke/smoke.test.ts"}`

#### Scenario: integration_root 前缀子目录集成测试路径

- **WHEN** `resolveTestPaths` 收到 `integration_scenarios: ["api-flow"]`、`extension: "ts"` 且 `integration_root: "plugins/dev-team/bin"`
- **THEN** `integration_tests` 包含 `{scenario: "api-flow", test_file: "plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts"}`

#### Scenario: integration_root 为点号时无前缀

- **WHEN** `resolveTestPaths` 收到 `integration_scenarios: ["api-flow"]`、`extension: "ts"` 且 `integration_root: "."`
- **THEN** `integration_tests` 包含 `{scenario: "api-flow", test_file: "__tests__/api-flow/api-flow.test.ts"}`

#### Scenario: integration_root 不影响单元测试路径

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/config.ts"]`、`integration_scenarios: ["api-flow"]` 且 `integration_root: "plugins/dev-team/bin"`
- **THEN** `unit_tests` 包含 `{source: "src/config.ts", test_file: "src/config.test.ts"}`
- **AND** `integration_tests[0].test_file` 以 `plugins/dev-team/bin/__tests__/` 为前缀

### Requirement: MCP 工具 test_resolve_paths 注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `test_resolve_paths` 的 MCP 工具，遵循 `test_` 域名前缀的下划线命名约定。

输入 schema SHALL 包含：
- `modules`: `string[]`，必填，模块路径列表（文件或目录，相对于 `project_root`）
- `integration_scenarios`: `string[]`，可选，集成测试场景名列表
- `extension`: `string`，可选，集成测试文件扩展名（如 `"ts"`、`"py"`，可带或不带前导 `.`）
- `integration_root`: `string`，可选，`__tests__/` 目录的父路径（相对于 `project_root`；默认为项目根，即 `"."`）
- `project_root`: `string`，可选，项目根目录（默认由 `resolveProjectRoot()` 解析）

输出 schema SHALL 包含：
- `unit_tests`: `{source: string, test_file: string}[]`，单元测试路径列表
- `integration_tests`: `{scenario: string, test_file: string}[]`，集成测试路径列表
- `errors`: `{path: string, message: string}[]`，解析错误列表

工具 handler SHALL 调用 `runTestResolvePaths()` 并返回 JSON 内容，SHALL 将 `integration_root` 透传至命令层。

#### Scenario: test_resolve_paths 返回完整结构

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/config.ts"], "integration_scenarios": ["api-flow"], "extension": "ts"}`
- **THEN** 返回对象包含非空 `unit_tests`、`integration_tests` 及 `errors`（可为空数组）字段
- **AND** `unit_tests[0].test_file` 为 `"src/config.test.ts"`
- **AND** `integration_tests[0].test_file` 为 `"__tests__/api-flow/api-flow.test.ts"`

#### Scenario: test_resolve_paths 支持 project_root 覆盖

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/a.ts"], "project_root": "/abs/project"}`
- **AND** `/abs/project/src/a.ts` 存在
- **THEN** 相对于该根目录解析路径并返回 `src/a.test.ts`

#### Scenario: test_resolve_paths 支持 integration_root

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/config.ts"], "integration_scenarios": ["api-flow"], "extension": "ts", "integration_root": "plugins/dev-team/bin"}`
- **THEN** `integration_tests[0].test_file` 为 `"plugins/dev-team/bin/__tests__/api-flow/api-flow.test.ts"`
- **AND** `unit_tests[0].test_file` 仍为 `"src/config.test.ts"`

## Module Contract

### Function: deriveIntegrationTestPath

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-resolve-paths.ts` |
| **Signature** | `deriveIntegrationTestPath(scenario: string, ext: string, integrationRoot?: string): string` |
| **Input** | 场景名；扩展名（不含点）；可选 `integrationRoot`（相对 `project_root` 的父目录，默认 `"."`） |
| **Output** | POSIX 相对路径字符串 |
| **Behavior** | `integrationRoot` 未提供或为 `"."` 时返回 `__tests__/<scenario>/<scenario>.test.<ext>`；否则返回 `<integrationRoot>/__tests__/...`；去除 `integrationRoot` 尾部斜杠 |

### Function: resolveTestPaths

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-resolve-paths.ts` |
| **Signature** | `resolveTestPaths(params: ResolveTestPathsParams): ResolveTestPathsResult` |
| **Input** | `projectRoot: string`；`modules: string[]`；`integrationScenarios?: string[]`；`extension?: string`；`integrationRoot?: string` |
| **Output** | `{ unit_tests: {source, test_file}[], integration_tests: {scenario, test_file}[], errors: {path, message}[] }` |
| **Behavior** | 展开目录、按语言规则推导单元/集成测试路径；`integrationRoot` 仅影响 `integration_tests`；错误不中断其余条目；纯函数、确定性输出 |

### Function: runTestResolvePaths

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-resolve-paths.ts` |
| **Signature** | `runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult` |
| **Input** | MCP 工具入参（含可选 `project_root`、`integration_root`） |
| **Output** | 与 `resolveTestPaths` 相同 |
| **Behavior** | 解析 `project_root` 后委托 `resolveTestPaths`，映射 `integration_root` → `integrationRoot` |

### API: test_resolve_paths (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `test_resolve_paths` |
| **Input schema** | `{ modules: z.array(z.string()).min(1), integration_scenarios: z.array(z.string()).optional(), extension: z.string().optional(), integration_root: z.string().optional(), project_root: z.string().optional().nullable() }` |
| **Output schema** | `{ unit_tests: z.array(z.object({source, test_file})), integration_tests: z.array(z.object({scenario, test_file})), errors: z.array(z.object({path, message})) }` |
| **Registration** | `mcp.ts` — `server.registerTool('test_resolve_paths', ...)` |
| **Handler** | `async (args) => jsonContent(runTestResolvePaths({...}))` |

### Schema: test-resolve-paths.schema.ts

| Property | Description |
|----------|-------------|
| **Module** | `schemas/test-resolve-paths.schema.ts` |
| **Exports** | `testResolvePathsInputSchema`, `testResolvePathsOutputSchema` |
| **Behavior** | Zod v4 schema；`modules` 最少 1 项；含可选 `integration_root`；与 MCP 注册一致 |
