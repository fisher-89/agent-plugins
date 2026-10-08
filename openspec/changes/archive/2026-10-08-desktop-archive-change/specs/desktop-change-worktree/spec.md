# desktop-change-worktree Specification (Delta)

## MODIFIED Requirements

### Requirement: merge-first 归档引导

带 worktree 记录的 change 归档 SHALL 以主仓目录在场为前置：主仓 active / archive 两树均未命中该 change 目录时，直接归档 SHALL 显式拒绝并引导「先 merge worktree 分支（change/<name>）回主仓再归档」（MUST NOT 泛化「目录未找到」了事）；主仓出现 `changes/<n>/` 后，既有归档双写（db status 翻转 + 主仓目录改名）SHALL 零改动可用。

满足前置的两条等价路径：**归档链自动段**（desktop-change-archive：归档编排链在双写收口前自动执行 worktree 提交 + 主仓合入，为用户点击归档按钮后的缺省路径）与**用户手动 merge**（既有路径维持，归档链的幂等跳过将其视为已完成段）。merge SHALL NOT 被强制：用户可不 merge 弃置 change（V1 弃置 = 手动清 worktree / branch，db 记录保留，见「V1 范围与边界留痕」）。自动合入的冲突与主仓状态约束（显式失败引导手动、不吞并无关改动、不改写主仓历史）由 desktop-change-archive 约束。run 收口语义不变：全相位 pass 照常停等「Ready for archiving」，MUST NOT 自动归档、MUST NOT 自动 merge（归档链由用户点击归档入口发起，非 run 收口的自动动作）；worktree / branch 清理仍为用户手动边界（归档链成功后不 remove worktree、不删 branch）。

#### Scenario: 归档链自动提交合入满足前置

- **WHEN** 带 worktree 记录且主仓两树均未命中目录的 change 经归档入口发起归档链，提交与合入段成功
- **THEN** 主仓出现 `changes/<n>/`，双写收口零特判走通（无 worktree 路径分支进入收口段）；用户全程无手动 git 操作

#### Scenario: 手动 merge 后归档链跳过合入

- **WHEN** 用户已手动将 worktree 分支 merge 回主仓（主仓出现 `changes/<n>/`）后经归档入口发起归档链
- **THEN** 合入段幂等跳过（零 git 动作），链径直至双写收口；直接经裸归档命令调用亦零改动可用（既有语义）

#### Scenario: 未 merge 的直接归档仍被引导拒绝

- **WHEN** 绕过归档链对带 worktree 记录且主仓两树均未命中目录的 change 直接调用归档写面
- **THEN** 显式拒绝且错误文案引导先 merge worktree 分支；db 与磁盘零变化（既有引导语义不因归档链在场而退役）

#### Scenario: run 收口不自动合入

- **WHEN** 带 worktree 记录的 change run 以全相位 pass 收口
- **THEN** run 照常停等「Ready for archiving」，零自动 merge、零自动归档、零归档链触发（用户点击归档入口是唯一发起源）
