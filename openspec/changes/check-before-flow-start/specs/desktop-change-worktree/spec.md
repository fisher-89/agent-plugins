# desktop-change-worktree Specification (Delta)

## MODIFIED Requirements

### Requirement: merge-first 归档引导

带 worktree 记录的 change 归档 SHALL 以主仓目录在场为前置：主仓 active / archive 两树均未命中该 change 目录时，直接归档 SHALL 显式拒绝并引导「先 merge worktree 分支（change/<name>）回主仓再归档」（MUST NOT 泛化「目录未找到」了事）；主仓出现 `changes/<n>/` 后，既有归档双写（db status 翻转 + 主仓目录改名）SHALL 零改动可用。

满足前置的两条等价路径：**archive 相位自动段**（desktop-change-archive：run 的 archive 相位在 Seal 双写收口前经 Commit / Rebase / ResolveConflicts / Merge 步自动执行 worktree 提交 + 主仓合入，为用户确认进入 archive 相位后的缺省路径）与**用户手动 merge**（既有路径维持，archive 相位的 step 内判断将其视为已完成段）。merge SHALL NOT 被强制：用户可不 merge 弃置 change（V1 弃置 = 手动清 worktree / branch，db 记录保留，见「V1 范围与边界留痕」）。自动合入的冲突处置（worktree 内 rebase 冲突态保留 + 解冲突 agent 裁决，agent 无法裁决时 lean 档停链咨询用户）与主仓状态约束（不吞并无关改动、不改写主仓历史）由 desktop-change-archive 约束。run 收口语义改写：全前序路由相位（一次性 initialize + 内容相位）pass 后停等 archive 前（「等待确认进入归档」），MUST NOT 自动归档、MUST NOT 自动 merge（archive 相位由用户确认进入，非 run 收口的自动动作）；worktree 清理由 archive 相位 Cleanup 步在双写收口前自动执行（尽力步，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」——V1 手动边界就此偿还），branch `change/<name>` 保留（删除仍为用户手动边界）。

#### Scenario: archive 相位自动提交合入满足前置

- **WHEN** 带 worktree 记录且主仓两树均未命中目录的 change 经用户确认进入 archive 相位，Commit / Rebase / ResolveConflicts / Merge 步成功（含零提交分支形态——Commit 步不因分支基线可达主仓 HEAD 而跳过）
- **THEN** 主仓出现 `changes/<n>/`，Seal 双写收口零特判走通（无 worktree 路径分支进入收口步）；用户全程无手动 git 操作

#### Scenario: 手动 merge 后 archive 相位跳过合入

- **WHEN** 用户已手动将 worktree 分支 merge 回主仓（主仓出现 `changes/<n>/`）后进入 archive 相位
- **THEN** Commit 步按干净探测跳过、Rebase / Merge 步幂等跳过（零 git 动作），链径直至 Cleanup / Seal / Finalize 收口；spec 同步在主仓当前基线执行（产物不再滞留 worktree）；直接经裸归档命令调用亦零改动可用（既有语义）

#### Scenario: 未 merge 的直接归档仍被引导拒绝

- **WHEN** 绕过 archive 相位对带 worktree 记录且主仓两树均未命中目录的 change 直接调用归档写面
- **THEN** 显式拒绝且错误文案引导先 merge worktree 分支；db 与磁盘零变化（既有引导语义不因 archive 相位在场而退役）

#### Scenario: run 收口不自动合入

- **WHEN** 带 worktree 记录的 change run 走完全部前序路由相位（含一次性 initialize）
- **THEN** run 停在 archive 前（零自动 merge、零自动归档、零 archive 相位触发），用户显式确认进入 archive 相位是唯一发起源

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/create.rs`（零改动面） | worktree 建域 | 基线恒 HEAD、目录树与 bootstrap 语义零改动；initialize 的 clean 步在既有 worktree 面上恢复基线（不新增建域语义） |
| `crates/infra/vcs`（restore_baseline，新） | 基线恢复 | 经 `ArchiveVcsPort::restore_baseline` 缝执行 `reset --hard` + `clean -fd` 排除面（foundation 名单 + `openspec/`）；仅 worktree 在场时被调用（walker 保证），主仓零重置 |
