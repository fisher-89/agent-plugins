# 任务: merge-archive-steps-into-flow

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖实现文件。顺序按依赖排列：词汇单点 → 计划图单源与路由 → 详情 10 列 → 编排状态与 port → prompt 搬移 → steps.rs 内联步 → control.rs 停等面 → walker 归档相位 → archive_flow 退役 → 命令面收编 → bindings 再生 → 前端随动与退役 → 交付。
>
> 依赖序要点：walker.rs 消费 `ArchivePreflight`（state.rs）、`WorkerRole::SpecSync/ResolveConflicts` 与 `ToolCommand::ArchivePreflight/ArchiveSeal`（port.rs）、`LocalToolSteps::new` 的 vcs 注入（steps.rs）、`wait_confirm` 的 `(proceed, sync_specs)` 回流（control.rs）与 `spec_sync_prompt` / `merge_conflict_prompt`（prompt.rs），故这些先行于 walker；命令面装配依赖 `walk_run` 新签名与 `ProcessArchiveVcs` 注入；bindings 再生（阶段九）依赖命令面 DTO 演进（阶段八）并先于前端随动（阶段十，前端读 `bindings.ts` 类型）。

## 阶段一：core/workflow 词汇与计划图

- [x] 修改 `crates/core/workflow/src/state.rs`：`StepNode` +8（Preflight / Commit / Rebase / ResolveConflicts / Merge / SpecSync / Seal / Finalize，`as_str` snake_case 随动）、`RunStepKind` +8（同词面）、`StepGroup` +`Git`、`AgentRole` +`SpecSync` / `ResolveConflicts`；`StepNode::group()`（commit/rebase/merge/finalize→Git、preflight/seal→Exec、spec_sync/resolve_conflicts→Agent）与 `StepNode::role()`（spec_sync→SpecSync、resolve_conflicts→ResolveConflicts）随动
- [x] 修改 `crates/core/workflow/src/write/phase_table.rs`：新增 `ARCHIVE_STEPS`（PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog；`prompt_ref=None` / `on_fail=None` 恒值）；`REQUIREMENT_TABLE` 追加 archive `PhaseDefinition`（`executor=None` / `evaluator=None` / `artifact_manifest=&[]`）
- [x] 修改 `crates/core/workflow/src/write/phase_next.rs`：确认路由经 `REQUIREMENT_TABLE` 自动覆盖 archive（8 相位 pass → next=archive、archive 空 checklist 落账后 `all_passed` → done）；`has_phase_passed` / `allowed_backtrack_phases` 零特判（必要时仅注释同步）

## 阶段二：详情 10 列投影

- [x] 修改 `crates/core/workflow/src/queries/detail.rs`：`PIPELINE_PHASES` `[&str; 9]` → `[&str; 10]`（追加 `"archive"`）；注释同步（runs nodes 十八值随 `RunStepKind` 自动零过滤）

## 阶段三：orchestration 状态与 port

- [x] 修改 `crates/core/orchestration/src/state.rs`：`ChangeStepKind` +8（snake_case 词，`as_str` 随动）；新增 `ArchivePreflight { name, completed, incomplete_phases, missing_artifacts, delta_specs, worktree, branch, merge_target }`（serde + specta）；`RunUpdate::ConfirmWait` 改 `{ phase, archive: Option<ArchivePreflight> }`；`ChangeRunSnapshot` 增 `archive: Option<ArchivePreflight>`
- [x] 修改 `crates/core/orchestration/src/port.rs`：`WorkerRole` +`SpecSync` / `ResolveConflicts`（`as_str` = `"spec_sync"` / `"resolve_conflicts"`）；`ToolCommand` +`ArchivePreflight { change_id }` / `ArchiveSeal { change_id }`；`ToolStepOutput` +`ArchivePreflight(ArchivePreflight)` / `ArchiveSeal(workflow::write::ArchiveOutcome)`

## 阶段四：prompt 与内联步

- [x] 修改 `crates/core/orchestration/src/prompt.rs`：新增 `spec_sync_prompt(change) -> String` 与 `merge_conflict_prompt(change, conflicts: &[String]) -> String`（自 archive_flow.rs 文本搬移，零语义改动）
- [x] 修改 `crates/core/orchestration/src/steps.rs`：`LocalToolSteps::new` 增 `vcs: Arc<dyn ArchiveVcsPort>`；`execute` 增 `ToolCommand::ArchivePreflight`（搬移 `completion_of` / `missing_artifacts` / `detect_delta_specs` + `vcs.current_branch` / `branch_merged` → `ArchivePreflight`）与 `ToolCommand::ArchiveSeal`（`workflow::write::archive`）两分支，两分支不写 `StepRecord` 审计行；新增 `run_archive_preflight(main_root, store, id, vcs) -> Option<ArchivePreflight>`

## 阶段五：control.rs 停等面扩展

- [x] 修改 `crates/core/orchestration/src/control.rs`：`confirm` 增 `sync_specs: bool`（oneshot 载荷改 `(bool, bool)`）；`RunGuard::wait_confirm` 返回 `(bool, bool)`（取消 → `(false, false)` 语义）；`RunEntry` 增 `archive: Option<ArchivePreflight>`；`publish` 的 `ConfirmWait` 分支落 `entry.archive`；`snapshot` 出 `archive`

## 阶段六：walker 归档相位

- [x] 修改 `crates/core/orchestration/src/walker.rs`：`RunRequest` 增 `main_root: String` + `stop_before: Option<String>`；`walk_run` 增 `vcs: Arc<dyn ArchiveVcsPort>`
- [x] 修改 `crates/core/orchestration/src/walker.rs`：`drive` 删除 `next.evaluator.is_none()` 检查；循环顶部（`phase_next` 解析出 `next.next_phase` 后、进入 steps 前）增停止门——`next.next_phase == stop_before` 时经 `ToolCommand::ArchivePreflight` 算载荷、发 `ConfirmWait { phase, archive }`、`wait_confirm` 取 `(proceed, sync_specs)`（proceed=false/取消 → stopped）；收口后确认块跳过 `phase == stop_before`
- [x] 修改 `crates/core/orchestration/src/walker.rs`：`StepContext` 增 `vcs: &'a Arc<dyn ArchiveVcsPort>`；`PhaseCursor` 增 `preflight` / `rebase_conflicts` / `sync_specs` / `seal_outcome`
- [x] 修改 `crates/core/orchestration/src/walker.rs`：`run_step` 增 8 归档步分支（Preflight / Commit / Rebase / ResolveConflicts / Merge / SpecSync / Seal / Finalize）；新增对应 step 执行体（git 步直调 `ctx.vcs`；agent 步经 `run_worker`；Preflight / Seal 经 `ToolStepPort`）；Commit 跳过依据仅干净探测、Rebase 产出 `rebase_conflicts`、ResolveConflicts 条件解算 + `rebase_continue` + 后验、Merge 独立 `ff_merge`、SpecSync 按 `sync_specs` + delta_specs 条件会话、Finalize 按 `seal_outcome` + preflight 组 pathspec `commit_paths`；步失败一律 `Terminal`
- [x] 修改 `crates/core/orchestration/src/walker.rs`：`run_worker` 增 `root: &str` 参数（cwd 覆盖：SpecSync 传 `main_root`、ResolveConflicts 传 `root`/worktree、其余 `root`）并映射 `WorkerRole::SpecSync` / `ResolveConflicts` → `ChangeStepKind::SpecSync` / `ResolveConflicts`
- [x] 修改 `crates/core/orchestration/src/walker.rs`：`step_phase_log` 增空 checklist 第三分支（test_outcome 分支之后、verdict 分支之前——`PhaseLogInput { phase, report: String::new(), checklist: vec![], skipped: false, 三槽位 None }` 直接 `run_phase_log`）；`ALL_PHASES_PASSED` 文案改写为归档收口语义；`TryFrom<ToolStepOutput>` 增 `ArchivePreflight` / `ArchiveSeal` 两 impl

## 阶段七：archive_flow 退役

- [x] 删除 `crates/core/orchestration/src/archive_flow.rs`（+ 摘 `archive_flow_test` 声明）
- [x] 修改 `crates/core/orchestration/src/lib.rs`：摘 `pub mod archive_flow` 与全部 archive 再导出（`ArchiveControl` / `ArchiveGuard` / `ArchiveRequest` / `ArchiveStage` 等）；挂 `ArchivePreflight` 再导出

## 阶段八：命令面收编

- [x] 删除 `src/commands/archive_flow/mod.rs` + `mod_test.rs`
- [x] 修改 `src/commands/mod.rs`：摘 `pub mod archive_flow` 与 5 条 archive_flow 命令注册
- [x] 修改 `src/main.rs`：摘 `app.manage(Arc::new(ArchiveControl::new()))`
- [x] 修改 `src/commands/change_flow/mod.rs`：摘 `ArchiveControl` 反向互斥面；`change_flow_start` 装配 `vcs: Arc<dyn ArchiveVcsPort>`（`ProcessArchiveVcs`）、`main_root: root.clone()`、`stop_before: Some("archive")`，注入 `LocalToolSteps::new` 与 `walk_run`；`change_flow_confirm` 增 `sync_specs: bool` 透传 `control.confirm`
- [x] 修改 `src/commands/changes/mod.rs`：`ActiveRunView` 增 `archive: Option<orchestration::ArchivePreflight>`（自 `ChangeRunSnapshot` 投影）；`archive_change` 零改动保留

## 阶段九：bindings 再生（前端随动的前置）

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生成 `src/types/generated/bindings.ts`（`ArchivePreflight` / `ConfirmWait.archive` / `ActiveRunView.archive` / `changeFlowConfirm.syncSpecs` / 退役 archive_flow 五命令 / 十八值词汇，非手改）——先于阶段十前端随动（前端 `run-state.ts` / `run-control-panel.tsx` / `change-detail-view.tsx` 读 `bindings.ts` 新类型）

## 阶段十：前端随动与退役

- [x] 修改 `src/views/changes/flow/layout.ts`：`PIPELINE_PHASES` 追加 `"archive"`
- [x] 修改 `src/views/changes/flow/types.ts`：`RunStepGroup` +`'git'`
- [x] 修改 `src/views/changes/flow/run-state.ts`：`normalizeKind` / `stepWord` 加 8 词映射（snake ↔ camel）；`groupOf` 把 commit/rebase/merge/finalize 归 `git`、specSync/resolveConflicts 归 `workerAgent`、preflight/seal 归 `toolStep`
- [x] 修改 `src/views/changes/flow/run-step-node.tsx`：`GROUP_LABEL` 加 `git: 'Git'`
- [x] 修改 `src/views/changes/flow/run-control-panel.tsx`：`ConfirmCard` 检测 `phase === 'archive'` 时渲染归档确认卡片（`activeRun.archive` 完成度警告 / delta specs / merge 目标 + 跳过同步勾选），确认走 `confirm(proceed, syncSpecs)`
- [x] 修改 `src/views/changes/change-detail-view.tsx`：摘 `ArchivePanel` 装配与 `useArchiveFlow` 调用、`archiveOpen` 状态与「归档…」按钮路径（归档入口 = run 的 stop-before-archive 停等面）
- [x] 删除 `src/views/changes/flow/archive-panel.tsx` + `archive-state.ts` + `src/views/changes/hooks/use-archive-flow.ts`（+ 各测试）

## 阶段十一：版本交付与静态自查

- [x] 修改 `packages/desktop/package.json`：`version` 0.4.36 → 0.4.37
- [x] 静态自查：`archive_flow` / `ArchiveControl` / `ArchiveSink` / `ArchiveUpdate` / `ArchiveStage` / `useArchiveFlow` / `applyArchiveUpdate` / `ARCHIVE_STAGES` 零残留引用；`next.evaluator.is_none()` 零残留；`PIPELINE_PHASES` 双侧 10 列同步；`plugins/dev-team` 零改动；`StepKind`（审计七臂）未被扩展
