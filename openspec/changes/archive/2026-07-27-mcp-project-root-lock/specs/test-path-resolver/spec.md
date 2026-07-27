## MODIFIED Requirements

### Requirement: MCP 工具 test_resolve_paths 注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `test_resolve_paths` 的 MCP 工具，遵循 `test_` 域名前缀的下划线命名约定。

输入 schema SHALL 包含：
- `modules`: `string[] | "git-change"`，必填，可为空数组。为空时自动从 config.json 推导扫描目录；为 `"git-change"` 时读取 git diff 变更文件。所有路径均受 test config 过滤

输入 schema SHALL NOT 包含：
- `project_root`（改由 MCP 启动锁定根提供）
- `integration_scenarios`
- `integration_root`
- `extension`

工具 handler SHALL 通过锁定根入口（`requireLockedProjectRoot()` 或等价）解析项目根；锁定未设置时 SHALL 返回结构化错误，MUST NOT 回退 `process.cwd()`。

输出 schema SHALL 包含：
- `unit_tests`: `{source: string, test_file: string}[]`，单元测试路径列表
- `errors`: `{path: string, message: string}[]`，解析错误列表

输出 schema SHALL NOT 包含以下已移除字段：
- `integration_tests`

#### Scenario: test_resolve_paths 返回单元测试路径（输入 schema 不再含集成测试参数）

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/config.ts"]}`
- **AND** 项目根已锁定
- **AND** test config 覆盖 `src/` 目录
- **THEN** 返回对象包含 `unit_tests` 及 `errors` 字段
- **AND** `unit_tests[0].test_file` 为 `"src/config.test.ts"`
- **AND** 返回对象不包含 `integration_tests` 字段

#### Scenario: test_resolve_paths 使用锁定根而非 project_root 覆盖

- **WHEN** 项目根锁定为 `/abs/project`
- **AND** `test_resolve_paths` 收到 `{"modules": ["src/a.ts"]}`（无 `project_root` 字段）
- **AND** `/abs/project/src/a.ts` 存在且 test config 覆盖该目录
- **THEN** 相对于该锁定根解析路径并返回 `src/a.test.ts`

#### Scenario: test_resolve_paths 拒绝 MCP input 中的 project_root

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/a.ts"], "project_root": "/abs/project"}`
- **THEN** 输入校验失败（`project_root` 不在 schema 中）

#### Scenario: test_resolve_paths 在未锁定时失败

- **WHEN** 项目根锁未设置
- **AND** `test_resolve_paths` 收到 `{"modules": ["src/a.ts"]}`
- **THEN** 工具返回结构化错误
- **AND** MUST NOT 以 `process.cwd()` 解析路径

#### Scenario: test_resolve_paths 接受空 modules

- **WHEN** `test_resolve_paths` 收到 `{"modules": []}`
- **AND** 项目根已锁定
- **THEN** 输入校验通过（不再因 `.min(1)` 拒绝）
- **AND** 函数进入 config-driven 自动推导分支

#### Scenario: test_resolve_paths 接受 "git-change"

- **WHEN** `test_resolve_paths` 收到 `{"modules": "git-change"}`
- **AND** 项目根已锁定
- **THEN** 输入校验通过
- **AND** 函数进入 git diff 分支

#### Scenario: test_resolve_paths 拒绝非法 modules 类型

- **WHEN** `test_resolve_paths` 收到 `{"modules": 123}` 或 `{"modules": {}}` 等非合法类型
- **THEN** 输入校验失败（Zod union 拒绝）

## Module Contract

### Function: runTestResolvePaths

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-resolve-paths.ts` |
| **Signature** | `runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult` |
| **Input** | Command 层可含可选 `project_root`（CLI/单测 fixture）；MCP handler 不得传入该字段 |
| **Output** | 与 `resolveTestPaths` 相同 |
| **Behavior** | 解析项目根后委托 `resolveTestPaths` |

### API: test_resolve_paths (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `test_resolve_paths` |
| **Input schema** | `{ modules: z.union([z.array(z.string()), z.literal("git-change")]) }` |
| **Output schema** | `{ unit_tests: z.array(z.object({source, test_file})), errors: z.array(z.object({path, message})) }` |
| **Registration** | `mcp.ts` — `server.registerTool('test_resolve_paths', ...)` |
| **Handler** | `async (args) => { const projectRoot = requireLockedProjectRoot(); return jsonContent(runTestResolvePaths({ ...args, project_root: projectRoot })); }` |
