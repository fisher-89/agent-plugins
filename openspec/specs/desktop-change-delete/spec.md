# desktop-change-delete Specification

## Purpose

定义 change 用户删除面：`/changes` 清单每行（进行中 + 归档分组）的行级删除入口、二次确认弹窗（不可撤销告知 + 进行中条目的「同时清空 worktree」勾选，缺省不勾）、`delete_change` 命令与写面 `workflow::write::delete` 的执行序（可选 worktree / 分支清理 → 主仓产物目录清理 → 级联删库），以及**运行中一律拒绝**的护栏。本能力偿还 desktop-change-archive「V1 边界 #8 弃置不做」与 desktop-change-worktree「V1 边界 #1 branch 清理手动 / #6 孤儿对账文档化」两处挂账。

## Requirements

### Requirement: 写面删除与执行序

core/workflow SHALL 提供 sync 写面 `write::delete(main_root, layout, store, vcs, id, clean_worktree) -> Result<DeleteOutcome, String>`，按 change **id** 寻址（`get_change(id)` miss → 显式 `Err`，零副作用），对解析出的 `record.name` 施单分量名校验（与 archive / create 同一单点）。执行序 SHALL 为**磁盘先、库最后**（沿 archive 的「先改名后翻库」哲学）：

1. **worktree 清理（条件步）**：仅当 `clean_worktree = true` 且 `record.worktree = Some` 时执行——目录在场则 `WorktreePort::remove_worktree`（`--force`），目录缺席则跳过（重试链幂等）；随后 branch `change/<name>` 经 `branch_exists` 命中才 `delete_branch`（**不随目录缺席跳过**——残留分支会撞 create 前置⑥「branch 已存在」）。任一 vcs 步 `Err` → 整体 `Err`（库记录与产物目录原样未动）。
2. **主仓产物目录清理（恒执行）**：候选定位序 = 归档记录的自身 `archived_at` 推导目录 `<archive_root>/<YYYY-MM-DD>-<name>` → `locate_change` 的 active → archive 精确 → 日期前缀后缀扫描；命中即 `remove_dir_all`，失败 → `Err`（携路径）。
3. **级联删库（最后一步）**：`ChangeStateStore::delete_change_cascade(id)`；`Err` 原样透出（此时目录已删，重试由步骤 1/2 的缺席跳过收敛，仅补库半边）。

`DeleteOutcome` SHALL 携 `name` / `removed_worktree`（本次是否实际移除 worktree 目录）/ `warnings`（未勾选而目录仍在场时的保留注记）。各步 SHALL 幂等可重入；写面 MUST NOT 自行停止或观察 run（并发护栏归命令层）。git 触点 SHALL 全经 `WorktreePort` 缝（进程 spawn 驻 `crates/infra/vcs`），写面零直接 spawn。

#### Scenario: 勾选清空后同名可重建

- **WHEN** 对带 worktree 记录的 active change 以 `clean_worktree = true` 调用删除
- **THEN** worktree 目录消失、`git worktree list` 不再列出、branch `change/<name>` 删除、主仓产物目录删除、库记录级联删除；随后以同名创建 change 成功（create 前置⑥/⑦不再撞）

#### Scenario: 不勾选保留 worktree 并给出注记

- **WHEN** 以 `clean_worktree = false` 删除带 worktree 记录的 active change
- **THEN** worktree 目录与 branch 原样保留，`warnings` 携保留注记（含路径与分支名）；主仓产物目录与库记录照常删除

#### Scenario: worktree 目录已缺席仍清分支

- **WHEN** 记录的 worktree 目录已被用户手动删除，以 `clean_worktree = true` 删除
- **THEN** 目录步幂等跳过、`removed_worktree = false`，branch `change/<name>` 若在场仍被删除

#### Scenario: worktree 移除失败即中断

- **WHEN** `git worktree remove` 非零退出
- **THEN** 写面显式 `Err`（含路径与原因）；库记录与主仓产物目录原样未动，branch 零触碰

#### Scenario: 归档条目按自身日期前缀命中

- **WHEN** 删除一条 archived 记录（`archived_at` 在场、磁盘目录为 `YYYY-MM-DD-<name>`）
- **THEN** 删除对象为该记录日期前缀命中的目录（同名归档共存时各自删自己那份），不留残余目录

#### Scenario: 归档条目与 legacy 记录的清理边界

- **WHEN** 删除归档条目（无清空勾选）或 legacy 记录（`worktree = None`）
- **THEN** 二者均不清空 worktree / 分支（归档路径的分支保留为历史可溯，见 desktop-change-worktree V1 边界 #1；legacy 记录无 worktree 可清）；`clean_worktree` 对无 worktree 记录零作用。归档路径遗留的 branch `change/<name>` 仍由 create 前置⑥的错误文案引导手动清理

### Requirement: 级联删库范围与会话留痕

`ChangeStateStore::delete_change_cascade(id)` SHALL 在单写事务内清空该 change 的全部**流程状态行**：`ChangeRecord` 主行、`PhaseRecord` 及其 `ChecklistItemRecord` 子行、`StepRecord`、`RunRecord` / `RunNodeRecord` / `RunEdgeRecord`。miss SHALL 幂等 `Ok(false)`（不开写事务）。MUST NOT 删除 `SessionRecord` / `SessionEventRecord` / `AgentRunRecord`——会话与转录是审计留痕（`source_ref` 携 change id），删除后按 `sessionId` 直查仍可达（与 `delete_explore_record` 的会话级联口径刻意不同：explore 记录是会话的归属主体，change 不是）。补偿删除 `delete_change_record` 的既有「只删本次自插单行」语义 SHALL 保持不变，两函数职责分立。

#### Scenario: 他 change 的行不误删

- **WHEN** 两个 change 各有相位条目 / 步骤审计 / run 历史，删除其一
- **THEN** 仅被删 change 的六类行清零，另一 change 的建档 / 相位 / 步骤 / run 行原样在场

#### Scenario: 会话与转录保留

- **WHEN** 被删 change 名下存在流程会话与 chat 会话（`source = change`、`source_ref` 携 change id）
- **THEN** 删除后这些会话与转录行仍在案，按 change id 的来源筛选查询仍返回它们

#### Scenario: miss 幂等

- **WHEN** 对未建档 id 调用级联删除
- **THEN** 返回 `Ok(false)`，零写入；重复删除已删 id 同样 `Ok(false)`

### Requirement: 命令面与运行中拒绝

壳层 SHALL 提供 `delete_change(root, id, clean_worktree) -> Result<DeleteOutcome, String>`（`#[tauri::command] #[specta::specta]`，`ErrorHandlingMode::Throw` → 前端 `Promise<DeleteOutcome>`）。blank `root` / `id` SHALL 显式 `Err`（写命令语义，与 `create_change` / `archive_change` 同口径）。**运行中 SHALL 拒绝**：`ChangeFlowControl::snapshot(root, id).is_some()` 即 `Err("change \"{id}\" 正在运行中，请先停止运行后再删除")`，零副作用——运行注册表是「此刻是否有 run 在跑」的唯一权威（`ChangeRecord.active_phase` 可能是中断残留），删除面 MUST NOT 自行停止 run。该判定 SHALL 驻命令层（core/workflow MUST NOT 依赖 orchestration）；写面调用 SHALL 经 `spawn_blocking` 承载（同 `create_change_with` 同构）。

#### Scenario: 运行中拒绝且零副作用

- **WHEN** 该 change 有在飞 run（注册表有条目）时调用 `delete_change`
- **THEN** 返回 `Err` 且文案提示先停止运行；零磁盘删除、零库删除；停掉 run 后重试成功

#### Scenario: blank 参数显式拒绝

- **WHEN** `root` 为空 / 空白，或 `id` 为空 / 空白时调用 `delete_change`
- **THEN** 返回 `Err`（`非法 root` / `非法 id`），零副作用

### Requirement: 清单行级删除入口与二次确认

`/changes` 清单 SHALL 在**每一行**（进行中与归档分组的条目一致）呈现删除入口，且点击删除入口 MUST NOT 触发导航（行点击仍落 `/changes/<id>`）。删除入口 SHALL 先开二次确认弹窗，弹窗 SHALL 呈现：条目标题、删除范围告知（库记录：相位 / 步骤 / 运行史，与磁盘产物目录，**不可撤销**）、底部「取消」与破坏性样式的「删除」。进行中条目（`source = active`）SHALL 另呈「同时清空 worktree（移除 worktree 目录与分支 `change/<name>`）；不勾选则原样保留」勾选框，**缺省不勾选**（误删未提交改动是更坏的失败）；归档条目 MUST NOT 呈该勾选框（无 worktree 执行面）。确认时 SHALL 以 `{ root, id, cleanWorktree }` 调 `delete_change`，成功即关窗、刷新清单**并经 sonner 呈现结果 toast**，失败 SHALL 在弹窗内以行内错误呈现后端原文并**保持弹窗打开**可重试，清单不刷新、**零 toast**（错误面单轨，不重复告知）。取消 / 遮罩 / Esc SHALL 关窗且零调用。每次开窗 SHALL 重置勾选态与错误（不跨确认残留）。

结果 toast 的呈现 SHALL 为：`DeleteOutcome.warnings` 非空 → `toast.warning`，标题「已删除变更「<title>」」、description 为各条注记换行拼接（worktree / 分支保留的后果必须可见——否则「不勾选」是静默的）；`warnings` 为空 → `toast.success`，文案同标题。<title> SHALL 取该条目在清单中的 `title`（人类可读标题，非 `name`），条目不在当前取数内（清单未落定）时回落 `id`，MUST NOT 因标题缺席阻断删除。

#### Scenario: 确认删除并刷新清单

- **WHEN** 用户点击某行的删除入口并在弹窗中确认（勾选或不勾选清空）
- **THEN** `delete_change` 恰被调用一次（携该行 id 与勾选值），成功后弹窗关闭、清单重新取数、该行消失；URL 仍为 `/changes`

#### Scenario: 失败留窗可重试

- **WHEN** 删除调用 reject（如运行中拒绝）
- **THEN** 弹窗内呈现后端错误原文、弹窗不关闭、按钮恢复可用；清单不刷新、该行仍在、零 toast（错误面单轨）

#### Scenario: 成功 toast 区分有无警告

- **WHEN** 删除成功且 `warnings` 非空（如未勾选清空而 worktree 与分支保留）
- **THEN** 呈现 `toast.warning`，标题「已删除变更「<title>」」、description 列全部注记；`warnings` 为空时呈现 `toast.success`（同标题）

#### Scenario: 取消零调用

- **WHEN** 用户在确认弹窗中点击取消（或遮罩 / Esc）
- **THEN** 弹窗关闭，零 `delete_change` 调用、零清单刷新
