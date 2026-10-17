# desktop-change-archive Specification (Delta)

## MODIFIED Requirements

### Requirement: 归档入口与前置确认面

change 详情页 SHALL 以 run 流程的 archive 相位为归档入口：db `ChangeRecord.status=active` 的 change 在 run 走完全部前序路由相位（一次性 initialize + 内容相位）后 SHALL 停在 archive 相位之前（stop-before-archive 停等面），该停等面 SHALL 呈现归档前置确认数据——完成度核对结论（以该 change id 的 db PhaseRecord eval 序列确定性核算——workflow 相位表全部路由相位存在 non-stale 的 pass / skipped 条目即完成（initialize 的一次性 pass / skipped 条目含于核算面），MUST NOT 读取 workflow.json 或调 MCP `change_list`）、delta specs 在场情况、worktree 记录在场时「将执行提交 + 合入主仓当前分支、收口后自动移除 worktree（分支 `change/<name>` 保留）」的告知（清理语义见「Cleanup 步与 worktree 自动清理」——删除磁盘目录为用户可感知动作，确认在先）。警告 SHALL NOT 阻断（inform + confirm 护栏为本能力自持语义），用户显式确认后 walker 才进入 archive 相位；未确认时 run 停在 archive 前（不自动归档）。已归档（`source=archive` / status=archived）SHALL NOT 再进入 archive 相位（`phase_next` 已路由 done）；无 db 建档的 change（id 不可达）SHALL NOT 提供归档（写面既有拒绝面沿袭）。运行中 run 已在该 run 内，归档相位为该 run 的尾部（零独立归档链互斥面）。

#### Scenario: 未完成警告确认后照常进入归档相位

- **WHEN** 一个存在 fail 相位（无全相位 non-stale pass）的 active change 的 run 到达 archive 前停等面，用户在停等面选择确认
- **THEN** 停等面呈现「工作流未全部通过」警告，确认后 walker 进入 archive 相位并走通收口；警告不构成阻断

#### Scenario: 未确认不自动归档

- **WHEN** run 走完全部前序路由相位（含一次性 initialize）到达 archive 前停等面而用户未确认
- **THEN** walker 停在 archive 前（stop-before-archive），不进入 archive 相位、零归档变更；run 处于停等态，用户可确认续走或终止

#### Scenario: 已归档与无建档 id 无入口

- **WHEN** 打开已归档 change（archive 树条目）详情，并审查无 db 记录的 id 情形
- **THEN** 已归档 change 的 `phase_next` 路由 done、不再进入 archive 相位；无建档 id 即使经命令面直调也被写面既有拒绝面拒绝（零落账、零目录改动）

#### Scenario: worktree 清理告知在场

- **WHEN** 带 worktree 记录的 active change 的 run 到达 archive 前停等面
- **THEN** 确认面呈现「将执行提交 + 合入主仓当前分支、收口后自动移除 worktree（分支保留）」告知；legacy（`worktree=None`）change 的确认面零清理告知

### Requirement: 归档编排链与阶段幂等

归档 SHALL 为 walker 的第 11 个路由相位（`archive`，确定性相位：`PhaseDefinition.executor=None` / `evaluator=None`——initialize 表头相位之后的末相位），11 步有序 step 序列 SHALL 为：`PhaseStart`（开相，attempt max+1 分配）→ `Preflight`（store 读 + vcs 探测的只读校验，detail 携完成度警告 / delta specs 清单 / merge 目标）→ `Commit`（vcs.commit_all，step 内「干净」跳过）→ `Rebase`（vcs.rebase_branch 重放至主仓当前分支，产出冲突清单入 `PhaseCursor.rebase_conflicts`）→ `ResolveConflicts`（条件执行：无冲突 `passed` + `detail` 记「无冲突」；有冲突经解冲突 agent 会话解算）→ `Merge`（vcs.ff_merge 主仓 ff-only 快进）→ `SpecSync`（delta specs 在场且未跳过时 agent 会话，否则 `passed` + `detail` 记跳过）→ `Cleanup`（vcs worktree 移除尽力步——语义见「Cleanup 步与 worktree 自动清理」）→ `Seal`（经写面 `archive` 双写收口：目录改名 + db status 翻转）→ `Finalize`（vcs.commit_paths 落盘提交，step 内「已落盘」跳过）→ `PhaseLog`（空 report + 空 checklist 落 `verdict=pass` 的 PhaseRecord）。Cleanup 与 Finalize 的相对座位由 design 定稿（Finalize 语义不变：主仓归档两路径 + delta capability 主 specs 的脏探测落盘）。每步 SHALL 幂等可重入：**worktree 干净（`status --porcelain` 非空为假）是 Commit 步唯一跳过依据——分支可达性 MUST NOT 构成 Commit 步的跳过分支**；分支已合入则 Rebase / Merge 步幂等跳过（重放已在目标中的提交显式丢弃、ff-only 快进零 merge commit）；Cleanup 幂等面见其单点 requirement（worktree 已不在场即跳过）；双写半完成重试走写面既有续半边（仅补 db 翻转）。delta specs 缺席或用户选择跳过时 SpecSync SHALL 零会话（`passed` + 跳过因）。除 Cleanup（尽力步，步恒 `passed`）外任一步失败 SHALL 以 `Terminal` 收口（`on_fail=None` 的步失败即 run 失败），MUST NOT 静默跳过或降级；重试靠 step 内判断短路（干净 / 已合入 / 已落盘 / worktree 已不在场 / 双写续半边），已成功步不重复执行。

#### Scenario: 11 步按序执行

- **WHEN** 假引擎 + 假 vcs port + 假写面驱动 archive 相位
- **THEN** 11 步按序执行：PhaseStart 开相、Preflight 校验、Commit 提交（或干净跳过）、Rebase 重放、ResolveConflicts 条件解算、Merge 快进、SpecSync 同步（或跳过）、Cleanup 清理（或幂等跳过）、Seal 双写、Finalize 落盘（或跳过）、PhaseLog 空 checklist 落账

#### Scenario: 零提交分支脏工作区照常提交合入

- **WHEN** 桌面执行相位全程未提交（分支零提交、tip = 创建基线）且主仓 HEAD 已前进，worktree 内编辑 / staged / change 目录在场
- **THEN** Commit 步照常产生提交（不因基线可达 HEAD 而跳过）、Rebase 重放 + Merge 快进走通（主仓 HEAD 前移、零 merge commit），写面 merge-first 守卫零触发

#### Scenario: 已合入跳过合入直达收口

- **WHEN** 分支已 merge 回主仓（主仓 active 树已含 `changes/<n>/`）且 worktree 干净
- **THEN** Commit 步零动作（干净跳过）、Rebase / Merge 步幂等跳过（已合入），链径直到 SpecSync / Cleanup / Seal / Finalize 收口

#### Scenario: 步失败即 Terminal 收口

- **WHEN** archive 相位某步失败（commit 失败 / rebase 冲突无法裁决 / seal 半完成 / spec_sync 失败）
- **THEN** 该步以 `Terminal` 收口、run 直接 `Failed`，不唤决策 agent、不触发 phase retry；db 与磁盘保持该步失败前状态；重试经 step 内判断短路续走（Cleanup 步除外——其失败以 detail 警告呈现、永不 Terminal，见「Cleanup 步与 worktree 自动清理」）

### Requirement: 归档作为 run 相位与手动门

archive 相位 SHALL 是 run 状态机的一部分（第 11 个路由相位），零独立归档链事件面：归档节点 / 边走当前 run 的 `RunNodeRecord` / `RunEdgeRecord` 整包落库，归档实时流经既有 `RunUpdate` / `RunNotice` 变更通知（`archive_flow` 独立 `ArchiveUpdate` / `ArchiveSink` 退役）。「独立手动执行」SHALL 由 stop-before-archive 停等面承载：run 走完全部前序路由相位（一次性 initialize + 内容相位）后 SHALL 停在 archive 前，MUST NOT 自动推进进入 archive 相位；用户显式确认后才进入 archive 相位；archive 相位完成后 run 才 `completed`（"Ready for archiving" 停等语义改写为「等待确认进入归档」）。归档相位的停止 SHALL 复用 run 的停止命令（停止 run 自然停归档相位）。

#### Scenario: 归档节点进 run 执行图

- **WHEN** archive 相位一次 run 收口后查询执行图
- **THEN** 归档 10 步节点与串行 forward 边落当前 run 的 `RunNodeRecord` / `RunEdgeRecord`（零新表、零独立归档链事件），详情页 refresh 后归档列全史常驻

#### Scenario: 停等面承载手动门

- **WHEN** run 走完全部前序路由相位（含一次性 initialize）到达 archive 前
- **THEN** 停等面呈现，用户确认后进入 archive 相位；run 未确认前不 completed、不归档；停止 run 自然停在 archive 前（零独立归档链停止命令）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/orchestration/src/walker.rs`（archive 相位，零改动面） | 相位序随动 | archive 仍为表末确定性相位（第 11 个路由相位）；11 步序列与幂等语义零改动——表头 initialize 插入不影响归档链 |
| 归档完成度核算（preflight / stop-before-archive 面板） | 核算面随动 | `all_passed` / `completion_of` 自动含 initialize 表项（表驱动）；零特判 |
