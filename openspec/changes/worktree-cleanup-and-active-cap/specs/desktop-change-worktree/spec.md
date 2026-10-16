# desktop-change-worktree Specification (Delta)

## MODIFIED Requirements

### Requirement: merge-first 归档引导

带 worktree 记录的 change 归档 SHALL 以主仓目录在场为前置：主仓 active / archive 两树均未命中该 change 目录时，直接归档 SHALL 显式拒绝并引导「先 merge worktree 分支（change/<name>）回主仓再归档」（MUST NOT 泛化「目录未找到」了事）；主仓出现 `changes/<n>/` 后，既有归档双写（db status 翻转 + 主仓目录改名）SHALL 零改动可用。

满足前置的两条等价路径：**archive 相位自动段**（desktop-change-archive：run 的 archive 相位在 Seal 双写收口前经 Commit / Rebase / ResolveConflicts / Merge 步自动执行 worktree 提交 + 主仓合入，为用户确认进入 archive 相位后的缺省路径）与**用户手动 merge**（既有路径维持，archive 相位的 step 内判断将其视为已完成段）。merge SHALL NOT 被强制：用户可不 merge 弃置 change（V1 弃置 = 手动清 worktree / branch，db 记录保留，见「V1 范围与边界留痕」）。自动合入的冲突处置（worktree 内 rebase 冲突态保留 + 解冲突 agent 裁决，agent 无法裁决时 lean 档停链咨询用户）与主仓状态约束（不吞并无关改动、不改写主仓历史）由 desktop-change-archive 约束。run 收口语义改写：全相位 pass 后停等 archive 前（「等待确认进入归档」），MUST NOT 自动归档、MUST NOT 自动 merge（archive 相位由用户确认进入，非 run 收口的自动动作）；worktree 清理由 archive 相位 Cleanup 步在双写收口前自动执行（尽力步，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」——V1 手动边界就此偿还），branch `change/<name>` 保留（删除仍为用户手动边界）。

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

- **WHEN** 带 worktree 记录的 change run 走完 8 个路由相位
- **THEN** run 停在 archive 前（零自动 merge、零自动归档、零 archive 相位触发），用户显式确认进入 archive 相位是唯一发起源

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **branch 清理手动（worktree 清理已偿还）**：worktree 由 archive 相位 Cleanup 步自动移除（`git worktree remove` 尽力步，见 desktop-change-archive「Cleanup 步与 worktree 自动清理」）；branch `change/<name>` 保留，`git branch -d` 仍由用户手动执行；桌面 cleanup 按钮对 worktree 面不再必要。
2. **legacy 主 root 路径**：`worktree=None` 存量记录照旧主 root 执行，零迁移；非 git 仓 workspace 的 create 显式拒绝（不提供 legacy 创建逃生口，真实诉求另立变更）。
3. **`agent_start` 与探索笔记留主仓 root**：exec 轨道手动会话与 `openspec/explores/` 笔记不混入 change worktree。
4. **config / AGENT.md 冻结**：worktree 内配置停在创建时刻（base commit 检出），主仓中途改动不传导，文档化。
5. **构建成本接受**：fresh worktree 依赖 / 构建产物全量重建；共享 target 目录类逃生口（`CARGO_TARGET_DIR` 等）仅文档化，不内建机制。
6. **孤儿 worktree 对账文档化**：create 中途崩溃遗留的 worktree / branch 由 `git worktree list` 对账、幂等采纳或手动清理（前置校验保证下次创建显式报错而非误用），不内建自动清理；create 前置 worktree 磁盘保底门槛为失控防线（见 desktop-change-create）。
7. **report_dir 绝对路径接受失效**：phase_log 报告文本内嵌的 worktree 绝对路径在 worktree 删除后失效（转录历史，仅展示面），不做相对化；归档 Cleanup 步使删除成为缺省路径，本边界维持。
8. **并行 merge 冲突处置分层**：两 change 改同一文件各自绿、merge 撞时——经 archive 相位自动合入的冲突由 Rebase / ResolveConflicts 步的解冲突 agent 裁决（agent 无法裁决时 lean 档停链转人工，见 desktop-change-archive）；链外用户手动 merge 的冲突走 git 原生冲突处理。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 八条边界均可考，后续变更无需重新论证是否知情

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| （零实现触点——本能力清理条款由 desktop-change-archive Cleanup 步承载） | 边界改写 | worktree 清理自动化偿还归属 archive 相位；branch 生命周期仍手动；`worktrees/{身份段}/` 落位与身份段派生单点零改动（目录数扫描由 desktop-change-create 前置⑨消费） |
