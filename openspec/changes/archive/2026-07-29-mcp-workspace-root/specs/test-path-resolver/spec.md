## MODIFIED Requirements

### Requirement: MCP 工具 test_resolve_paths 注册

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `test_resolve_paths` 的 MCP 工具，遵循 `test_` 域名前缀的下划线命名约定。

输入 schema SHALL 包含：
- `modules`: `string[] | "git-change"`，必填，可为空数组。为空时自动从 config.json 推导扫描目录；为 `"git-change"` 时读取 git diff 变更文件。所有路径均受 test config 过滤
- `project_root`: `string`，必填。经 MCP 候选/force 入口解析为本次调用的项目根

输入 schema SHALL NOT 包含：
- `integration_scenarios`
- `integration_root`
- `extension`

工具 handler SHALL 通过共享候选/force 入口（如 `resolveProjectRootForTool('test_resolve_paths', args)`）解析 `project_root`；校验失败时 SHALL 返回结构化错误（含 candidates / force 提示，若适用），MUST NOT 回退 `process.cwd()`。handler MUST NOT 使用 `requireLockedProjectRoot()`。

输出 schema SHALL 包含：
- `unit_tests`: `{source: string, test_file: string}[]`，单元测试路径列表
- `errors`: `{path: string, message: string}[]`，解析错误列表

输出 schema SHALL NOT 包含以下已移除字段：
- `integration_tests`

#### Scenario: test_resolve_paths 返回单元测试路径（输入 schema 不再含集成测试参数）

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/config.ts"], "project_root": "/abs/project"}`
- **AND** `/abs/project` 通过候选/force 校验
- **AND** test config 覆盖 `src/` 目录
- **THEN** 返回对象包含 `unit_tests` 及 `errors` 字段
- **AND** `unit_tests[0].test_file` 为 `"src/config.test.ts"`
- **AND** 返回对象不包含 `integration_tests` 字段

#### Scenario: test_resolve_paths 使用必填 project_root

- **WHEN** candidates 含 `/abs/project`
- **AND** `test_resolve_paths` 收到 `{"modules": ["src/a.ts"], "project_root": "/abs/project"}`
- **AND** `/abs/project/src/a.ts` 存在且 test config 覆盖该目录
- **THEN** 相对于该根解析路径并返回 `src/a.test.ts`

#### Scenario: test_resolve_paths 省略 project_root 时失败

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/a.ts"]}`（无 `project_root`）
- **THEN** 输入校验失败（`project_root` 为必填）

#### Scenario: test_resolve_paths 在 project_root 未获准时失败

- **WHEN** `/abs/other` 不在 candidates 且不是 pending force 确认
- **AND** `test_resolve_paths` 收到 `{"modules": ["src/a.ts"], "project_root": "/abs/other"}`
- **THEN** 工具返回结构化错误（含 candidates / force 提示，若适用）
- **AND** MUST NOT 以 `process.cwd()` 解析路径

#### Scenario: test_resolve_paths 接受空 modules

- **WHEN** `test_resolve_paths` 收到 `{"modules": [], "project_root": "/abs/project"}`
- **AND** `/abs/project` 通过候选/force 校验
- **THEN** 输入校验通过（不再因 `.min(1)` 拒绝）
- **AND** 函数进入 config-driven 自动推导分支

#### Scenario: test_resolve_paths 接受 "git-change"

- **WHEN** `test_resolve_paths` 收到 `{"modules": "git-change", "project_root": "/abs/project"}`
- **AND** `/abs/project` 通过候选/force 校验
- **THEN** 输入校验通过
- **AND** 函数进入 git diff 分支

#### Scenario: test_resolve_paths 拒绝非法 modules 类型

- **WHEN** `test_resolve_paths` 收到 `{"modules": 123, "project_root": "/abs/project"}` 或 `{"modules": {}, "project_root": "/abs/project"}` 等非合法类型
- **THEN** 输入校验失败（Zod union 拒绝）

---

## Module Contract

### Function: resolveTestPaths / MCP test_resolve_paths

| Property | Description |
|----------|-------------|
| **MCP Input** | `{ modules, project_root }`（`project_root` 必填） |
| **Handler** | `async (args) => { const projectRoot = resolveProjectRootForTool('test_resolve_paths', args); return jsonContent(runTestResolvePaths({ ...args, project_root: projectRoot })); }` |
| **CLI / command** | Command 层可继续接受 `project_root` 作 fixture；与 MCP 必填契约一致时可复用同一字段 |
