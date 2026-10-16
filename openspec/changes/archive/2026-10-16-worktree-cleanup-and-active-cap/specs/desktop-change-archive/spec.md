# desktop-change-archive Specification (Delta)

## MODIFIED Requirements

### Requirement: 归档入口与前置确认面

change 详情页 SHALL 以 run 流程的 archive 相位为归档入口：db `ChangeRecord.status=active` 的 change 在 run 走完 8 个路由相位后 SHALL 停在 archive 相位之前（stop-before-archive 停等面），该停等面 SHALL 呈现归档前置确认数据——完成度核对结论（以该 change id 的 db PhaseRecord eval 序列确定性核算——workflow 相位表全部路由相位存在 non-stale 的 pass / skipped 条目即完成，MUST NOT 读取 workflow.json 或调 MCP `change_list`）、delta specs 在场情况、worktree 记录在场时「将执行提交 + 合入主仓当前分支、收口后自动移除 worktree（分支 `change/<name>` 保留）」的告知（清理语义见「Cleanup 步与 worktree 自动清理」——删除磁盘目录为用户可感知动作，确认在先）。警告 SHALL NOT 阻断（inform + confirm 护栏为本能力自持语义），用户显式确认后 walker 才进入 archive 相位；未确认时 run 停在 archive 前（不自动归档）。已归档（`source=archive` / status=archived）SHALL NOT 再进入 archive 相位（`phase_next` 已路由 done）；无 db 建档的 change（id 不可达）SHALL NOT 提供归档（写面既有拒绝面沿袭）。运行中 run 已在该 run 内，归档相位为该 run 的尾部（零独立归档链互斥面）。

#### Scenario: 未完成警告确认后照常进入归档相位

- **WHEN** 一个存在 fail 相位（无全相位 non-stale pass）的 active change 的 run 到达 archive 前停等面，用户在停等面选择确认
- **THEN** 停等面呈现「工作流未全部通过」警告，确认后 walker 进入 archive 相位并走通收口；警告不构成阻断

#### Scenario: 未确认不自动归档

- **WHEN** run 走完 8 个路由相位到达 archive 前停等面而用户未确认
- **THEN** walker 停在 archive 前（stop-before-archive），不进入 archive 相位、零归档变更；run 处于停等态，用户可确认续走或终止

#### Scenario: 已归档与无建档 id 无入口

- **WHEN** 打开已归档 change（archive 树条目）详情，并审查无 db 记录的 id 情形
- **THEN** 已归档 change 的 `phase_next` 路由 done、不再进入 archive 相位；无建档 id 即使经命令面直调也被写面既有拒绝面拒绝（零落账、零目录改动）

#### Scenario: worktree 清理告知在场

- **WHEN** 带 worktree 记录的 active change 的 run 到达 archive 前停等面
- **THEN** 确认面呈现「将执行提交 + 合入主仓当前分支、收口后自动移除 worktree（分支保留）」告知；legacy（`worktree=None`）change 的确认面零清理告知

### Requirement: 归档编排链与阶段幂等

归档 SHALL 为 walker 的第 10 个路由相位（`archive`，确定性相位：`PhaseDefinition.executor=None` / `evaluator=None`），11 步有序 step 序列 SHALL 为：`PhaseStart`（开相，attempt max+1 分配）→ `Preflight`（store 读 + vcs 探测的只读校验，detail 携完成度警告 / delta specs 清单 / merge 目标）→ `Commit`（vcs.commit_all，step 内「干净」跳过）→ `Rebase`（vcs.rebase_branch 重放至主仓当前分支，产出冲突清单入 `PhaseCursor.rebase_conflicts`）→ `ResolveConflicts`（条件执行：无冲突 `passed` + `detail` 记「无冲突」；有冲突经解冲突 agent 会话解算）→ `Merge`（vcs.ff_merge 主仓 ff-only 快进）→ `SpecSync`（delta specs 在场且未跳过时 agent 会话，否则 `passed` + `detail` 记跳过）→ `Cleanup`（vcs worktree 移除尽力步——语义见「Cleanup 步与 worktree 自动清理」）→ `Seal`（经写面 `archive` 双写收口：目录改名 + db status 翻转）→ `Finalize`（vcs.commit_paths 落盘提交，step 内「已落盘」跳过）→ `PhaseLog`（空 report + 空 checklist 落 `verdict=pass` 的 PhaseRecord）。Cleanup 与 Finalize 的相对座位由 design 定稿（Finalize 语义不变：主仓归档两路径 + delta capability 主 specs 的脏探测落盘）。每步 SHALL 幂等可重入：**worktree 干净（`status --porcelain` 非空为假）是 Commit 步唯一跳过依据——分支可达性 MUST NOT 构成 Commit 步的跳过分支**；分支已合入则 Rebase / Merge 步幂等跳过（重放已在目标中的提交显式丢弃、ff-only 快进零 merge commit）；Cleanup 幂等面见其单点 requirement（worktree 已不在场即跳过）；双写半完成重试走写面既有续半边（仅补 db 翻转）。delta specs 缺席或用户选择跳过时 SpecSync SHALL 零会话（`passed` + 跳过因）。除 Cleanup（尽力步，步恒 `passed`）外任一步失败 SHALL 以 `Terminal` 收口（`on_fail=None` 的步失败即 run 失败），MUST NOT 静默跳过或降级；重试靠 step 内判断短路（干净 / 已合入 / 已落盘 / worktree 已不在场 / 双写续半边），已成功步不重复执行。

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

### Requirement: V1 范围与边界留痕

以下边界 SHALL 作为 V1 显式限制留痕（后续变更偿还，不由实现隐式吸收）：

1. **branch 清理仍手动**：worktree 清理由 archive 相位 Cleanup 步自动移除（本变更偿还 V1 手动边界，见「Cleanup 步与 worktree 自动清理」）；branch `change/<name>` 保留，`git branch -d` 仍由用户手动执行（历史可溯）。
2. **legacy 提交面手动**：`worktree=None` change 的主仓编辑提交仍由用户手动完成，archive 相位不代劳。
3. **主 spec 同步仅归档时**：specs 基线同步只发生在 archive 相位的 SpecSync 步；run 的其余相位不做主基线同步（proposal 相位只写 change 目录内 delta）。
4. **archive 相位不处理主仓无关改动**：主仓无关未提交 / staged 内容原样保留，处置归用户。
5. **无 delta specs 不触碰主 spec**：缺席 delta specs 时主基线零变更（SpecSync 步跳过因注记）。
6. **归档无 evaluator 无 checklist**：archive 相位不落评估 checklist（空 checklist 落账仅为「相位已 pass」的 PhaseRecord 标记，非评估证据面）。
7. **裸 `archive_change` 逃生口漏清**：不经 archive 相位的裸双写命令（D11 逃生口，无前端入口）无 Cleanup 步、不清理 worktree——是否补 best-effort 清理由 design 定稿（若补落命令层，MUST NOT 进写面 `archive` 双写本体）；残留由 create 前置 worktree 磁盘保底门槛兜底拦截（见 desktop-change-create）。
8. **弃置（abandon）另立变更**：active 占额度但无退路的弃置面（清 worktree / branch + 记录处置）本轮不做；create 上限错误文案引导「归档或手动清理」偿还。

#### Scenario: 边界留痕可考

- **WHEN** 查阅本 spec
- **THEN** 八条边界均可考，后续变更无需重新论证是否知情

## ADDED Requirements

### Requirement: Cleanup 步与 worktree 自动清理

archive 相位 SHALL 在 SpecSync 步之后、Seal 步之前执行 `Cleanup` 步，对带 worktree 记录（`ChangeRecord.worktree = Some`）的 change 以 `git worktree remove` 移除该 worktree（`--force` 与否由 design 定稿——bootstrap 的 `node_modules` / `target` 残留可由 `--force` 一并带走）；branch `change/<name>` MUST NOT 删除（`worktree remove` 本就不动分支，历史可溯）。Cleanup SHALL 为**尽力步**：清理失败 SHALL 仅以 step detail 警告呈现（含 worktree 路径与手动清理指引），步状态 `passed`、链照常收口（Seal / Finalize / PhaseLog 续走——归档必收口，残留是警告不是回滚），MUST NOT 以 `Terminal` 停链、MUST NOT 扩 `ArchiveOutcome` 字段面（golden wire 冻结不动）。Cleanup SHALL 幂等：worktree 目录已不在场（用户手动清理 / 重试链）时 SHALL `passed` + `detail` 注记跳过（MUST NOT 失败）；legacy 记录（`worktree=None`）SHALL 跳过该步（`passed` + `detail` 记跳过因）。清理的 git 触点 SHALL 经 orchestration 侧 `ArchiveVcsPort` 缝承载（进程执行驻 `crates/infra/vcs`），MUST NOT 进入写面 `archive` 双写本体（desktop-change-state-store「归档双写」零 git 触点红线保持）。清理完成后 `ChangeRecord.worktree` 字段 SHALL 原样保留（历史锚——change 已 archived 无后续操作，字段作创建期执行锚留痕；详情信息行照旧呈现路径，「已清理」注记为 design 可选非必须项）。

#### Scenario: 清理成功收口

- **WHEN** 带 worktree 记录的 change 走 archive 相位至 Cleanup 步成功
- **THEN** worktree 目录自磁盘消失（`git worktree list` 不再列出），Seal 照常双写收口（db status=archived + 主仓目录改名）；`ChangeRecord.worktree` 字段原值保留

#### Scenario: 清理失败仅警告不断链

- **WHEN** Cleanup 步的 `git worktree remove` 非零退出
- **THEN** Cleanup 步 `passed` + `detail` 携警告（worktree 路径 + 手动清理指引），Seal / Finalize / PhaseLog 照常收口、run 正常完成；`ArchiveOutcome` 字段面零变化

#### Scenario: worktree 缺席幂等跳过

- **WHEN** 用户已手动删除 worktree 目录后经重试链再次走到 Cleanup 步
- **THEN** Cleanup 步 `passed` + `detail` 注记「worktree 已不在场，跳过清理」，零失败面、链续走

#### Scenario: legacy 记录跳过清理

- **WHEN** 对 `worktree=None` 的存量建档 change 进入 archive 相位
- **THEN** Cleanup 步 `passed` + `detail` 记跳过因，零 `git worktree` 触点，链径直经 Seal / Finalize 收口

#### Scenario: 分支保留可溯

- **WHEN** 归档全链（含 Cleanup 成功与失败两路径）完成后审查主仓 branch 清单
- **THEN** branch `change/<name>` 在案（全程零 `git branch -d` 调用），历史可溯

## Module Contract

| 模块 | 职责 | 关键契约 |
|------|------|----------|
| `crates/core/workflow/src/write/phase_table.rs`（ARCHIVE_STEPS） | 11 步序列单点 | SpecSync → Cleanup → Seal（Cleanup 尽力步；与 Finalize 相对座位 design 定稿）；其余步 `on_fail=None` |
| `crates/core/orchestration/src/walker.rs`（step_cleanup，新） | Cleanup 步执行体 | 记录解析 worktree 字段 → 缺席 / legacy 幂等跳过（`passed` + detail）；remove 失败吞为 detail 警告（步恒 `passed`、链必收口）；零 branch 触点 |
| `crates/core/orchestration/src/port.rs`（ArchiveVcsPort 清理面，扩） | 归档链 → vcs 清理的进程内缝 | worktree 在场探测 + 移除（形态 design 定稿）；实现复用 `crates/infra/vcs` 既有 `remove_worktree --force` 单点或等价；`Err` 面带路径与手动清理引导 |
| `crates/infra/vcs/src/git.rs` | git 工作面进程执行 | `git worktree remove [--force]`（既有补偿面单点复用或等价新单点）；真实 git tempdir 夹具可测 |
| `packages/desktop/src/views/changes/`（stop-before-archive 停等面） | 清理告知 | worktree 记录在场时确认面告知含「收口后自动移除 worktree（分支保留）」；legacy 零清理告知 |
| `crates/core/workflow/src/write/archive.rs`（不动） | 双写本体零 git 触点 | Cleanup 步不属双写本体；写面 `archive` 语义零改动 |
