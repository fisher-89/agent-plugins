## ADDED Requirements

### Requirement: resolveTestPaths 按语言规则推导单元测试路径

`plugins/dev-team/bin/src/commands/test-resolve-paths.ts` SHALL 导出纯函数 `resolveTestPaths`，根据源文件路径推导**同级目录**单元测试文件路径。命名规则 SHALL 为：

| 源文件扩展名 | 单元测试文件名模式 | 目录 |
|-------------|-------------------|------|
| `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | `<basename>.test<ext>` | 与源文件相同目录 |
| `.py` | `test_<basename>.py` | 与源文件相同目录 |
| `.go` | `<basename>_test.go` | 与源文件相同目录 |
| `.rs` | `<basename>_test.rs` | 与源文件相同目录 |

函数 SHALL 将所有路径规范为相对于 `project_root` 的 POSIX 风格相对路径（使用 `/` 分隔符）。

函数 SHALL 为确定性纯函数：相同输入始终产生相同输出，不访问网络。

#### Scenario: TypeScript 源文件推导单元测试路径

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/config.ts"]` 且 `project_root` 指向含该文件的项目
- **THEN** 返回的 `unit_tests` 包含 `{source: "src/config.ts", test_file: "src/config.test.ts"}`

#### Scenario: TSX 组件文件推导单元测试路径

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/component/Button.tsx"]`
- **THEN** 返回的 `unit_tests` 包含 `{source: "src/component/Button.tsx", test_file: "src/component/Button.test.tsx"}`

#### Scenario: Python 源文件推导单元测试路径

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/auth.py"]`
- **THEN** 返回的 `unit_tests` 包含 `{source: "src/auth.py", test_file: "src/test_auth.py"}`

#### Scenario: Go 源文件推导单元测试路径

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/handler.go"]`
- **THEN** 返回的 `unit_tests` 包含 `{source: "src/handler.go", test_file: "src/handler_test.go"}`

#### Scenario: Rust 源文件推导单元测试路径

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/lib.rs"]`
- **THEN** 返回的 `unit_tests` 包含 `{source: "src/lib.rs", test_file: "src/lib_test.rs"}`

### Requirement: resolveTestPaths 展开目录模块为源文件列表

当 `modules` 中的条目为目录路径时，`resolveTestPaths` SHALL 递归遍历该目录（深度优先），收集所有可测试源文件并对每个源文件应用单元测试路径推导规则。

遍历 SHALL 跳过以下目录名：`node_modules`、`.git`、`dist`、`build`、`coverage`、`.nyc_output`、`target`。

遍历 SHALL 跳过以下文件：
- 扩展名不在可测试源文件集合（`.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`, `.py`, `.go`, `.rs`）内的文件
- 已是测试文件的文件：匹配 `*.test.*`、`test_*.py`、`*_test.go`、`*_test.rs`
- 文档与配置文件：`.md`、`.json`、`.yaml`、`.yml`、`.txt`、`.lock`

`unit_tests` 结果 SHALL 按 `source` 字典序排序并去重。

#### Scenario: 目录输入展开为多份单元测试路径

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/commands/"]`
- **AND** 目录内含 `src/commands/foo.ts` 与 `src/commands/bar.ts`
- **THEN** `unit_tests` 包含 `{source: "src/commands/bar.ts", test_file: "src/commands/bar.test.ts"}` 与 `{source: "src/commands/foo.ts", test_file: "src/commands/foo.test.ts"}`

#### Scenario: 目录遍历跳过 node_modules

- **WHEN** `modules` 包含 `src/` 且其下存在 `src/node_modules/pkg/index.ts`
- **THEN** `unit_tests` 不包含 `src/node_modules/pkg/index.ts` 的条目

### Requirement: resolveTestPaths 推导集成测试路径

当提供 `integration_scenarios` 字符串数组时，`resolveTestPaths` SHALL 为每个场景名 `scenario` 生成集成测试路径：

```
__tests__/<scenario>/<scenario>.test.<ext>
```

其中 `<ext>` 为不含点号的扩展名字符串（如 `ts`、`py`）。

扩展名解析优先级 SHALL 为：
1. 若调用方提供 `extension` 参数，使用该值（去除前导 `.`）
2. 否则从 `modules` 展开后的源文件扩展名统计众数（`.ts` 计为 `ts`，`.py` 计为 `py` 等）
3. 若无法推断（无有效源文件），默认 `ts`

`integration_tests` 结果 SHALL 按 `scenario` 字典序排序。

#### Scenario: 显式 extension 生成集成测试路径

- **WHEN** `resolveTestPaths` 收到 `integration_scenarios: ["api-flow"]` 且 `extension: "ts"`
- **THEN** `integration_tests` 包含 `{scenario: "api-flow", test_file: "__tests__/api-flow/api-flow.test.ts"}`

#### Scenario: 从 modules 推断集成测试扩展名

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/auth.py"]` 且 `integration_scenarios: ["db-roundtrip"]`（未传 `extension`）
- **THEN** `integration_tests` 包含 `{scenario: "db-roundtrip", test_file: "__tests__/db-roundtrip/db-roundtrip.test.py"}`

#### Scenario: 无有效源文件时集成测试默认 ts 扩展名

- **WHEN** `resolveTestPaths` 收到 `modules: ["README.md"]`（非源文件，写入 `errors`）
- **AND** `integration_scenarios: ["smoke"]` 且未传 `extension`
- **THEN** `integration_tests` 包含 `{scenario: "smoke", test_file: "__tests__/smoke/smoke.test.ts"}`

### Requirement: resolveTestPaths 错误收集与输入校验

`resolveTestPaths` SHALL 将无法解析的模块条目写入 `errors` 数组，每项为 `{path: string, message: string}`，并继续处理其余条目。

以下情形 SHALL 产生 `errors` 条目：
- 路径在 `project_root` 下不存在
- 文件路径扩展名不在可测试源文件集合内
- 文件路径已是测试文件
- 路径解析后越出 `project_root`（路径穿越）

当 `modules` 为空数组时，函数 SHALL 抛出校验错误或在 MCP 层由 Zod 拒绝，SHALL NOT 返回空的「成功」结果。

`errors` 数组 SHALL 按 `path` 字典序排序。

#### Scenario: 不存在路径写入 errors

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/missing.ts", "src/config.ts"]`
- **AND** `src/missing.ts` 不存在但 `src/config.ts` 存在
- **THEN** `errors` 包含 `{path: "src/missing.ts", message: 描述文件不存在}`
- **AND** `unit_tests` 仍包含 `src/config.ts` 的推导结果

#### Scenario: 非源文件扩展名写入 errors

- **WHEN** `resolveTestPaths` 收到 `modules: ["README.md"]`
- **THEN** `errors` 包含该路径的条目
- **AND** `unit_tests` 为空数组

#### Scenario: modules 为空数组被拒绝

- **WHEN** MCP 工具 `test_resolve_paths` 收到 `{"modules": []}`
- **THEN** 输入校验失败并返回错误
- **AND** 不返回 `unit_tests` 或 `integration_tests` 字段

### Requirement: MCP 工具 test_resolve_paths 注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `test_resolve_paths` 的 MCP 工具，遵循 `test_` 域名前缀的下划线命名约定。

输入 schema SHALL 包含：
- `modules`: `string[]`，必填，模块路径列表（文件或目录，相对于 `project_root`）
- `integration_scenarios`: `string[]`，可选，集成测试场景名列表
- `extension`: `string`，可选，集成测试文件扩展名（如 `"ts"`、`"py"`，可带或不带前导 `.`）
- `project_root`: `string`，可选，项目根目录（默认由 `resolveProjectRoot()` 解析）

输出 schema SHALL 包含：
- `unit_tests`: `{source: string, test_file: string}[]`，单元测试路径列表
- `integration_tests`: `{scenario: string, test_file: string}[]`，集成测试路径列表
- `errors`: `{path: string, message: string}[]`，解析错误列表

工具 handler SHALL 调用 `runTestResolvePaths()` 并返回 JSON 内容。

#### Scenario: test_resolve_paths 返回完整结构

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/config.ts"], "integration_scenarios": ["api-flow"], "extension": "ts"}`
- **THEN** 返回对象包含非空 `unit_tests`、`integration_tests` 及 `errors`（可为空数组）字段
- **AND** `unit_tests[0].test_file` 为 `"src/config.test.ts"`
- **AND** `integration_tests[0].test_file` 为 `"__tests__/api-flow/api-flow.test.ts"`

#### Scenario: test_resolve_paths 支持 project_root 覆盖

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/a.ts"], "project_root": "/abs/project"}`
- **AND** `/abs/project/src/a.ts` 存在
- **THEN** 相对于该根目录解析路径并返回 `src/a.test.ts`

## Module Contract

### Function: resolveTestPaths

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-resolve-paths.ts` |
| **Signature** | `resolveTestPaths(params: ResolveTestPathsParams): ResolveTestPathsResult` |
| **Input** | `projectRoot: string`；`modules: string[]`；`integrationScenarios?: string[]`；`extension?: string` |
| **Output** | `{ unit_tests: {source, test_file}[], integration_tests: {scenario, test_file}[], errors: {path, message}[] }` |
| **Behavior** | 展开目录、按语言规则推导单元/集成测试路径；错误不中断其余条目；纯函数、确定性输出 |

### Function: runTestResolvePaths

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-resolve-paths.ts` |
| **Signature** | `runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult` |
| **Input** | MCP 工具入参（含可选 `project_root`） |
| **Output** | 与 `resolveTestPaths` 相同 |
| **Behavior** | 解析 `project_root` 后委托 `resolveTestPaths` |

### Function: resolveProjectRoot

| Property | Description |
|----------|-------------|
| **Module** | `mcp.ts` |
| **Signature** | `resolveProjectRoot(cwd?: string \| null): string` |
| **Input** | 可选目录提示 |
| **Output** | 绝对项目根路径 |
| **Behavior** | 复用现有实现：`CLAUDE_PROJECT_DIR` 或 `process.cwd()` |

### API: test_resolve_paths (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `test_resolve_paths` |
| **Input schema** | `{ modules: z.array(z.string()).min(1), integration_scenarios: z.array(z.string()).optional(), extension: z.string().optional(), project_root: z.string().optional().nullable() }` |
| **Output schema** | `{ unit_tests: z.array(z.object({source, test_file})), integration_tests: z.array(z.object({scenario, test_file})), errors: z.array(z.object({path, message})) }` |
| **Registration** | `mcp.ts` — `server.registerTool('test_resolve_paths', ...)` |
| **Handler** | `async (args) => jsonContent(runTestResolvePaths({...}))` |

### Schema: test-resolve-paths.schema.ts

| Property | Description |
|----------|-------------|
| **Module** | `schemas/test-resolve-paths.schema.ts` |
| **Exports** | `testResolvePathsInputSchema`, `testResolvePathsOutputSchema` |
| **Behavior** | Zod v4 schema；`modules` 最少 1 项；与 MCP 注册一致 |
