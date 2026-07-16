## REMOVED Requirements

### Requirement: resolveTestPaths 推导集成测试路径

此要求所在段落（原文"### Requirement: resolveTestPaths 推导集成测试路径"）从 `resolveTestPaths` 规范中移除。该要求定义了根据 `integration_scenarios` 和 `extension` 参数在 `__tests__/<scenario>/` 目录下生成集成测试路径的行为。

**Reason**: 集成测试路径推导职责不属路径解析工具的范畴。Subagent 需在各自的 plan entry 目录下自行创建 `__tests__/` 子目录并管理测试文件，无需中心化路径推导。

**Migration**: 原有调用方不应再传入 `integration_scenarios`、`integration_root`、`extension` 参数（Zod schema 已移除此三个字段，多余参数被静默忽略）。Subagent 应通过约定在 plan entry 目录下手动创建 `__tests__/<scenario>/` 目录并生成测试文件。

以下四个 Scenario 被整体移除：

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

#### Scenario: 空 modules 时 integration_scenarios 仍正常工作

- **WHEN** `resolveTestPaths` 收到 `modules: []`, `integration_scenarios: ["smoke"]`, `extension: "ts"`
- **AND** config 中存在有效的 test 配置（plan 非空）
- **THEN** `unit_tests` 非空（从 config 目录扫描）
- **AND** `integration_tests` 包含 `{scenario: "smoke", test_file: "__tests__/smoke/smoke.test.ts"}`

## MODIFIED Requirements

### Requirement: resolveTestPaths 错误收集与输入校验

原文第四段（关于 `integration_root` 路径穿越错误）被移除。其余行为不变。

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

所有原有 Scenario 保持不变。

### Requirement: MCP 工具 test_resolve_paths 注册

修改后输入 schema 移除 `integration_scenarios`、`integration_root`、`extension` 字段。输出 schema 移除 `integration_tests` 字段。

`plugins/dev-team/bin/src/mcp.ts` SHALL 注册名为 `test_resolve_paths` 的 MCP 工具，遵循 `test_` 域名前缀的下划线命名约定。

输入 schema SHALL 包含：
- `modules`: `string[] | "git-change"`，必填，可为空数组。为空时自动从 config.json 推导扫描目录；为 `"git-change"` 时读取 git diff 变更文件。所有路径均受 test config 过滤
- `project_root`: `string`，可选，项目根目录（默认由 `resolveProjectRoot()` 解析）

输入 schema SHALL NOT 包含以下已移除字段：
- `integration_scenarios`
- `integration_root`
- `extension`

输出 schema SHALL 包含：
- `unit_tests`: `{source: string, test_file: string}[]`，单元测试路径列表
- `errors`: `{path: string, message: string}[]`，解析错误列表

输出 schema SHALL NOT 包含以下已移除字段：
- `integration_tests`

#### Scenario: test_resolve_paths 返回单元测试路径（输入 schema 不再含集成测试参数）

- **WHEN** `test_resolve_paths` 收到 `{"modules": ["src/config.ts"]}`
- **AND** test config 覆盖 `src/` 目录
- **THEN** 返回对象包含 `unit_tests` 及 `errors` 字段
- **AND** `unit_tests[0].test_file` 为 `"src/config.test.ts"`
- **AND** 返回对象不包含 `integration_tests` 字段

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

### Requirement: modules 为空时从 config 自动推导扫描目录

移除 `modules` 为空时 `integration_scenarios` 和 `integration_tests` 相关内容。当 `resolveTestPaths` 收到空的 `modules` 数组时，函数 SHALL 调用 `runTestDetectFrameworks`（来自 `test-detect-frameworks.ts`）获取配置计划 `plan`，并按以下规则推导源文件扫描范围：

1. 不传 `files` 参数调用 `runTestDetectFrameworks({})`，触发 auto-scan 获取含完整 `plan` 的返回结果
2. 从返回的 `plan` 中提取每个条目的 `directory` 字段作为扫描根目录
3. 对每个目录递归遍历，收集可测试源文件（复用现有 `collectFiles` 排除规则：跳过 `node_modules`、`.git`、`dist`、`build`、`target` 等）
4. 对收集到的源文件应用现有单元测试路径推导规则（同非空 `modules` 的处理方式）
5. 若 `plan` 为空数组（无 `test.framework` 也无 `test.overrides` 配置），则在 `errors` 中添加指导性消息

函数 SHALL 对来自多个目录的源文件进行去重（同一文件仅产生一份 `unit_tests` 条目）。

移除以下已废弃 Scenario：
- `#### Scenario: 空 modules 时 integration_scenarios 仍正常工作` — 已移除，因 `integration_scenarios` 参数和 `integration_tests` 输出均不再存在

保留以下原有 Scenario：

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

#### Scenario: 多个 override 指向同一目录时去重

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** `config.json` 的 `test.overrides` 中包含两个条目指向同一目录（如 `plugins/dev-team/bin`）
- **AND** 该目录下存在文件 `plugins/dev-team/bin/src/foo.ts`
- **THEN** `unit_tests` 中 `foo.ts` 仅出现一次（去重）

### Requirement: modules 为 "git-change" 时读取 git diff 变更文件

移除 `integration_tests` 相关断言。当 `resolveTestPaths` 收到 `modules: "git-change"` 时，函数 SHALL：

1. 在 `projectRoot` 下执行 `git diff HEAD --name-only`
2. 解析命令输出（按换行分割，过滤空行），得到变更文件列表
3. 以变更文件列表作为 `modules`，后续处理同非空 modules（调用 `runTestDetectFrameworks({ files })` 过滤并推导）

若 git 命令执行失败（非 git 仓库、git 未安装等），SHALL 在 `errors` 中添加错误消息并返回空 `unit_tests`。

保留以下原有 Scenario（移除 `integration_tests` 引用）：

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
- **AND** `errors` 为空（或无致命错误）

#### Scenario: git 命令失败

- **WHEN** `resolveTestPaths` 收到 `modules: "git-change"`
- **AND** 项目根目录不是 git 仓库（`git diff` 失败）
- **THEN** `errors` 包含 `{path: "git", message: <git 错误信息>}`
- **AND** `unit_tests` 为空数组

## Module Contract

### Module: commands/test-resolve-paths.ts (Removed code)

| Aspect | Detail |
|--------|--------|
| **Removed interface** | `IntegrationTestEntry` — 不再存在 |
| **Removed functions** | `deriveIntegrationTestPath()`, `normalizeIntegrationRoot()`, `isValidIntegrationRoot()`, `inferExtension()`, `normalizeExtension()`, `resolveIntegrationTests()` — 全部删除 |
| **Changed type** | `ResolveTestPathsParams` — 移除 `integrationScenarios?: string[]`, `extension?: string`, `integrationRoot?: string` 三个属性 |
| **Changed type** | `TestResolvePathsInput` — 移除 `integration_scenarios?: string[]`, `extension?: string`, `integration_root?: string` 三个属性 |
| **Changed function** | `resolveTestPaths()` — 移除对 `resolveIntegrationTests` 的调用；返回类型不再包含 `integration_tests` |
| **Changed function** | `runTestResolvePaths()` — 不再向 `resolveTestPaths` 传递 `integrationScenarios`, `extension`, `integrationRoot` |

### Module: schemas/test-resolve-paths.schema.ts (Changed schemas)

| Property | Before | After |
|----------|--------|-------|
| `testResolvePathsInputShape` keys | `modules`, `integration_scenarios`, `extension`, `integration_root`, `project_root` | `modules`, `project_root` |
| `testResolvePathsOutputShape` keys | `unit_tests`, `integration_tests`, `errors` | `unit_tests`, `errors` |

### API: test_resolve_paths (MCP tool)

| Property | Before | After |
|----------|--------|-------|
| **Input schema** | `{ modules, integration_scenarios?, extension?, integration_root?, project_root? }` | `{ modules, project_root? }` |
| **Output schema** | `{ unit_tests, integration_tests, errors }` | `{ unit_tests, errors }` |
| **Description** | 提及 `__tests__/<scenario>/` 集成测试路径 | 仅描述单元测试路径推导，不再包含集成测试相关内容 |

### Module Contract (Unchanged)

以下模块契约不受本次变更影响：
- `resolveProjectRoot` 函数签名与行为不变
- 单元测试路径推导函数 `deriveUnitTestPath` 不变
- `collectFiles` 函数不变
- `processNonEmptyModules` / `processEmptyModules` / `resolveEffectiveModules` 等步骤函数不变
- `ResolveTestPathsResult` 输出类型中 `unit_tests` 和 `errors` 字段不变

---
