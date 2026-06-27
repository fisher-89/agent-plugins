## MODIFIED Requirements

### Requirement: resolveTestPaths 错误收集与输入校验

修改：
- 移除空数组拒绝条款，当 `modules` 为空数组时改为触发 config-driven 自动推导
- `modules` 类型从 `string[]` 扩展为 `string[] | "git-change"`
- `modules` 非空时增加 test config 过滤逻辑

`resolveTestPaths` SHALL 将无法解析的模块条目写入 `errors` 数组，每项为 `{path: string, message: string}`，并继续处理其余条目。

以下情形 SHALL 产生 `errors` 条目：
- 路径在 `project_root` 下不存在
- 文件路径扩展名不在可测试源文件集合内
- 文件路径已是测试文件
- 路径解析后越出 `project_root`（路径穿越）
- 文件不在任何 test config 覆盖范围内（新增）
- git diff 命令执行失败（新增，仅 `"git-change"` 模式）

`errors` 数组 SHALL 按 `path` 字典序排序。

当 `modules` 为空数组时，函数 SHALL NOT 报错或拒绝，SHALL 触发 config-driven 自动推导逻辑（见新增要求）。

当 `modules` 为非空数组时，函数 SHALL 调用 `runTestDetectFrameworks({ files: modules })` 获取检测结果，仅对 `detected` 中的文件推导测试路径。

当 `modules` 为 `"git-change"` 时，函数 SHALL 先执行 `git diff HEAD --name-only` 获取变更文件列表，后续处理同非空 modules。

#### Scenario: 不存在路径写入 errors（不变）

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/missing.ts", "src/config.ts"]`
- **AND** `src/missing.ts` 不存在但 `src/config.ts` 存在
- **THEN** `errors` 包含 `{path: "src/missing.ts", message: 描述文件不存在}`
- **AND** `unit_tests` 仍包含 `src/config.ts` 的推导结果（前提：在 test config 覆盖范围内）

#### Scenario: 非源文件扩展名写入 errors（不变）

- **WHEN** `resolveTestPaths` 收到 `modules: ["README.md"]`
- **THEN** `errors` 包含该路径的条目
- **AND** `unit_tests` 为空数组

#### Scenario: 文件不在 test config 覆盖范围内（新增过滤行为）

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/foo.ts", "scripts/bar.ts"]`
- **AND** test config 仅覆盖 `src/` 目录（scripts/ 不在任何 override 或 framework glob 范围内）
- **THEN** `runTestDetectFrameworks({ files: modules })` 仅返回 `src/foo.ts` 的检测结果
- **AND** `unit_tests` 仅包含 `src/foo.ts` 的推导结果
- **AND** `errors` 包含 `{path: "scripts/bar.ts", message: "Not in test config scope"}`

#### Scenario: modules 为空数组触发 config-driven 推导（替代原拒绝行为）

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** 当前 `config.json` 包含 `test.overrides` 或 `test.framework` 配置
- **THEN** `errors` 为空（无拒绝错误）
- **AND** `unit_tests` 可能包含从 config 目录扫描推导的条目

#### Scenario: modules 为 "git-change" 触发 git diff

- **WHEN** `resolveTestPaths` 收到 `modules: "git-change"`
- **AND** `git diff HEAD --name-only` 成功返回 `["src/foo.ts", "scripts/bar.ts"]`
- **AND** test config 仅覆盖 `src/`
- **THEN** `unit_tests` 包含 `src/foo.ts` 的推导结果
- **AND** `scripts/bar.ts` 不出现在 `unit_tests` 中（被 test config 过滤）

#### Scenario: "git-change" 在非 git 仓库中返回错误

- **WHEN** `resolveTestPaths` 收到 `modules: "git-change"`
- **AND** 项目不在 git 仓库中（`git diff` 失败）
- **THEN** `errors` 包含 `{path: "git", message: "git diff 失败信息"}`
- **AND** `unit_tests` 为空数组

### Requirement: MCP 工具 test_resolve_paths 注册

修改：
- 输入 schema 中 `modules` 从 `z.array(z.string()).min(1)` 改为 `z.union([z.array(z.string()), z.literal("git-change")])`
- 更新 description 以反映 config-driven 过滤、空 modules 自动扫描、`"git-change"` 模式

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `test_resolve_paths` 的 MCP 工具，遵循 `test_` 域名前缀的下划线命名约定。

输入 schema SHALL 包含：
- `modules`: `string[] | "git-change"`，必填，可为空数组。为空时自动从 config.json 推导扫描目录；为 `"git-change"` 时读取 git diff 变更文件。所有路径均受 test config 过滤
- `integration_scenarios`: `string[]`，可选，集成测试场景名列表
- `extension`: `string`，可选，集成测试文件扩展名（如 `"ts"`、`"py"`，可带或不带前导 `.`）
- `integration_root`: `string`，可选，`__tests__/` 的父级目录（相对于 `project_root`）
- `project_root`: `string`，可选，项目根目录（默认由 `resolveProjectRoot()` 解析）

输出 schema SHALL 包含（不变）：
- `unit_tests`: `{source: string, test_file: string}[]`，单元测试路径列表
- `integration_tests`: `{scenario: string, test_file: string}[]`，集成测试路径列表
- `errors`: `{path: string, message: string}[]`，解析错误列表

#### Scenario: test_resolve_paths 返回完整结构（不变）

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/config.ts"], "integration_scenarios": ["api-flow"], "extension": "ts"}`
- **AND** test config 覆盖 `src/` 目录
- **THEN** 返回对象包含 `unit_tests`、`integration_tests` 及 `errors` 字段
- **AND** `unit_tests[0].test_file` 为 `"src/config.test.ts"`
- **AND** `integration_tests[0].test_file` 为 `"__tests__/api-flow/api-flow.test.ts"`

#### Scenario: test_resolve_paths 支持 project_root 覆盖（不变）

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/a.ts"], "project_root": "/abs/project"}`
- **AND** `/abs/project/src/a.ts` 存在且 test config 覆盖该目录
- **THEN** 相对于该根目录解析路径并返回 `src/a.test.ts`

#### Scenario: test_resolve_paths 接受空 modules

- **WHEN** `test_resolve_paths` 收到 `{"modules": []}`
- **THEN** 输入校验通过（不再因 `.min(1)` 拒绝）
- **AND** 函数进入 config-driven 自动推导分支

#### Scenario: test_resolve_paths 接受 "git-change"

- **WHEN** `test_resolve_paths` 收到 `{"modules": "git-change"}`
- **THEN** 输入校验通过
- **AND** 函数进入 git diff 分支

#### Scenario: test_resolve_paths 拒绝非法 modules 类型

- **WHEN** `test_resolve_paths` 收到 `{"modules": 123}` 或 `{"modules": {}}` 等非合法类型
- **THEN** 输入校验失败（Zod union 拒绝）

---

## ADDED Requirements

### Requirement: modules 为空时从 config 自动推导扫描目录

当 `resolveTestPaths` 收到空的 `modules` 数组时，函数 SHALL 调用 `runTestDetectFrameworks`（来自 `test-detect-frameworks.ts`）获取配置计划 `plan`，并按以下规则推导源文件扫描范围：

1. 不传 `files` 参数调用 `runTestDetectFrameworks({})`，触发 auto-scan 获取含完整 `plan` 的返回结果
2. 从返回的 `plan` 中提取每个条目的 `directory` 字段作为扫描根目录
3. 对每个目录递归遍历，收集可测试源文件（复用现有 `collectFiles` 排除规则：跳过 `node_modules`、`.git`、`dist`、`build`、`target` 等）
4. 对收集到的源文件应用现有单元测试路径推导规则（同非空 `modules` 的处理方式）
5. 若 `plan` 为空数组（无 `test.framework` 也无 `test.overrides` 配置），则在 `errors` 中添加指导性消息

函数 SHALL 对来自多个目录的源文件进行去重（同一文件仅产生一份 `unit_tests` 条目）。

#### Scenario: 从 overrides 目录自动扫描源文件

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** `config.json` 包含 `test.overrides: [{ file: "plugins/dev-team/bin", framework: "vite-plus" }]`
- **AND** `plugins/dev-team/bin/src/commands/` 目录下存在 `test-resolve-paths.ts` 文件
- **THEN** 函数调用 `runTestDetectFrameworks` 获得包含 `directory: "plugins/dev-team/bin"` 的 plan
- **AND** `unit_tests` 包含从该目录下扫描到的源文件的推导结果（如 `{source: "src/commands/test-resolve-paths.ts", test_file: "src/commands/test-resolve-paths.test.ts"}`）
- **AND** `errors` 为空数组

#### Scenario: 无配置时返回指导性错误

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** `config.json` 中既无 `test.framework` 也无 `test.overrides`（`plan` 为空）
- **THEN** `unit_tests` 为空数组
- **AND** `errors` 包含一条消息，指示用户在 `openspec/config.json` 中配置 `test.framework` 或 `test.overrides`
- **AND** `integration_tests` 为空数组

#### Scenario: 多个 override 指向同一目录时去重

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** `config.json` 的 `test.overrides` 中包含两个条目指向同一目录（如 `plugins/dev-team/bin`）
- **AND** 该目录下存在文件 `plugins/dev-team/bin/src/foo.ts`
- **THEN** `unit_tests` 中 `foo.ts` 仅出现一次（去重）

#### Scenario: 空 modules 时 integration_scenarios 仍正常工作

- **WHEN** `resolveTestPaths` 收到 `modules: []`, `integration_scenarios: ["smoke"]`, `extension: "ts"`
- **AND** config 中存在有效的 test 配置（plan 非空）
- **THEN** `unit_tests` 非空（从 config 目录扫描）
- **AND** `integration_tests` 包含 `{scenario: "smoke", test_file: "__tests__/smoke/smoke.test.ts"}`

### Requirement: modules 非空时基于 test config 过滤

当 `resolveTestPaths` 收到非空的 `modules` 数组时，函数 SHALL 调用 `runTestDetectFrameworks({ files: modules, projectRoot })` 获取文件→框架的检测结果。

1. 仅对 `detected` 数组中的文件推导测试路径（`detected` 中的文件已确认在 test config 覆盖范围内）
2. 未被 `detected` 的文件（不在任何 test config 覆盖范围内）SHALL 在 `errors` 中添加提示
3. `detected` 中每个文件的 `framework` 字段仅供参考，不影响测试路径推导逻辑（推导仍基于文件扩展名，见 `deriveUnitTestPath`）

#### Scenario: 仅返回 test config 覆盖范围内的文件

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/app.ts", "tools/deploy.ts"]`
- **AND** test config 覆盖 `src/`（`tools/` 不在任何 override 或 framework glob 范围内）
- **THEN** `runTestDetectFrameworks` 返回 `detected: [{ file: "src/app.ts", framework: "vite-plus" }]`
- **AND** `unit_tests` 仅包含 `src/app.ts` 的推导结果
- **AND** `errors` 包含 `{path: "tools/deploy.ts", message: "Not in test config scope"}`

#### Scenario: 所有文件都在 test config 范围内（全量通过）

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/a.ts", "src/b.ts"]`
- **AND** test config 覆盖 `src/` 目录
- **THEN** `detected` 包含两个文件
- **AND** `unit_tests` 包含两个文件的推导结果
- **AND** `errors` 为空（无过滤错误）

### Requirement: modules 为 "git-change" 时读取 git diff 变更文件

当 `resolveTestPaths` 收到 `modules: "git-change"` 时，函数 SHALL：

1. 在 `projectRoot` 下执行 `git diff HEAD --name-only`
2. 解析命令输出（按换行分割，过滤空行），得到变更文件列表
3. 以变更文件列表作为 `modules`，后续处理同非空 modules（调用 `runTestDetectFrameworks({ files })` 过滤并推导）

若 git 命令执行失败（非 git 仓库、git 未安装等），SHALL 在 `errors` 中添加错误消息并返回空 `unit_tests`。

#### Scenario: git diff 返回变更文件并过滤

- **WHEN** `resolveTestPaths` 收到 `modules: "git-change"`
- **AND** `git diff HEAD --name-only` 返回 `src/foo.ts\nscripts/deploy.ts\n`
- **AND** test config 仅覆盖 `src/`
- **THEN** `unit_tests` 包含 `src/foo.ts` 的推导结果
- **AND** `scripts/deploy.ts` 不出现在 `unit_tests` 中
- **AND** `errors` 包含 `{path: "scripts/deploy.ts", message: "Not in test config scope"}`

#### Scenario: git diff 无变更

- **WHEN** `resolveTestPaths` 收到 `modules: "git-change"`
- **AND** `git diff HEAD --name-only` 返回空（无变更）
- **THEN** `unit_tests` 为空数组
- **AND** `integration_tests` 为空数组
- **AND** `errors` 为空（或无致命错误）

#### Scenario: git 命令失败

- **WHEN** `resolveTestPaths` 收到 `modules: "git-change"`
- **AND** 项目根目录不是 git 仓库（`git diff` 失败）
- **THEN** `errors` 包含 `{path: "git", message: <git 错误信息>}`
- **AND** `unit_tests` 为空数组

---

## Module Contract

### Function: resolveTestPaths

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-resolve-paths.ts` |
| **Signature** | `resolveTestPaths(params: ResolveTestPathsParams): ResolveTestPathsResult` |
| **Input** | `projectRoot: string`；`modules: string[] \| "git-change"`；`integrationScenarios?: string[]`；`extension?: string`；`integrationRoot?: string` |
| **Output** | `{ unit_tests: {source, test_file}[], integration_tests: {scenario, test_file}[], errors: {path, message}[] }` |
| **Behavior** | `modules: "git-change"` → 执行 `git diff HEAD --name-only` 获取文件列表，后续同 string[]；`modules: string[]` 非空 → 调用 `runTestDetectFrameworks({ files })` 过滤后推导；`modules: []` → 调用 `runTestDetectFrameworks({})` 获取 plan，从 `plan[].directory` 扫描源文件；plan 为空时写入指导性 errors |

### Function: runTestResolvePaths

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-resolve-paths.ts` |
| **Signature** | `runTestResolvePaths(args: TestResolvePathsInput): ResolveTestPathsResult` |
| **Input** | MCP 工具入参（含可选 `project_root`，`modules` 为 `string[] \| "git-change"`） |
| **Output** | 与 `resolveTestPaths` 相同 |
| **Behavior** | 解析 `project_root` 后委托 `resolveTestPaths` |

### Function: runTestDetectFrameworks (引用)

| Property | Description |
|----------|-------------|
| **Module** | `commands/test-detect-frameworks.ts` |
| **Signature** | `runTestDetectFrameworks(options: TestDetectFrameworksOptions): TestDetectFrameworksResult` |
| **Usage 1** | `modules: []` 时调用 `runTestDetectFrameworks({})` → 触发 auto-scan 获取含 plan 的完整结果；`resolveTestPaths` 提取 `plan` 字段，忽略 `detected` 与 `frameworks` |
| **Usage 2** | `modules: string[]` 非空时调用 `runTestDetectFrameworks({ files: modules, projectRoot })` → 获取文件→框架检测结果；`resolveTestPaths` 使用 `detected` 过滤文件列表 |

### API: test_resolve_paths (MCP tool)

| Property | Description |
|----------|-------------|
| **Tool name** | `test_resolve_paths` |
| **Input schema** | `{ modules: z.union([z.array(z.string()), z.literal("git-change")]), integration_scenarios: ..., extension: ..., integration_root: ..., project_root: ... }` |
| **Output schema** | `{ unit_tests: z.array(z.object({source, test_file})), integration_tests: z.array(z.object({scenario, test_file})), errors: z.array(z.object({path, message})) }` |
| **Registration** | `mcp.ts` — `server.registerTool('test_resolve_paths', ...)` |

### Schema: test-resolve-paths.schema.ts

| Property | Description |
|----------|-------------|
| **Module** | `schemas/test-resolve-paths.schema.ts` |
| **Exports** | `testResolvePathsInputSchema`, `testResolvePathsOutputSchema` |
| **Behavior** | Zod v4 schema；`modules` 为 `z.union([z.array(z.string()), z.literal("git-change")])`，无 `.min(1)` 限制 |
