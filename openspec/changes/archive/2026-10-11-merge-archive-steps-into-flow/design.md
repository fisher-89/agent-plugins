# 设计: merge-archive-steps-into-flow

> **变更**: merge-archive-steps-into-flow
> **日期**: 2026-10
> **基于**: proposal.md、explore.md（八轮收敛）、specs/ 九个能力 delta

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 九份 delta 已由提案阶段定稿，本设计基于其文本展开，只覆盖「变更范围 - 实现文件」并把 proposal 遗留的七个待决问题定稿到类型面 / 状态机面 / 命令面 / 前端面的具体形态。

工作树实况（本变更起点，`packages/desktop` 0.4.36 基线）：`ArchiveVcsPort` 与 `crates/infra/vcs` 的归档子命令族（`commit_all` / `rebase_branch` / `ff_merge` / `commit_paths` / `current_branch` / `branch_merged` / `dirty` / `worktree_snapshot` / `rebase_continue` / `rebase_abort`）已由 archive-merge-first 落地；`RunStepKind` / `ChangeStepKind` / `StepNode` 现为十值全词汇；`phase_table` 现为 8 路由相位 + `code-analyze` 占位列；`RunNodeRecord` / `RunEdgeRecord` 已落库。本设计描述**本变更完成后的目标形态**：归档从「独立六段阶段机」收敛为「walker 第 10 相位（archive）的 10 步 step 序列」，零新表、零独立归档链事件面。

---

## 关键设计决策（proposal 待决问题定稿）

| # | 问题 | 定稿 | 理由 |
|---|------|------|------|
| D1 | `auto_next_phase`「按指定 phase 停止」形态 | `RunRequest.stop_before: Option<String>`（相位名；命令层固定 `Some("archive")`）。**确认门不改语义锚**：现有「相位收口后确认」块保留（`!auto_next_phase` 时逐相位流出），并在 `drive` 循环顶部（`phase_next` 解析出 `next.next_phase` 后、进入该相位 steps 前）追加一条**停止门**——`next.next_phase == stop_before` 时无论 auto 与否都发 `ConfirmWait` 并 `wait_confirm`；现有收口后确认块对 `phase == stop_before` 跳过（archive 收口后不再补一道确认，直接走 `done`） | 手动模式逐相位停等契约逐字不变（proposal 之后 acceptance 之后仍各一道确认）；auto 模式仅在 archive 前停等一次；停止门只在 `phase_next` 供给下一相位处判定，walker 零自持相位序；archive 前恰好一道确认、archive 后零确认 |
| D2 | 归档前停等是 `confirm` 还是 `ask` | **`confirm`（proceed 二选一）+ 停等载荷**。`RunUpdate::ConfirmWait` 扩为 `{ phase, archive: Option<ArchivePreflight> }`；停止门在发 `ConfirmWait` 前经 `ToolCommand::ArchivePreflight` 算一次前置只读载荷随行，`ChangeRunSnapshot` 增 `archive` 字段透出活面。「跳过 sync」经 `change_flow_confirm` 增参 `sync_specs: bool` 回流（`wait_confirm` 返回 `(proceed, sync_specs)`） | 归档前置数据（完成度 / delta specs / merge 目标）需要 vcs 探测（merge 目标）与 `specs/` 目录扫描，`ChangeDetail` 纯读面不含 vcs，故载荷须由停等面随行；`ask` 是决策 agent 专用中断通道，复用为归档确认会污染决策语义；confirm 扩展一个 `sync_specs` bool 是最小差异化 |
| D3 | 归档 agent 步 provenance `source_ref` 定式 | 沿用 `source_ref = <change_id>/<phase>/<role>/<attempt>`，**扩 `WorkerRole` 增 `SpecSync` / `ResolveConflicts`**（`as_str` = `"spec_sync"` / `"resolve_conflicts"`）。spec_sync 步 = `<id>/archive/spec_sync/<attempt>`、resolve_conflicts 步 = `<id>/archive/resolve_conflicts/<attempt>` | 同 phase 同 attempt 内唯一（两词互异）；复用 `run_worker` 的 provenance 组装单点；前端抽屉按节点 `session_id` 直查优先、槽位缺席回退 sourceRef 既有反查 |
| D4 | `ResolveConflicts` 的 `StepGroup` | 归 **`Agent`**（`StepNode::role() = AgentRole::ResolveConflicts`）；git 分类徽章只覆盖 commit / rebase / merge / finalize | 冲突解算主体是 agent 会话（vcs 只是 rebase 前置与后验），flow-view spec 已定「SpecSync / ResolveConflicts 为 agent」；前端 git 徽章不含 ResolveConflicts |
| D5 | archive 列位置 | 恒追加为**第 10 列**（`code-analyze` 之后），后端 `detail.rs` 与前端 `layout.ts` 双侧同步 | 列身份纯布局、不参与路由权威；acceptance → archive 的 forward 边跨 `code-analyze` 空列（R6 低风险，接受） |
| D6 | 空 checklist / 空 report PhaseRecord 视觉 | archive 列呈现 **1 个 pass eval 节点 + 10 步节点**；eval 节点 report 空态、checklist 空表（不渲染条目行，抽屉以「（空）」占位）；从简不新增样式特判 | 空 checklist 落账仅为「相位已 pass」标记，非评估证据面（desktop-change-archive V1 边界第 6 条） |
| D7 | 裸 `archive_change` 薄命令 | **保留零改动**（`changes` 组低层双写直调面；Seal 步之外的既有逃生口）；归档编排入口唯一经 run 的 archive 相位，`archive_flow` 五命令退役 | merge-first 前置引导的「绕过 archive 相位直接调归档写面被拒」语义依赖裸命令路径的既有拒绝面；零改动风险最低 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| step 词汇与分组单点 | `StepNode` / `RunStepKind` 十值 → 十八值（+8 归档步词）；`StepGroup` 三分类 → 四分类（+Git）；`AgentRole` +SpecSync/+ResolveConflicts；`group()` / `role()` 派生随动 | `crates/core/workflow/src/state.rs` | model、serde、specta | Rust（中性状态类型） |
| requirement 计划图单源 | `ARCHIVE_STEPS` 10 步序列声明；`REQUIREMENT_TABLE` 追加 archive `PhaseDefinition`（`executor=None` / `evaluator=None`、零 artifact_manifest） | `crates/core/workflow/src/write/phase_table.rs` | state、agent（ModelLevel） | Rust（sync，编译期 const） |
| 相位路由权威 | 路由经 `REQUIREMENT_TABLE` 自然覆盖 archive（8 相位 pass → next=archive、archive 空 checklist 落账后 `all_passed` → done）；`has_phase_passed` / `allowed_backtrack_phases` 零特判 | `crates/core/workflow/src/write/phase_next.rs` | phase_table、state、store port | Rust（sync 只读路由） |
| 详情 10 列投影 | `PIPELINE_PHASES` 9 → 10（追加 archive）；`runs[].nodes` 十八值全词汇投影零过滤 | `crates/core/workflow/src/queries/detail.rs` | state、artifacts、foundation::layout | Rust（sync） |
| 编排步词汇与停等载荷 | `ChangeStepKind` +8；新增 `ArchivePreflight`（前置只读载荷）；`RunUpdate::ConfirmWait` 增 `archive`；`ChangeRunSnapshot` 增 `archive` | `crates/core/orchestration/src/state.rs` | serde、specta | Rust |
| walker 三执行缝 | `StepContext` 增 `vcs: Arc<dyn ArchiveVcsPort>`（第三缝）；git 步（commit / rebase / merge / finalize）在 `run_step` 直调 vcs；`run_step` 增 8 归档步分支；`step_phase_log` 增空 checklist 第三分支；删除 `evaluator.is_none()` 检查；停止门 + 收口语义改写 | `crates/core/orchestration/src/walker.rs` | port、state、control、run_history、verdict、decision、transcript、prompt | Rust、tokio async |
| 归档 agent prompt | spec 同步 / 解冲突两模板自 archive_flow.rs 迁入（零 CLI skill 溯源、零 MCP / workflow.json 依赖；change 名 / 冲突清单运行时插值） | `crates/core/orchestration/src/prompt.rs` | decision、verdict | Rust |
| Preflight / Seal 内联步 | `ToolCommand::ArchivePreflight` / `ArchiveSeal` 两变体；`LocalToolSteps` 增 `vcs` 注入；Preflight 复用搬移的 `completion_of` / `missing_artifacts` / `detect_delta_specs`；Seal 直调 `workflow::write::archive` | `crates/core/orchestration/src/steps.rs` | port、workflow::write、foundation::layout | Rust、tokio async |
| run 控制注册表 | `confirm` 增 `sync_specs` 回流；`wait_confirm` 返回 `(proceed, sync_specs)`；`publish` 落 `ConfirmWait.archive`；`snapshot` 出 `archive` | `crates/core/orchestration/src/control.rs` | state、tokio sync | Rust、Mutex + broadcast + oneshot |
| vcs 执行缝实现 | `ProcessArchiveVcs`（`ArchiveVcsPort` 实现）已齐备，零改动复用 | `crates/infra/vcs/src/lib.rs` + `git.rs` | orchestration::port、workflow::write | Rust（sync，spawn 驻此） |
| 命令面收编 | `archive_flow` 五命令退役；`change_flow_start` 装配 vcs + `main_root` + `stop_before`；`change_flow_confirm` 增 `sync_specs`；`archive_change` 保留 | `src/commands/change_flow/mod.rs` + `changes/mod.rs` + `mod.rs` + `main.rs` | orchestration、workflow、vcs_runtime、tauri | Rust、tauri IPC |
| 前端 10 列布局 | `PIPELINE_PHASES` 10 列 | `src/views/changes/flow/layout.ts` | types | TypeScript |
| 前端步词汇与分组 | `RunStepGroup` + `git`；`groupOf` 归组；`normalizeKind` / `stepWord` 加 8 词；`GROUP_LABEL` 加 git | `src/views/changes/flow/run-state.ts` + `types.ts` + `run-step-node.tsx` | dto 类型 | TypeScript |
| 前端停等面 | archive 确认卡片（`phase === 'archive'` 时呈现 `activeRun.archive` 载荷 + 跳过同步勾选）；六段清单退役 | `src/views/changes/flow/run-control-panel.tsx` + `change-detail-view.tsx` | dto、hooks | TypeScript、React |

---

## 变更清单

### 新增文件

<!-- 不涉及：本变更零新增文件。归档提示 / 前置只读载荷 / 两 agent prompt 均落既有文件（state.rs / steps.rs / prompt.rs）。 -->

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `crates/core/workflow/src/state.rs` | `StepNode` +8（`Preflight` / `Commit` / `Rebase` / `ResolveConflicts` / `Merge` / `SpecSync` / `Seal` / `Finalize`，snake_case 词）；`RunStepKind` +8（同词面）；`StepGroup` +`Git`；`AgentRole` +`SpecSync` / `ResolveConflicts`；`StepNode::as_str()` / `group()` / `role()` 随动（commit/rebase/merge/finalize→Git、preflight/seal→Exec、spec_sync/resolve_conflicts→Agent，role 分别为 `SpecSync` / `ResolveConflicts`） | 计划图 + 执行图 + 分组词汇单点 |
| `crates/core/workflow/src/write/phase_table.rs` | 新增 `ARCHIVE_STEPS` const（PhaseStart → Preflight → Commit → Rebase → ResolveConflicts → Merge → SpecSync → Seal → Finalize → PhaseLog，`prompt_ref` 恒 `None`、`on_fail` 恒 `None`——归档步失败即 Terminal）；`REQUIREMENT_TABLE` 追加 archive `PhaseDefinition`（`executor=None` / `evaluator=None`、`artifact_manifest=&[]`） | 计划图单源；archive 为确定性相位第二例 |
| `crates/core/workflow/src/write/phase_next.rs` | 无逻辑改动（路由经 `REQUIREMENT_TABLE` 自动覆盖：8 相位 pass → next=archive、archive 空 checklist 落账后 `all_passed` → done）；确认 `has_phase_passed` / `allowed_backtrack_phases` 对 archive 零特判 | 路由权威不变；archive 为末相位，回溯白名单仅前向 |
| `crates/core/workflow/src/queries/detail.rs` | `PIPELINE_PHASES` `[&str; 9]` → `[&str; 10]`（追加 `"archive"`）；注释同步 | 详情 10 列；runs nodes 十八值投影随 `RunStepKind` 自动 |
| `crates/core/orchestration/src/state.rs` | `ChangeStepKind` +8（snake_case 词，`as_str` 随动）；新增 `ArchivePreflight`（`{ name, completed, incomplete_phases, missing_artifacts, delta_specs, worktree, branch, merge_target }`，serde + specta）；`RunUpdate::ConfirmWait` 变体改 `{ phase, archive: Option<ArchivePreflight> }`；`ChangeRunSnapshot` 增 `archive: Option<ArchivePreflight>` | 编排步词汇 + 停等载荷 |
| `crates/core/orchestration/src/port.rs` | `WorkerRole` +`SpecSync` / `ResolveConflicts`（`as_str` 随动）；`ToolCommand` +`ArchivePreflight { change_id }` / `ArchiveSeal { change_id }`；`ToolStepOutput` +`ArchivePreflight(ArchivePreflight)` / `ArchiveSeal(workflow::write::ArchiveOutcome)`；`ArchiveVcsPort` 零改动（已齐备） | walker 第三缝 + 归档 agent 角色 + 内联步命令面 |
| `crates/core/orchestration/src/prompt.rs` | 新增 `spec_sync_prompt(change)` / `merge_conflict_prompt(change, conflicts)`（自 archive_flow.rs 搬移，文本零改动） | 归档 agent prompt 单点 |
| `crates/core/orchestration/src/steps.rs` | `LocalToolSteps::new` 增 `vcs: Arc<dyn ArchiveVcsPort>`；`execute` 增 `ToolCommand::ArchivePreflight`（搬移的 `completion_of` / `missing_artifacts` / `detect_delta_specs` + `vcs.current_branch` / `branch_merged` → `ArchivePreflight`）与 `ToolCommand::ArchiveSeal`（`workflow::write::archive`）两分支；两分支不写 `StepRecord` 审计行（归档步为执行图节点，非相位机步骤） | Preflight / Seal 内联步落此 |
| `crates/core/orchestration/src/control.rs` | `confirm` 增 `sync_specs: bool`（oneshot 载荷改 `(bool, bool)`）；`RunGuard::wait_confirm` 返回 `(bool, bool)`；`publish` 的 `ConfirmWait` 分支落 `entry.archive`；`RunEntry` 增 `archive: Option<ArchivePreflight>`；`snapshot` 出 `archive` | 停等载荷与 sync 选择回流 |
| `crates/core/orchestration/src/walker.rs` | `RunRequest` +`main_root: String` +`stop_before: Option<String>`；`walk_run` +`vcs: Arc<dyn ArchiveVcsPort>`；`drive` 删除 `next.evaluator.is_none()` 检查、增循环顶部停止门（`next.next_phase == stop_before` → `ConfirmWait{archive}` + `wait_confirm` 取 `sync_specs`）、收口后确认块跳过 `phase == stop_before`；`StepContext` +`vcs`；`PhaseCursor` +`preflight` / `rebase_conflicts` / `sync_specs` / `seal_outcome`；`run_step` 增 8 归档步分支；`run_worker` 增 `root` 参数（cwd 覆盖）并映射 `WorkerRole::SpecSync` / `ResolveConflicts` → `ChangeStepKind`；`step_phase_log` 增空 checklist 第三分支（test_outcome 分支之后、verdict 分支之前）；`ALL_PHASES_PASSED` 文案改写为归档收口语义；`TryFrom<ToolStepOutput>` 增 ArchivePreflight / ArchiveSeal 两 impl | 归档相位执行主体；零 `if phase == archive` 特判 |
| `crates/core/orchestration/src/archive_flow.rs` | **退役（删除）**：`run_archive_flow` / `preflight` 与 Preflight / Commit / Merge / SpecSync / Seal / Finalize 执行体并入 walker（`run_step` 8 分支）与 vcs 缝；`ArchiveControl` / `ArchiveGuard` 删除 | 归档链主执行体退役 |
| `crates/core/orchestration/src/lib.rs` | 摘 `pub mod archive_flow` 与全部 archive 再导出；挂 `ArchivePreflight` 再导出；摘 `archive_flow_test` 测试模块声明 | 出口面随退役 |
| `src/commands/archive_flow/mod.rs` + `mod_test.rs` | **退役**（删除） | 归档五命令收编进 change_flow |
| `src/commands/change_flow/mod.rs` | 摘 `ArchiveControl` 反向互斥面（归档链已并入 run）；`change_flow_start` 装配 `vcs: Arc<dyn ArchiveVcsPort>`（`ProcessArchiveVcs`）、`main_root: root.clone()`、`stop_before: Some("archive")`，注入 `LocalToolSteps` 与 `walk_run`；`change_flow_confirm` 增 `sync_specs: bool` 并透传 `control.confirm` | 命令面收编 + vcs 装配 |
| `src/commands/changes/mod.rs` | `ActiveRunView` 增 `archive: Option<orchestration::ArchivePreflight>`（自快照投影）；`archive_change` 裸命令零改动保留 | 统一视图出停等载荷 |
| `src/commands/mod.rs` | 摘 `pub mod archive_flow` 与 5 条 archive_flow 命令注册 | 命令面登记收编 |
| `src/main.rs` | 摘 `app.manage(Arc::new(ArchiveControl::new()))` | 归档链注册表退役 |
| `src/views/changes/flow/layout.ts` | `PIPELINE_PHASES` 追加 `"archive"` | 前端 10 列镜像 |
| `src/views/changes/flow/types.ts` | `RunStepGroup` +`'git'` | 前端分组词汇 |
| `src/views/changes/flow/run-state.ts` | `normalizeKind` / `stepWord` 加 8 词映射；`groupOf` 把 commit/rebase/merge/finalize 归 `git`、specSync/resolveConflicts 归 `workerAgent`、preflight/seal 归 `toolStep` | 步词汇与分组 |
| `src/views/changes/flow/run-step-node.tsx` | `GROUP_LABEL` 加 `git: 'Git'` | 分组徽章 |
| `src/views/changes/flow/run-control-panel.tsx` | `ConfirmCard` 检测 `phase === 'archive'` 时渲染归档确认卡片（`activeRun.archive` 完成度警告 / delta specs / merge 目标 + 跳过同步勾选），确认走 `confirm(proceed, syncSpecs)` | 停等面承载归档确认对话 |
| `src/views/changes/flow/archive-panel.tsx` + `archive-state.ts` + `hooks/use-archive-flow.ts`（+ 各测试） | **退役**（删除） | 六段清单随归档并入流程图退役 |
| `src/views/changes/change-detail-view.tsx` | 摘 `ArchivePanel` 装配与 `useArchiveFlow` 调用；`archiveOpen` 状态与归档按钮路径移除（归档入口 = run 流程的 stop-before-archive 停等面） | 归档确认面对话由停等面承载 |
| `src/types/generated/bindings.ts` | 随 DTO / 命令面演进再生（`ArchivePreflight` / `ConfirmWait.archive` / `ActiveRunView.archive` / `changeFlowConfirm.syncSpecs` / 退役的 archive_flow 五命令 / 十八值词汇） | 非手改，bindings:check 守卫 |
| `packages/desktop/package.json` | `version` 0.4.36 → 0.4.37 | 版本交付 |

> **零改动沿用**：`crates/infra/vcs/src/git.rs` 在 proposal 实现文件清单内，但本变更零 delta——`commit_all` / `rebase_branch` / `ff_merge` / `commit_paths` / `current_branch` / `branch_merged` / `dirty` / `worktree_snapshot` / `rebase_continue` / `rebase_abort` 归档子命令族已由 archive-merge-first 落地（`git.rs` 内为 `pub(crate)` 成员，`lib.rs` 中 `impl ArchiveVcsPort for ProcessArchiveVcs` 已接线），故不入修改文件表、对应零任务。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `phase_table` | `crates/core/workflow/src/write/phase_table.rs` | 修改 | `pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]>` | `REQUIREMENT_TABLE` 9 相位（+archive）；`ARCHIVE_STEPS` 10 步 |
| `phase_next` | `crates/core/workflow/src/write/phase_next.rs` | 不变（行为随表） | `pub fn phase_next(store: &dyn ChangeStateStore, change_id: &str, run_id: &str, anchors: &SessionAnchors) -> Result<PhaseNextOutcome, String>` | 路由覆盖 archive → done |
| `change_detail` | `crates/core/workflow/src/queries/detail.rs` | 修改 | `pub fn change_detail(layout: &Layout, store: &dyn ChangeStateStore, id: &str) -> Option<ChangeDetail>` | `pipeline` 10 列；nodes 十八值 |
| `new_run_id` | `crates/core/orchestration/src/walker.rs` | 不变 | `pub fn new_run_id() -> String` | 铸造 `run-<millis>` |
| `walk_run` | `crates/core/orchestration/src/walker.rs` | 修改 | `pub async fn walk_run(worker: Arc<dyn WorkerAgentPort>, tools: Arc<dyn ToolStepPort>, vcs: Arc<dyn ArchiveVcsPort>, snapshot: Arc<dyn WorkflowSnapshotPort>, history: Arc<dyn RunHistoryPort>, control: Arc<ChangeFlowControl>, guard: RunGuard, request: RunRequest) -> ChangeRunStatus` | 增 vcs 第三缝；archive 相位执行 |
| `run_archive_flow` | `crates/core/orchestration/src/archive_flow.rs` | 删除（退役） | — | 归档链主入口退役，执行体并入 walker |
| `preflight` | `crates/core/orchestration/src/archive_flow.rs` | 删除（搬移） | — | 前置只读载荷搬入 `steps.rs`（`ToolCommand::ArchivePreflight`） |
| `spec_sync_prompt` | `crates/core/orchestration/src/prompt.rs` | 修改（搬移） | `pub(crate) fn spec_sync_prompt(change: &str) -> String` | spec 同步 prompt 单点 |
| `merge_conflict_prompt` | `crates/core/orchestration/src/prompt.rs` | 修改（搬移） | `pub(crate) fn merge_conflict_prompt(change: &str, conflicts: &[String]) -> String` | 解冲突 prompt 单点 |
| `LocalToolSteps::new` | `crates/core/orchestration/src/steps.rs` | 修改 | `pub fn new(anchors: Arc<SessionAnchors>, static_check: Arc<dyn StaticCheckRunner>, test_execution: Arc<dyn TestExecutionRunner>, vcs: Arc<dyn ArchiveVcsPort>, store: Arc<dyn ChangeStateStore>, run_id: String) -> Self` | 增 vcs 注入（Preflight 探测） |
| `run_archive_preflight` | `crates/core/orchestration/src/steps.rs` | 新增 | `pub(crate) fn run_archive_preflight(main_root: &Path, store: &dyn ChangeStateStore, id: &str, vcs: &dyn ArchiveVcsPort) -> Option<ArchivePreflight>` | 前置只读载荷单点 |
| `change_flow_start`（IPC） | `src/commands/change_flow/mod.rs` | 修改 | `#[tauri::command] pub async fn change_flow_start(app: AppHandle, on_event: Channel<RunNotice>, root: String, id: String, auto_next_phase: bool) -> Result<ChangeRunSummary, String>` | 内部装配 vcs + `main_root` + `stop_before=Some("archive")`；签名不变 |
| `change_flow_confirm`（IPC） | `src/commands/change_flow/mod.rs` | 修改 | `#[tauri::command] pub fn change_flow_confirm(app: AppHandle, root: String, id: String, proceed: bool, sync_specs: bool) -> Result<(), String>` | 增 `sync_specs`（归档前停等回流） |
| `change_flow_stop` / `change_flow_answer` / `change_flow_watch`（IPC） | `src/commands/change_flow/mod.rs` | 不变 | 见源码 | 请求-应答语义不变 |
| `archive_flow_preflight` / `archive_flow_start` / `archive_flow_stop` / `archive_flow_state` / `archive_flow_watch`（IPC） | `src/commands/archive_flow/mod.rs` | 删除（退役） | — | 归档五命令收编进 change_flow |
| `archive_change`（IPC） | `src/commands/changes/mod.rs` | 不变 | `#[tauri::command] pub fn archive_change(app: AppHandle, root: String, id: String) -> Result<ArchiveOutcome, String>` | 裸双写薄命令保留零改动 |
| `get_change_detail`（IPC） | `src/commands/changes/mod.rs` | 修改 | `#[tauri::command] pub fn get_change_detail(stores: State<'_, WorkspaceStores>, control: State<'_, Arc<ChangeFlowControl>>, root: String, id: String) -> Option<ChangeDetailUnified>` | `ActiveRunView` 增 `archive` |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `StepNode` | `crates/core/workflow/src/state.rs` | 修改 | 十值 → 十八值（+`Preflight` / `Commit` / `Rebase` / `ResolveConflicts` / `Merge` / `SpecSync` / `Seal` / `Finalize`） |
| `RunStepKind` | `crates/core/workflow/src/state.rs` | 修改 | 十值 → 十八值（同词面） |
| `StepGroup` | `crates/core/workflow/src/state.rs` | 修改 | 三分类 → 四分类（+`Git`） |
| `AgentRole` | `crates/core/workflow/src/state.rs` | 修改 | 三值 → 五值（+`SpecSync` / `ResolveConflicts`） |
| `ChangeStepKind` | `crates/core/orchestration/src/state.rs` | 修改 | 十值 → 十八值（snake_case 词） |
| `ArchivePreflight` | `crates/core/orchestration/src/state.rs` | 新增 | `{ name, completed, incomplete_phases, missing_artifacts, delta_specs, worktree, branch, merge_target }`（自 archive_flow 同名类型搬移，摘 `run_active`） |
| `RunUpdate::ConfirmWait` | `crates/core/orchestration/src/state.rs` | 修改 | `{ phase: String, archive: Option<ArchivePreflight> }` |
| `ChangeRunSnapshot` | `crates/core/orchestration/src/state.rs` | 修改 | 增 `archive: Option<ArchivePreflight>` |
| `WorkerRole` | `crates/core/orchestration/src/port.rs` | 修改 | 三值 → 五值（+`SpecSync` / `ResolveConflicts`） |
| `ToolCommand` | `crates/core/orchestration/src/port.rs` | 修改 | 增 `ArchivePreflight { change_id }` / `ArchiveSeal { change_id }` |
| `ToolStepOutput` | `crates/core/orchestration/src/port.rs` | 修改 | 增 `ArchivePreflight(ArchivePreflight)` / `ArchiveSeal(ArchiveOutcome)` |
| `RunRequest` | `crates/core/orchestration/src/walker.rs` | 修改 | 增 `main_root: String` + `stop_before: Option<String>` |
| `ActiveRunView` | `src/commands/changes/mod.rs` | 修改 | 增 `archive: Option<ArchivePreflight>` |
| `ArchiveStage` / `ArchiveStageStatus` / `ArchiveStageState` / `ArchiveSummary` / `ArchiveSpecsStatus` / `ArchiveUpdate` / `ArchiveSnapshot` / `ArchiveControl` / `ArchiveGuard` / `ArchiveRequest` | `crates/core/orchestration/src/archive_flow.rs` | 删除（退役） | 归档链六段阶段机类型整体退役 |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.4.37` | 0.4.36 → 0.4.37（`tauri.conf.json` 经 `../package.json` 自动跟随；`src-tauri/Cargo.toml` 版本不随动） |

---

## 数据模型

### 计划图（源码，编译期静态）

`REQUIREMENT_TABLE` 追加第 9 个路由相位 `archive`（`PhaseDefinition { id: "archive", steps: ARCHIVE_STEPS, artifact_manifest: &[], executor: None, evaluator: None }`）。`ARCHIVE_STEPS` 声明 10 步：

```
PhaseStart      exec（复用；attempt max+1 分配）
Preflight       exec（只读校验 + vcs 探测）
Commit          git（vcs.commit_all；干净跳过）
Rebase          git（vcs.rebase_branch → PhaseCursor.rebase_conflicts）
ResolveConflicts agent（条件：无冲突 passed+detail；有冲突解冲突 agent）
Merge           git（vcs.ff_merge）
SpecSync        agent（条件：delta specs 在场且未跳过；否则 passed+detail）
Seal            exec（workflow::write::archive 双写）
Finalize        git（vcs.commit_paths；已落盘跳过）
PhaseLog        exec（空 checklist 落 verdict=pass）
```

每步 `on_fail = None`（步失败即 `Terminal`，不唤决策、不进相位重试）。`prompt_ref` 恒 `None`（spec_sync / resolve_conflicts 的 prompt 由 `orchestration::prompt` 运行时组装——workflow 不可依赖 orchestration）。

### 执行图与评估行（db / 进程内，字段与持久化）

本变更零新表——归档节点 / 边 / PhaseRecord 复用既有三表；停等载荷与跨步产物为进程内态。字段、关系与持久化归属如下：

| 模型 | 字段 | 类型 | 持久化 | 关系 / 说明 |
|------|------|------|--------|-------------|
| 执行图节点 `RunNodeRecord` | `node_key` / `run_id` / `change_id` | `u128`（打包键）/ `String` / `String` | native_db `RunNodeRecord`（复合主键 `node_key`，二级索引 `run_id`） | 归档 10 步节点复用本表（零新表）；`node_key = (hash64(run_id) << 64) \| node_seq` |
| | `phase` / `attempt` / `step` / `instance_no` | `String` / `u32` / `RunStepKind`（十八值）/ `u32` | 同上 | 稳定身份键 `(phase, attempt, step, instance_no)`；`step` 全词汇十八值（+8 归档步词）；attempt 由 PhaseStart max+1 分配 |
| | `status` / `session_id` / `detail` / `output_ref` / `timestamp` | `RunStepStatus` / `Option<String>` / `Option<String>` / `Option<String>` / `i64` | 同上 | `session_id` 仅 SpecSync / ResolveConflicts 在案，其余 None；`detail` 携跳过因（干净 / 已合入 / 无 delta / 用户选择 / 已落盘） |
| 执行图边 `RunEdgeRecord` | `edge_key` / `run_id` / `change_id` / `from_node_key` / `to_node_key` / `edge_kind` / `timestamp` | `u128` / `String` / `String` / `String` / `String` / `RunEdgeKind` / `i64` | native_db `RunEdgeRecord`（复合主键 `edge_key`，二级索引 `run_id` / `from_node_key`） | 归档串行推进全为 `Forward`；零 feedback / retry / backtrack |
| 相位评估行 `PhaseRecord` | `phase` / `attempt` / `verdict` / `report` / `skipped` / `executor_session_id` / `evaluator_session_id` / `decision_session_id` / `timestamp` | `String` / `u32` / `Verdict` / `String` / `bool` / `Option<String>`×3 / `i64` | native_db `PhaseRecord`（主键 `id`，二级索引 `change_id`） | archive 末尾以 `report=""` + 空 checklist 落 `verdict=pass`；`derive_verdict([])=Pass` |
| | checklist 子行 | `Vec<ChecklistItem>` | native_db `ChecklistItemRecord`（子表，`(phase_id, item_index)` 复合键） | archive 落账零 checklist 行（空载荷） |
| 前置只读载荷 `ArchivePreflight` | `name` / `completed` / `incomplete_phases` / `missing_artifacts` / `delta_specs` / `worktree` / `branch` / `merge_target` | `String` / `bool` / `Vec<String>` / `Vec<String>` / `Vec<String>` / `Option<String>` / `Option<String>` / `Option<String>` | 进程内（`ChangeRunSnapshot.archive` 广播，不落库） | 停等面随行载荷 + Preflight 步重算入 `PhaseCursor.preflight` |

### PhaseCursor 跨步产物（进程内，生产 → 消费）

| 字段 | 类型 | 持久化 | 生产者 → 消费者 |
|------|------|--------|----------------|
| `preflight` | `Option<ArchivePreflight>` | 进程内（run 期间；终态后蒸发） | Preflight 步 → Commit / Rebase / ResolveConflicts / Merge / SpecSync / Finalize |
| `rebase_conflicts` | `Option<Vec<String>>` | 进程内（同上） | Rebase 步 → ResolveConflicts 步 |
| `sync_specs` | `bool` | 进程内（同上） | 停止门 confirm 回流 → SpecSync 步 |
| `seal_outcome` | `Option<ArchiveOutcome>` | 进程内（同上） | Seal 步 → Finalize 步（`archived_date` 派生归档目录路径） |

### 状态机（run 收口语义）

- `phase_next` 的 `done` 判定 = `REQUIREMENT_TABLE.iter().all(has_phase_passed)`——archive 空 checklist 落账（`verdict=pass`）后 `all_passed` 才真，run 才 `Completed`。
- archive 相位步失败 = `Terminal` → run `Failed` 直接收口（零决策分叉、零 phase retry）。
- 停止：`change_flow_stop` 复用（停止 run 自然停 archive 相位）。

---

## 路由 / API 设计

| method | path | 说明 | 输入 | 输出 | auth |
|--------|------|------|------|------|------|
| IPC `change_flow_start` | — | 发起 run（归档为固定 stop-before） | `root`, `id`, `auto_next_phase` | `ChangeRunSummary { run_id, status }` | 桌面本地 |
| IPC `change_flow_stop` | — | 停止 run（含 archive 相位进行中） | `root`, `id` | `()`（幂等） | 桌面本地 |
| IPC `change_flow_answer` | — | ask 应答 | `root`, `id`, `answer` | `()` | 桌面本地 |
| IPC `change_flow_confirm` | — | 相位间 / archive 前停等确认 | `root`, `id`, `proceed`, `sync_specs` | `()` | 桌面本地 |
| IPC `change_flow_watch` | — | 运行中重挂补订 | `root`, `id`, `on_event` | `()` | 桌面本地 |
| IPC `get_change_detail` | — | 统一视图（库读史 ∪ 在飞 run 活面，含 archive 停等载荷） | `root`, `id` | `ChangeDetailUnified` | 桌面本地 |
| IPC `archive_change` | — | 裸双写直调面（保留） | `root`, `id` | `ArchiveOutcome` | 桌面本地 |
| （退役）IPC `archive_flow_preflight` / `archive_flow_start` / `archive_flow_stop` / `archive_flow_state` / `archive_flow_watch` | — | 归档五命令退役 | — | — | — |

---

## 依赖

### 运行时依赖

- `orchestration → workflow`（不变）：walker 经 `ToolStepPort` 直调写面 `phase_next` / `phase_start` / `phase_log` / `archive`。
- `orchestration → foundation::layout`（已有）：`steps.rs` 经 `root` 派生 `Layout` 供 Preflight 目录扫描与 Seal 双写。
- `orchestration` 新增消费 `ArchiveVcsPort`（port 定义已在 orchestration；实现驻 `infra/vcs`，spawn 不进 core）。
- `desktop-app → vcs-runtime`（已有）：命令层装配 `ProcessArchiveVcs` 注入 walker 与 `LocalToolSteps`。

### 构建 / 测试依赖

- 零新增 crate / 依赖；`bindings:export` / `bindings:check` / tsc / cargo 既有链路承载再生与一致性守卫。

---

## 待决问题

- 无未决事项。proposal 七个待决问题已在「关键设计决策」D1–D7 全部定稿。

---

## 数据流（归档相位一次 run 收口）

```
phase_next(8 相位 pass) → next=archive
drive 顶部停止门：phase == stop_before("archive")
  → 算 ArchivePreflight（ToolCommand::ArchivePreflight，经 steps.rs 的 store+vcs+layout）
  → emit ConfirmWait { phase:"archive", archive: Some(preflight) }
  → wait_confirm() → (proceed, sync_specs)
  → proceed=false/取消 → stopped；proceed=true → 进 archive 相位
PhaseStart（attempt 分配）→ Preflight（重算载荷入 cursor）→
Commit（干净跳过 | commit_all）→ Rebase（rebase_conflicts 入 cursor）→
ResolveConflicts（无冲突 passed | 解冲突 agent + rebase_continue + 后验）→
Merge（ff_merge）→ SpecSync（无 delta/用户跳过 passed | agent 会话）→
Seal（write::archive 双写）→ Finalize（已落盘跳过 | commit_paths）→
PhaseLog（空 checklist 落 verdict=pass）→ phase_next → done → Completed
```
