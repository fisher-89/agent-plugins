# desktop-change-state-store Specification (Delta)

## MODIFIED Requirements

### Requirement: 归档双写

写面 SHALL 提供 `archive` 操作：db `ChangeRecord.status` 翻转为 `archived`（附 `archived_at`）与主仓 active→archive 目录改名（`YYYY-MM-DD-<name>` 日期前缀）SHALL 以同一提交语义完成（双写顺序与失败补偿由 design 定稿）；`ChangeRecord` 主键 `name` MUST NOT 随目录改名变化（db 身份与磁盘目录名解耦）。对带 worktree 记录的 change，归档 SHALL 以主仓目录在场为前置：主仓 active / archive 两树均未命中该目录时 SHALL 显式拒绝并引导「先 merge worktree 分支回主仓再归档」（merge-first 语义见 desktop-change-worktree；归档编排链对前置的自动满足路径见 desktop-change-archive）；merge 后主仓目录在场，既有双写语义零特判。**双写操作本体** MUST NOT 触碰 worktree / branch（清理为用户手动边界）；归档编排链（desktop-change-archive）前置段的 worktree 提交与主仓合入不属双写本体，且双写收口 SHALL 保持归档链末段唯一收口触点身份（单点复用、语义零改动）。对无 db 记录的目录 MUST NOT 提供归档（显式拒绝）。IPC 面 SHALL 提供对应薄命令（三件事纪律，`Result<T, String>`）。

#### Scenario: 双写一致

- **WHEN** 对已建档 change 调用归档命令成功
- **THEN** db status 为 `archived` 且 `archived_at` 在案，主仓磁盘目录已迁移至 archive 树并带日期前缀，两者任一不存在即视为失败

#### Scenario: 主键不随改名变

- **WHEN** 归档后按 change 名查询 db 与按月分组的归档列表
- **THEN** `ChangeRecord.name` 保持原名，归档条目经原名可达且月份分组正确

#### Scenario: 未 merge 的 worktree change 归档被引导拒绝

- **WHEN** 对带 worktree 记录且主仓 active / archive 两树均未命中目录的 change 发起归档
- **THEN** 显式拒绝且错误文案引导先 merge worktree 分支（非泛化「目录未找到」），db 与磁盘零变化

#### Scenario: 双写本体零 worktree / branch 触碰

- **WHEN** 归档链末段双写收口执行（其前置段已由 desktop-change-archive 完成提交与合入）
- **THEN** 双写操作自身仅做目录改名与 db 翻转：零 `git worktree` / `git branch` 调用、零 worktree / branch 状态变更（清理仍为用户手动边界）

#### Scenario: 无建档目录拒绝归档

- **WHEN** 对仅有 workflow.json 的 CLI change 目录发起归档
- **THEN** 显式拒绝（无 ChangeRecord 不可归档），零落账零目录改动
