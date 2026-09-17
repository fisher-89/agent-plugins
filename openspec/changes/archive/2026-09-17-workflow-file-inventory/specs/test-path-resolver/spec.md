# test-path-resolver Specification（workflow-file-inventory 增量）

## MODIFIED Requirements

### Requirement: resolveTestPaths 错误收集与输入校验

修改：
- 移除空数组拒绝条款，当 `modules` 为空数组时改为触发 config-driven 自动推导
- `modules` 类型从 `string[]` 扩展为 `string[] | "git-change"`（本变更：`"git-change"` 字面量由清单模式字面量替代，建议 `"change"`，最终命名 design 定）
- `modules` 非空时增加 test config 过滤逻辑

`resolveTestPaths` SHALL 将无法解析的模块条目写入 `errors` 数组，每项为 `{path: string, message: string}`，并继续处理其余条目。

以下情形 SHALL 产生 `errors` 条目：
- 路径在 `project_root` 下不存在
- 文件路径扩展名不在可测试源文件集合内
- 文件路径已是测试文件
- 路径解析后越出 `project_root`（路径穿越）
- 文件不在任何 test config 覆盖范围内（新增）
- 清单读取失败（本变更替代原 git diff 失败：目标 change 不存在、`workflow.json` 缺失或非法、`files` 字段缺失等，仅清单模式）

`errors` 数组 SHALL 按 `path` 字典序排序。

当 `modules` 为空数组时，函数 SHALL NOT 报错或拒绝，SHALL 触发 config-driven 自动推导逻辑（见新增要求）。

当 `modules` 为非空数组时，函数 SHALL 调用 `runTestDetectFrameworks({ files: modules })` 获取检测结果，仅对 `detected` 中的文件推导测试路径。

当 `modules` 为清单模式时，函数 SHALL 先读取目标 change 的 `workflow.json` 的 `files.written` 获取变更文件列表（不再执行 `git diff HEAD --name-only`），后续处理同非空 modules。目标 change 的指定方式（独立参数或字面量内嵌，design 定）。

#### Scenario: 不存在路径写入 errors（不变）

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/missing.ts", "src/config.ts"]`
- **AND** `src/missing.ts` 不存在但 `src/config.ts` 存在
- **THEN** `errors` 包含 `{path: "src/missing.ts", message: 描述文件不存在}`
- **AND** `unit_tests` 仍包含 `src/config.ts` 的推导结果（前提：在 test config 覆盖范围内）

#### Scenario: 非源文件扩展名写入 errors（不变）

- **WHEN** `resolveTestPaths` 收到 `modules: ["README.md"]`
- **THEN** `errors` 包含该路径的条目
- **AND** `unit_tests` 为空数组

#### Scenario: 文件不在 test config 覆盖范围内（不变）

- **WHEN** `resolveTestPaths` 收到 `modules: ["src/foo.ts", "scripts/bar.ts"]`
- **AND** test config 仅覆盖 `src/` 目录（scripts/ 不在任何 override 或 framework glob 范围内）
- **THEN** `runTestDetectFrameworks({ files: modules })` 仅返回 `src/foo.ts` 的检测结果
- **AND** `unit_tests` 仅包含 `src/foo.ts` 的推导结果
- **AND** `errors` 包含 `{path: "scripts/bar.ts", message: "Not in test config scope"}`

#### Scenario: modules 为空数组触发 config-driven 推导（不变）

- **WHEN** `resolveTestPaths` 收到 `modules: []`
- **AND** 当前 `config.json` 包含非空 `tests` suite 配置
- **THEN** `errors` 为空（无拒绝错误）
- **AND** `unit_tests` 可能包含从 suite scope 扫描推导的条目

#### Scenario: 清单模式触发清单读取

- **WHEN** `resolveTestPaths` 收到清单模式入参（目标 change 的 `files.written` 为 `["src/foo.ts", "scripts/bar.ts"]`）
- **AND** test config 仅覆盖 `src/`
- **THEN** `unit_tests` 包含 `src/foo.ts` 的推导结果
- **AND** `scripts/bar.ts` 不出现在 `unit_tests` 中（被 test config 过滤）
- **AND** 进程不执行任何 `git diff` 命令

#### Scenario: 清单读取失败返回错误

- **WHEN** `resolveTestPaths` 收到清单模式入参且目标 change 不存在（或 `workflow.json` 缺失 / 非法 / 无 `files` 字段）
- **THEN** `errors` 包含描述失败原因的条目（含 `change_create` 或重建指引）
- **AND** `unit_tests` 为空数组

### Requirement: modules 为 "git-change" 时读取 git diff 变更文件

**MODIFIED（重写为清单模式）**：当 `modules` 为清单模式字面量（替代原 `"git-change"`）时，函数 SHALL：

1. 解析目标 change（经 change 参数或字面量，design 定）
2. 读取 `openspec/changes/<change>/workflow.json` 的 `files.written` 作为变更文件列表；目标 change 的 `workflow.json` 缺失、非法或无 `files` 字段时 SHALL 硬报错（与其它消费方一致的重建指引）
3. 以变更文件列表作为 `modules`，后续处理同非空 modules（调用 `runTestDetectFrameworks({ files })` 过滤并推导，exclude 过滤经同一路径自动生效）

函数 MUST NOT 执行 `git diff HEAD --name-only` 或任何 git 命令获取变更列表。

#### Scenario: 清单返回变更文件并过滤

- **WHEN** 清单模式入参且目标 change 的 `files.written` 解析为 `["src/foo.ts", "scripts/deploy.ts"]`
- **AND** test config 仅覆盖 `src/`
- **THEN** `unit_tests` 包含 `src/foo.ts` 的推导结果
- **AND** `scripts/deploy.ts` 不出现在 `unit_tests` 中
- **AND** `errors` 包含 `{path: "scripts/deploy.ts", message: "Not in test config scope"}`

#### Scenario: 清单为空

- **WHEN** 清单模式入参且目标 change 的 `files.written` 为空数组
- **THEN** `unit_tests` 为空数组
- **AND** `errors` 为空（或无致命错误）

#### Scenario: 清单读取失败

- **WHEN** 清单模式入参且 `workflow.json` 无 `files` 字段（机制前旧 change）
- **THEN** `errors` 包含硬错误条目（含重建指引）
- **AND** `unit_tests` 为空数组

## Module Contract

### Function: resolveTestPaths（增量）

| Property | Before | After |
|----------|--------|-------|
| **清单模式输入** | `modules: "git-change"` → `git diff HEAD --name-only` | 清单模式字面量 → 读目标 change 的 `workflow.json` `files.written` |
| **git 依赖** | `execSync('git diff HEAD --name-only')`（`resolveEffectiveModules`） | **REMOVED** |
| **失败语义** | git 失败 → `errors: [{path: "git", ...}]` | 清单读取失败 → `errors` 含重建指引条目 |
| **后续管线** | 非空 modules 管线（test config 过滤 + exclude 过滤） | 不变 |

### Schema: test-resolve-paths.schema.ts（增量）

| Property | Description |
|----------|-------------|
| `modules` | `string[] \| <清单模式字面量>`；`"git-change"` 字面量移除，替换名 design 定 |
| change 指定 | 独立参数或字面量内嵌，design 定 |
