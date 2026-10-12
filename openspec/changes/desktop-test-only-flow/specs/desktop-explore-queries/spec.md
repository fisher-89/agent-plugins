# desktop-explore-queries — desktop-test-only-flow delta

## MODIFIED Requirements

### Requirement: promote_explore 命令与笔记搬运

命令层 SHALL 提供 `promote_explore(root, name, workflow_type)`（落 `commands/explores/`）：blank root / 非 kebab name → 显式 `Err`；`workflow_type` 非法值（`phase_table` 不命中）→ 显式 `Err`。语义 SHALL 为 **move**：

1. 读主仓笔记全文（`read_explore` 命中且内容非空白，否则显式 `Err` 引导先完成探索）；
2. 读 `ExploreRecord`（存在且 `promoted_to` 为空，否则显式 `Err`）；
3. 调 workflow 写面 `promote_explore`（复用 change `create`：`name = explore.name`、`goal = 笔记全文`、`title = explore.title`、`workflow_type = 入参`，成功后删主仓 `explores/<name>.md`）；
4. 写面成功后调 store `mark_explore_promoted(root, name, change_id)` 打标（会话链保留，MUST NOT 删 explore 记录）。

命令 SHALL 返回 `PromoteOutcome`（`change_id` / `change_name`）。磁盘笔记删除 SHALL 仅经 workflow 写面完成，命令层 MUST NOT 直接 `remove_file`。写面 `promote_explore` SHALL 为 sync、零 Tauri、零 tokio（沿写面纪律），经 `Layout` 取全部磁盘路径（MUST NOT 自拼 `openspec` 字面量），并把 `workflow_type` 透传 `create`。

#### Scenario: promote move 成功

- **WHEN** 对已落盘且非空白的 kebab explore（`api-retry`，笔记首行 `# API 重试调研`，类型选择 requirement）调用 `promote_explore`
- **THEN** 返回 `PromoteOutcome`（change_id / change_name=api-retry）；worktree 内 `changes/api-retry/explore.md` 内容为笔记全文；主仓 `explores/api-retry.md` 已删；store 记录 `promoted_to = change_id` 且会话链保留；铸出 change `workflow_type = "requirement"`

#### Scenario: test-only 类型透传

- **WHEN** 以 `workflow_type="test-only"` 调用 `promote_explore`
- **THEN** 写面 `promote_explore` 把 `"test-only"` 透传 `create`，铸出 change `workflow_type = "test-only"`

#### Scenario: 非法 workflow_type 拒绝

- **WHEN** 以 `workflow_type="bug-fix"`（或空白）调用 `promote_explore`
- **THEN** 显式 `Err`，explore 记录与笔记零改动、change 侧零建档

#### Scenario: promote 拒绝面

- **WHEN** 分别以未落盘 / 空白内容 / 非 kebab name / 已 promoted / 同名 active change 冲突调用 `promote_explore`
- **THEN** 均返回显式 `Err`，explore 记录与笔记零改动、change 侧零建档（冲突拒绝发生在 create 前置，create 失败补偿链由既有语义承担）

#### Scenario: 命令层零直接磁盘删除

- **WHEN** 审查 `promote_explore` 命令实现
- **THEN** 命令层无 `std::fs::remove_file` 直接调用；笔记删除经 workflow 写面 `promote_explore` 完成

## ADDED Requirements

### Requirement: update_explore_workflow_type 回填命令

命令层 SHALL 提供 `update_explore_workflow_type(root, name, workflow_type)`（落 `commands/explores/`）：blank root / 非法 `workflow_type`（`phase_table` 不命中）→ 显式 `Err`；经 `for_root` 路由所属 workspace 库并调 store `set_explore_workflow_type`，返回更新后的 `ExploreRecord`。该命令 SHALL 为显式写命令（workflow_type 回填唯一写入口），读取路径 `read_explore` MUST NOT 回填或写入任何状态。

#### Scenario: 回填成功与拒绝面

- **WHEN** 以合法 root / name / `workflow_type="test-only"` 调用 `update_explore_workflow_type`
- **THEN** store 记录 `workflow_type` 更新且 `updated_at` 刷新，返回记录逐字段一致；非法值 / blank root 返回显式 `Err`，库内零变化

#### Scenario: 读取路径零写入

- **WHEN** 连续调用 `read_explore` 后查询 store 记录
- **THEN** `ExploreRecord.workflow_type` / `updated_at` 零变化（workflow_type 回填仅经 `update_explore_workflow_type`）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/queries/explore.rs`（不改） | explore 读面 | `read_explore(layout, name)`（stem 校验防穿越，缺失 → None）；纯读、零 Tauri |
| `crates/core/workflow/src/write/promote.rs` | promote 写面 | `promote_explore(..., workflow_type: &str)` → `create` 透传；sync、零 Tauri、零 tokio |
| `src-tauri/src/commands/explores/` | 薄命令 | `promote_explore(root, name, workflow_type)` 三参；`update_explore_workflow_type(root, name, workflow_type)`；blank root / 非法值显式 `Err`；纯文本 DTO 不套信封 |
| `src-tauri/src/commands/mod.rs` | 单一登记面 | `all_commands!` 登记 `update_explore_workflow_type` |
| `packages/desktop/src/types/generated/bindings.ts` | 类型跟随 | `promoteExplore` 三参、`updateExploreWorkflowType` 再生成；`bindings:check` 守卫拦截漂移 |
