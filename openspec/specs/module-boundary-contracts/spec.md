## 权威边界

proposal 阶段模块边界契约。执行主体为 `proposal-planner`（决议：与 `phase-skills` 命名对齐）。

## ADDED Requirements

### Requirement: proposal 阶段识别受影响的模块目录

`proposal-planner` SHALL 在生成 proposal.md 和 specs/ 时，分析当前变更涉及的文件变更集合，识别所有受影响的模块目录。
每个模块目录 SHALL 分配唯一模块标识符（如 `module:user-auth`）。
识别规则以项目根 `CLAUDE.md` 模块定义为准；若无，则以 `src/` / `packages/` 等顶层源码子目录为单位。

#### Scenario: 识别变更涉及的模块

- **WHEN** `proposal-planner` 分析变更路径集合
- **THEN** 提取非重复顶层模块目录并生成唯一标识符

### Requirement: proposal 阶段定义模块的 public/export 函数签名

对每个受影响模块，`proposal-planner` SHALL 定义 public/export 函数契约：函数名、参数（名称+类型+必需+默认值）、返回类型、简要描述；以表格写入对应 capability 的 `spec.md`。

#### Scenario: 函数签名写入 spec

- **WHEN** 定义了 public 函数签名
- **THEN** 写入 `openspec/changes/<name>/specs/<capability>/spec.md` 的 `## ADDED Requirements`，含函数名、参数、返回值、描述列

#### Scenario: 无 public export

- **WHEN** 模块无可导出函数
- **THEN** 注明「无可导出函数」，不生成空表

### Requirement: proposal 阶段定义模块的 API 接口

对每个受影响模块，定义 API 契约：method、path、request/response schema；写入对应 `spec.md`。无 API 则注明并跳过空表。

#### Scenario: API 契约写入 / 无 API 跳过

- **WHEN** 模块暴露或未暴露 API
- **THEN** 分别写入表格或注明「无 API 接口」

### Requirement: proposal 阶段定义模块的 CLI 命令

对每个受影响模块，定义 CLI 契约：命令名、参数、flags、示例；写入对应 `spec.md`。无 CLI 则注明并跳过空表。

#### Scenario: CLI 契约写入 / 无 CLI 跳过

- **WHEN** 模块暴露或未暴露 CLI
- **THEN** 分别写入表格或注明「无 CLI 命令」

### Requirement: proposal 阶段定义前端组件的 props & events

若项目含前端（检测到 `.vue`/`.tsx`/`.jsx`），对受影响前端模块定义组件契约：组件名、props、events；写入对应 `spec.md`。非前端项目则注明并跳过。

#### Scenario: 前端 / 非前端

- **WHEN** 有或无前端代码
- **THEN** 分别生成组件表或注明「无前端组件定义」
