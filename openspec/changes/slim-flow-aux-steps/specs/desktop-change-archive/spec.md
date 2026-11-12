# desktop-change-archive Specification (Delta)

## MODIFIED Requirements

### Requirement: 归档编排链与阶段幂等

归档 SHALL 为 walker 的第 11 个路由相位（`archive`，确定性相位：`PhaseDefinition.executor=None` / `evaluator=None`——initialize 表头相位之后的末相位），9 步有序 step 序列 SHALL 为：`Preflight`（store 读 + vcs 探测的只读校验，detail 携完成度警告 / delta specs 清单 / merge 目标）→ `Commit`（vcs.commit_all，step 内「干净」跳过）→ `Rebase`（vcs.rebase_branch 重放至主仓当前分支，产出冲突清单入 `PhaseCursor.rebase_conflicts`）→ `ResolveConflicts`（条件执行：无冲突 `passed` + `detail` 记「无冲突」；有冲突经解冲突 agent 会话解算）→ `Merge`（vcs.ff_merge 主仓 ff-only 快进）→ `SpecSync`（delta specs 在场且未跳过时 agent 会话，否则 `passed` + `detail` 记跳过）→ `Cleanup`（vcs worktree 移除尽力步——语义见「Cleanup 步与 worktree 自动清理」）→ `Seal`（经写面 `archive` 双写收口：目录改名 + db status 翻转）→ `Finalize`（vcs.commit_paths 落盘提交，step 内「已落盘」跳过）。相位的开相与落账 SHALL 由相位机固定动作承载（`enter_phase` / `settle_phase`，见 desktop-change-orchestration「相位入口与收口内化（计划图仅含真实步）」）：attempt 分配在 `enter_phase`（含 `start_at` 写 active_phase），落账由 `settle_phase` 以空 report + 空 checklist 落 `verdict=pass` 的 PhaseRecord——二者 MUST NOT 是计划步、MUST NOT 出图节点、MUST NOT 产 `StepRecord` 审计行。Cleanup 与 Finalize 的相对座位由 design 定稿（Finalize 语义不变：主仓归档两路径 + delta capability 主 specs 的脏探测落盘）。每步 SHALL 幂等可重入：**worktree 干净（`status --porcelain` 非空为假）是 Commit 步唯一跳过依据——分支可达性 MUST NOT 构成 Commit 步的跳过分支**；分支已合入则 Rebase / Merge 步幂等跳过（重放已在目标中的提交显式丢弃、ff-only 快进零 merge commit）；Cleanup 幂等面见其单点 requirement（worktree 已不在场即跳过）；双写半完成重试走写面既有续半边（仅补 db 翻转）。delta specs 缺席或用户选择跳过时 SpecSync SHALL 零会话（`passed` + 跳过因）。除 Cleanup（尽力步，步恒 `passed`）外任一步失败 SHALL 以 `Terminal` 收口（`on_fail=None` 的步失败即 run 失败），MUST NOT 静默跳过或降级；重试靠 step 内判断短路（干净 / 已合入 / 已落盘 / worktree 已不在场 / 双写续半边），已成功步不重复执行。相位收口落账失败 SHALL 以「相位 `archive` 全部步通过但落账失败」为 reason 前缀收 `Terminal::failed`（节点保持全绿，见 desktop-change-orchestration「相位收口失败可辨与相位内系统异常仍标 failed」）。

**Preflight 的 rebase 半程挡回（本期增补）**：`Preflight` SHALL 在任何写动作之前（`Commit` 之前）以既有 `worktree_snapshot()` 的 `rebase_head` 在场性检出「上一次归档失败后留下的 rebase 半程」，命中即以 `Terminal` 收口挡回重跑，MUST NOT 继续走 `Commit`：

- **理由（防污染）**：`ResolveConflicts` 的模型冲突分流（见「worktree 提交与主仓合入」）**保留** rebase 半程（MUST NOT 自动 `rebase --abort`，冲突面囚于 worktree）。用户直接重跑归档时，`Commit` 步会以 `git add -A` + `git commit` 在 rebase 中间态产生提交——这类污染极难排查。
- **文案三要素**：`Terminal` 文案 SHALL 含 ① worktree **绝对路径**；② 出路一「在该 worktree 内解完模型冲突后 `git rebase --continue`，再重跑归档」；③ 出路二「放弃本次重放：`git rebase --abort`，再重跑归档（会重新 rebase 一次）」。两条命令 SHALL 可直接照抄。
- **零新 git 能力**：检出只经既有 port 方法，MUST NOT 新增 vcs 能力、MUST NOT 在 `Preflight` 内 spawn git。
- **放行态**：半程不在场（含用户已 `--continue` 或已 `--abort` 后重跑）SHALL 照常放行，后续步按既有幂等短路续走。
- **连带成本（知情接受）**：半程期间该 change 持续占用 active 名额（上限 5）且 worktree 不可被 `Cleanup` 删除；前端失败面 MUST 呈现同一文案（worktree 绝对路径 + 两条命令）。

#### Scenario: 9 步按序执行

- **WHEN** 假引擎 + 假 vcs port + 假写面驱动 archive 相位
- **THEN** 9 步按序执行：Preflight 校验、Commit 提交（或干净跳过）、Rebase 重放、ResolveConflicts 条件解算、Merge 快进、SpecSync 同步（或跳过）、Cleanup 清理（或幂等跳过）、Seal 双写、Finalize 落盘（或跳过）；开相由 `enter_phase` 完成、空 checklist 落账由相位收口完成，二者零节点零审计

#### Scenario: 零提交分支脏工作区照常提交合入

- **WHEN** 桌面执行相位全程未提交（分支零提交、tip = 创建基线）且主仓 HEAD 已前进，worktree 内编辑 / staged / change 目录在场
- **THEN** Commit 步照常产生提交（不因基线可达 HEAD 而跳过）、Rebase 重放 + Merge 快进走通（主仓 HEAD 前移、零 merge commit），写面 merge-first 守卫零触发

#### Scenario: 已合入跳过合入直达收口

- **WHEN** 分支已 merge 回主仓（主仓 active 树已含 `changes/<n>/`）且 worktree 干净
- **THEN** Commit 步零动作（干净跳过）、Rebase / Merge 步幂等跳过（已合入），链径直到 SpecSync / Cleanup / Seal / Finalize 与相位收口

#### Scenario: 步失败即 Terminal 收口

- **WHEN** archive 相位某步失败（commit 失败 / rebase 冲突无法裁决 / seal 半完成 / spec_sync 失败）
- **THEN** 该步以 `Terminal` 收口、run 直接 `Failed`，不唤决策 agent、不触发 phase retry；db 与磁盘保持该步失败前状态；重试经 step 内判断短路续走（Cleanup 步除外——其失败以 detail 警告呈现、永不 Terminal，见「Cleanup 步与 worktree 自动清理」）

#### Scenario: 步全绿但落账失败可辨

- **WHEN** archive 相位 9 步全部 passed 后相位收口的 `log_phase` 返回 `Err`
- **THEN** 全部节点终态 passed（零红节点）、run `failed`、reason 前缀「相位 `archive` 全部步通过但落账失败」并携 `Err` 文案；状态库无该 attempt 的 PhaseRecord

#### Scenario: rebase 半程挡回重跑

- **WHEN** 上一次归档在 `ResolveConflicts` 以模型冲突失败（rebase 半程保留于 worktree，`REBASE_HEAD` 在场），用户未处置直接重跑归档
- **THEN** `Preflight` 步以 `Terminal` 收口挡回：文案含 worktree 绝对路径与两条处置命令（`git rebase --continue` / `git rebase --abort` 后各自重跑）；`Commit` 步零执行（不产生 rebase 中间态提交），db 与磁盘零变化

#### Scenario: 半程已处置则放行

- **WHEN** 用户在 worktree 内已 `git rebase --continue`（或已 `git rebase --abort`）后重跑归档
- **THEN** `Preflight` 因 `REBASE_HEAD` 不在场而放行；`Commit` 按干净或脏两态处置、`Rebase` / `Merge` 按已合入与否幂等短路，链续走至收口

### Requirement: 归档作为 run 相位与手动门

archive 相位 SHALL 是 run 状态机的一部分（第 11 个路由相位），零独立归档链事件面：归档节点 / 边走当前 run 的 `RunNodeRecord` / `RunEdgeRecord` 整包落库，归档实时流经既有 `RunUpdate` / `RunNotice` 变更通知（`archive_flow` 独立 `ArchiveUpdate` / `ArchiveSink` 退役）。「独立手动执行」SHALL 由 stop-before-archive 停等面承载：run 走完全部前序路由相位（一次性 initialize + 内容相位）后 SHALL 停在 archive 前，MUST NOT 自动推进进入 archive 相位；用户显式确认后才进入 archive 相位；archive 相位完成后 run 才 `completed`（"Ready for archiving" 停等语义改写为「等待确认进入归档」）。归档相位的停止 SHALL 复用 run 的停止命令（停止 run 自然停归档相位）。

**本变更为本 requirement 带来的改动点（逐条自足）**：① 相位的开相与落账 SHALL 由相位机固定动作承载（`enter_phase` 写 active_phase + `start_at` + attempt、`settle_phase` 以空 checklist 落账，见 desktop-change-orchestration「相位入口与收口内化（计划图仅含真实步）」），二者 MUST NOT 出图节点、MUST NOT 产审计行、MUST NOT 计入 `ARCHIVE_STEPS`——归档相位由此从 11 步收为 9 步计划步。② **stop-before 停等面语义 SHALL 逐字不变**：停等触发时机（走完全部前序路由相位后停在 archive 前）、确认载荷（完成度警告 / delta specs 清单 / merge 目标）、「未确认不入相位」「确认后才 `enter_phase` + 9 步」、「停止 run 自然停在 archive 前」五条均 MUST NOT 因辅助步退场而改变；辅助步退场只影响 archive 相位内（`enter_phase` 之后）的可观测面，不触及停等面本身。

#### Scenario: 归档节点进 run 执行图

- **WHEN** archive 相位一次 run 收口后查询执行图
- **THEN** 归档 9 个 step 节点（Preflight / Commit / Rebase / ResolveConflicts / Merge / SpecSync / Cleanup / Seal / Finalize）与串行 forward 边落当前 run 的 `RunNodeRecord` / `RunEdgeRecord`（零新表、零独立归档链事件），详情页 refresh 后归档列全史常驻；无 `phase_start` / `phase_log` 节点，相位落账体现为 PhaseRecord 与 `active_phase` 清位

#### Scenario: 停等面承载手动门

- **WHEN** run 走完全部前序路由相位（含一次性 initialize）到达 archive 前
- **THEN** 停等面呈现（archive 预检载荷照常随行），用户确认后进入 archive 相位（`enter_phase` + 9 步计划步）；run 未确认前不 completed、不归档；停止 run 自然停在 archive 前（零独立归档链停止命令）

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/phase_table.rs`（`ARCHIVE_STEPS`） | 9 步序列单点 | `Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Cleanup → Seal → Finalize`；`PhaseStart` / `PhaseLog` 两项删除；其余步 `on_fail=None`；`ARCHIVE_STEP_ORDER` 断言 11 → 9 |
| `crates/core/orchestration/src/walker.rs`（archive 相位执行面） | 相位序随动 | 归档步执行体（`step_preflight` / `step_commit` / `step_rebase` / `step_resolve_conflicts` / `step_merge` / `step_spec_sync` / `step_cleanup` / `step_seal` / `step_finalize`）逐字不动；开相与落账移出 `steps`，由 `enter_phase` / `settle_phase` 承载 |
| `crates/core/orchestration/src/walker.rs`（`open_step` / `close_step`） | 归档步节点辅助合一 | 由 `begin_archive_node` / `settle_archive_node` 合流（二者只差「是否先发 running」）；九步调用点语义零变 |
| `crates/core/orchestration/src/walker.rs`（`settle_phase`） | 归档空 checklist 落账 | archive 相位分支（`test_outcome` / `evaluator_turn` 双缺席 → 空 checklist → `derive_verdict([]) = Pass`）语义逐字保留；落账失败 → `Terminal::failed`「相位 archive 全部步通过但落账失败」 |
