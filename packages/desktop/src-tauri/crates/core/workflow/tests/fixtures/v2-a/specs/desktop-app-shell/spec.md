# desktop-app-shell Specification

## ADDED Requirements

### Requirement: Tauri command 查询双轨

desktop-app SHALL 按 queries / exec 双轨组织 Tauri command：

- `commands/queries/`：MVP 实现三个命令——`list_changes`（change 列表）、`get_change_detail`（change 详情）、`read_artifact`（按信封读取单个产物）
- `commands/exec/`：预留空轨道，MUST NOT 实现任何真实命令，MUST NOT 引入空壳 trait（trait 定形等第一条真实执行命令落地）

每个查询 command SHALL 是无状态薄包装：参数 → core 函数 → DTO 返回，MUST NOT 在 command 层持有或缓存 workspace 状态；所有 workspace 状态访问 SHALL 只经 workflow / foundation 的 core 函数。DTO SHALL 区分 Query Result 与 Command Result 形态。

#### Scenario: 查询命令薄包装

- **WHEN** 前端 invoke `list_changes` / `get_change_detail` / `read_artifact`
- **THEN** command 仅做参数转换并调用 core 查询函数，返回 DTO，自身无状态

#### Scenario: exec 轨道为空

- **WHEN** 检查 `commands/exec/`
- **THEN** 无任何已实现命令、无预定义 Executor trait 空壳

#### Scenario: 状态访问收敛 core

- **WHEN** 审查 command 层代码
- **THEN** 无直接文件系统访问，workspace 状态一律经 core 函数获取

### Requirement: workspace 选择

App SHALL 提供 workspace 选择入口（文件夹选择器）；选定 workspace 根后，列表与详情取数均以该根为基准。最近打开列表不在 MVP 范围，SHALL 于第二刀加回。

#### Scenario: 选择文件夹作为 workspace

- **WHEN** 用户经文件夹选择器选定目录
- **THEN** App 以该目录为根调用 `list_changes` 刷新列表

### Requirement: React 前端刷新取数模型

前端 SHALL 以 React + TS 实现，取数收在 hooks（`useChangeList` / `useChangeDetail`）内：由用户显式刷新动作触发 invoke，MUST NOT 实现文件 watch、后台轮询或事件订阅。刷新 SHALL 覆盖两个层级：workspace 级（重取列表）与 change 级（重取当前详情）。未来替换为推送时 SHALL 仅改动 hooks 内部实现，视图层不感知取数方式。

视图 SHALL 至少呈现：change 列表（代际标注、按月分组）、phase 流水线（attempt / verdict / checklist 展开）、经 renderer 注册表渲染的产物区（含 markdown 文档与 tasks 进度）。

#### Scenario: 刷新按钮触发重取

- **WHEN** 用户点击刷新
- **THEN** hooks 重新 invoke 对应查询命令并更新视图，期间无自动轮询发生

#### Scenario: 取数收口 hooks

- **WHEN** 审查视图组件代码
- **THEN** 组件不直接 invoke，取数统一经 `useChangeList` / `useChangeDetail`

#### Scenario: 无 watch 依赖

- **WHEN** 检查 desktop 依赖与前端代码
- **THEN** 无文件系统监听（notify 等）依赖、无定时器轮询逻辑

## Module Contract

| 模块                             | 职责                       | 关键契约                                                                 |
| -------------------------------- | -------------------------- | ------------------------------------------------------------------------ |
| `desktop-app::commands::queries` | 三个查询命令               | list_changes / get_change_detail / read_artifact；无状态薄包装；返回 DTO |
| `desktop-app::commands::exec`    | 预留空轨道                 | 无实现、无空壳 trait                                                     |
| 前端 `hooks/`                    | 取数收口                   | useChangeList / useChangeDetail；显式刷新触发；未来可换推送              |
| 前端视图                         | 列表 / 流水线 / 产物区渲染 | 消费 DTO 与 ArtifactEnvelope；未注册 kind 由 Fallback 兜底               |
