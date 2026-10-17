# desktop-explore-queries Specification (Delta)

## MODIFIED Requirements

### Requirement: promote_explore 命令与笔记搬运

命令层 SHALL 提供 `promote_explore(root, name, workflow_type)`（落 `commands/explores/`）：blank root / 非 kebab name / 非法 workflow_type 显式 `Err`。语义 SHALL 为 **move**：

1. **占位前缀门禁**：记录名以 `draft-` 前缀（免命名建档的未定名态，见 desktop-workspace-store「ExploreRecord 模型与 explore 清单操作面」）→ 显式 `Err`（引导先在会话中完成定名再启动变更）；该校验 SHALL 先于笔记读取——占位名从未落盘，「笔记未落盘」错误形态对该态不足以引导；
2. 读主仓笔记全文（`read_explore` 命中且内容非空白，否则显式 `Err` 引导先完成探索）；
3. 读 `ExploreRecord`（存在且 `promoted_to` 为空，否则显式 `Err`）；
4. 调 workflow 写面 `promote_explore`（复用 change `create`：`name = explore.name`、`goal = 笔记全文`、`title = explore.title`、`workflow_type = 入参`，成功后删主仓 `explores/<name>.md`）；
5. 写面成功后调 store `mark_explore_promoted(root, name, change_id)` 打标（会话链保留，MUST NOT 删 explore 记录）。

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

#### Scenario: 占位名门禁先于笔记读取

- **WHEN** 对 `draft-` 前缀的未定名记录调用 `promote_explore`
- **THEN** 显式 `Err`（文案引导先在会话中完成定名），零磁盘 IO、零 store 写、change 侧零建档；报错先于「笔记未落盘」形态出现

#### Scenario: 命令层零直接磁盘删除

- **WHEN** 审查 `promote_explore` 命令实现
- **THEN** 命令层无 `std::fs::remove_file` 直接调用；笔记删除经 workflow 写面 `promote_explore` 完成

## Module Contract

| 模块 | 类别 | 关键契约 |
|------|------|----------|
| `src-tauri/src/commands/explores/mod.rs`（`promote_explore_with`） | API | 步序前置占位前缀门禁（`draft-` 前缀 → 显式 `Err` 引导定名，先于笔记读取）；其余步序与语义零变化 |
| `src-tauri/src/commands/explores/mod.rs`（`create_explore_record`） | API | `name: Option<String>` 透传 store（`None` = 占位建档；`Some` = 导入绑定显式名）；薄包装三件事纪律不变 |
