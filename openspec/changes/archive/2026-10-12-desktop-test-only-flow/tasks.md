# 任务: desktop-test-only-flow

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖实现文件。顺序按依赖排列：相位表两值分派 + code-analyze 步序列与 prompt → 写面严格分派文案 + create/promote 入参 → 详情聚合列序出线 → 发起前置校验文案 → ExploreRecord v3 + store 回填操作面 → IPC 命令面 → bindings 再生 → 前端 explore 侧（stance / 解析 / 动作面 / 详情选择器）→ 前端新建类型选择器 → 前端流程图层列序出线 → 版本交付。
>
> 依赖序要点：`detail.rs`（阶段三）消费 `phase_table` 两值分派与 `PIPELINE_PHASES` 删除（阶段一）；`create.rs` / `promote.rs`（阶段二）消费 `phase_table` 做合法性校验（阶段一）；`update_explore_workflow_type` 命令（阶段六）消费 store `set_explore_workflow_type`（阶段五）；`create_change` / `promote_explore` 命令（阶段六）消费写面新签名（阶段二）；bindings 再生（阶段七）依赖命令面（阶段六）并先于前端（阶段八 / 九读 bindings 新签名）；前端 `attachments.ts` / `run-state.ts` / `graph.ts`（阶段十）消费 `PIPELINE_PHASES` 删除（阶段一并联前端 `layout.ts`，但接线在阶段十收敛）；`change-detail-view.tsx`（阶段十）消费 `runStepNodes` 新签名。

## 阶段一：相位表两值分派 + code-analyze 步序列与 prompt

- [x] 新增 `crates/core/workflow/src/prompts/code-analyze-planner.md`（全静态零占位角色知识 md）
- [x] 新增 `crates/core/workflow/src/prompts/code-analyze-evaluator.md`（全静态零占位角色知识 md）
- [x] 修改 `crates/core/workflow/src/write/phase_table.rs`：新增 `CODE_ANALYZE_EXECUTOR_PROMPT` / `CODE_ANALYZE_EVALUATOR_PROMPT` 两个 `include_str!("../prompts/code-analyze-*.md")` 常量
- [x] 修改 `crates/core/workflow/src/write/phase_table.rs`：新增 `CODE_ANALYZE_STEPS`（`[PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]`，Executor `prompt_ref=Some(CODE_ANALYZE_EXECUTOR_PROMPT)`、Evaluator `prompt_ref=Some(CODE_ANALYZE_EVALUATOR_PROMPT)`、VerdictGate `on_fail=Some(OnFail::Terminal)`）
- [x] 修改 `crates/core/workflow/src/write/phase_table.rs`：`PhaseDefinition` 加 `pub prerequisites: &'static [&'static str]` 字段；`REQUIREMENT_TABLE` 每相位补 `prerequisites`（proposal=[] / dev-design=[proposal] / test-design=[proposal,dev-design] / implement=[dev-design] / test-gen=[test-design,implement] / test-execution=[test-gen,implement] / code-review=[test-gen,implement] / acceptance=[proposal,dev-design,implement] / archive=[]）；删独立 `fn prerequisites()` match；`dependents` 改 `candidate.prerequisites.contains(&phase)`
- [x] 修改 `crates/core/workflow/src/write/phase_table.rs`：新增 `TEST_ONLY_WORKFLOW_TYPE: &str = "test-only"` 常量与 `TEST_ONLY_TABLE`（6 相位：proposal（description=「测试向提案」，复用 `PROPOSAL_STEPS` / `PROPOSAL_MANIFEST` / proposal prompts）、code-analyze（`CODE_ANALYZE_STEPS`、`artifact_manifest=&[]`、executor/evaluator 均 `Some` + `ModelLevel::High`）、test-design / test-gen / test-execution（复用同名 static）、archive（复用 `ARCHIVE_STEPS`、executor/evaluator 均 `None`））；`phase_table` 改两值分派（trim + eq_ignore_ascii_case 命中 `requirement` / `test-only`，其余 `None`）

## 阶段二：写面严格类型分派文案 + create / promote 入参

- [x] 修改 `crates/core/workflow/src/write/create.rs`：加 `use crate::write::phase_table::phase_table;`；`create` 签名加 `workflow_type: &str`（`title` 之后）；前置②校验加 `let workflow_type = workflow_type.trim().to_ascii_lowercase(); if phase_table(&workflow_type).is_none() { return Err(format!("workflow_type \"{workflow_type}\" 不受支持（支持 requirement / test-only）")) }`；`ChangeStateRecord.workflow_type` 由 `"requirement".to_owned()` 改为 `workflow_type.clone()`
- [x] 修改 `crates/core/workflow/src/write/promote.rs`：`promote_explore` 签名加 `workflow_type: &str`；`super::create(...)` 调用加 `workflow_type` 透传
- [x] 修改 `crates/core/workflow/src/write/phase_next.rs`：`phase_table` `None` 文案改为 `workflow_type "{wf}" 不受支持（支持 requirement / test-only 工作流）`
- [x] 修改 `crates/core/workflow/src/write/phase_start.rs`：同上文案
- [x] 修改 `crates/core/workflow/src/write/phase_log.rs`：同上文案
- [x] 修改 `crates/core/workflow/src/write/backtrack.rs`：同上文案

## 阶段三：详情聚合列序出线

- [x] 修改 `crates/core/workflow/src/queries/detail.rs`：删 `pub const PIPELINE_PHASES` 常量；`change_detail` 在 `let record = store.get_change(id).ok().flatten()?;` 后立即 `let table = phase_table(&record.workflow_type)?;`（`None` → 返回 `None`，读面不 fallback requirement）
- [x] 修改 `crates/core/workflow/src/queries/detail.rs`：pipeline 建站改 `table.iter().map(|def| PhaseEntry { phase: def.id.to_string(), attempts: Vec::new() }).collect()`；产物清单 `let manifest = table;`（去 `unwrap_or_default`）；attempt 归位 / 排序逻辑不变

## 阶段四：发起前置校验文案

- [x] 修改 `src/commands/change_flow/mod.rs`：前置校验 2 文案改为 `仅支持 requirement / test-only 工作流（change "{}" 的 workflow_type 为 "{}"）`；注释同步（test-only 进表后自然放行，此门只剩未知类型拒绝面；walker 零 workflow_type 分支）

## 阶段五：ExploreRecord v3 + store 回填操作面

- [x] 修改 `crates/infra/store/src/model.rs`：`ExploreRecord` 升 `#[native_model(id = 4, version = 3, from = ExploreRecordV2)]` 并加 `pub workflow_type: String` 字段（文档注释随动）
- [x] 修改 `crates/infra/store/src/model.rs`：新增 `ExploreRecordV2` 历史形态（`#[native_model(id = 4, version = 2, from = ExploreRecordV1)]`，字段 = 现 v2 七字段：id/root/name/title/promoted_to/created_at/updated_at）；`impl From<ExploreRecordV1> for ExploreRecordV2`（原 v1→v2 升级语义平移：title=name、promoted_to=None）；`impl From<ExploreRecordV2> for ExploreRecordV1`（降级：丢 title/promoted_to）；`impl From<ExploreRecordV2> for ExploreRecord`（升级：workflow_type=`"requirement"`、其余直透）；`impl From<ExploreRecord> for ExploreRecordV2`（降级：丢 workflow_type）；删原 `From<ExploreRecordV1> for ExploreRecord` / `From<ExploreRecord> for ExploreRecordV1`（被 v2 中转替代）
- [x] 修改 `crates/infra/store/src/model.rs`：`ExploreRecord::new` 加 `workflow_type: "requirement".to_owned()`（文档注释随动）
- [x] 修改 `crates/infra/store/src/store.rs`：新增私有 `fn is_valid_workflow_type(value: &str) -> bool`（trim + case-insensitive 命中 `{requirement, test-only}`，自持不依赖 workflow）
- [x] 修改 `crates/infra/store/src/store.rs`：新增 `pub fn set_explore_workflow_type(&self, root: &str, name: &str, workflow_type: &str) -> Result<ExploreRecord, StoreError>`（`is_valid_workflow_type` 不命中 → `Err`；`find_explore_record` miss → `Err`；in-place 写 `workflow_type = canonical 小写` + 刷新 `updated_at` → `upsert` + commit，抄 `set_explore_title` 事务形态）

## 阶段六：IPC 命令面

- [x] 修改 `src/commands/changes/mod.rs`：`create_change` 加 `workflow_type: String` 入参（四参）；`let title = name.clone();` 后调 `create_change_with(app, root, name, goal, title, workflow_type)`；`create_change_with` 加 `workflow_type: String` 入参并在 `spawn_blocking` 内 `write::create(..., &title, &workflow_type)`
- [x] 修改 `src/commands/explores/mod.rs`：`use workflow::write::{self, phase_table, PromoteOutcome};`；`promote_explore` 加 `workflow_type: String` 入参（三参）→ `promote_explore_with(app, root, name, workflow_type)`；`promote_explore_with` 加 `workflow_type` 入参、校验 `phase_table(&workflow_type).is_none()` → `Err(format!("workflow_type \"{workflow_type}\" 不受支持（支持 requirement / test-only）"))`、`write::promote_explore(..., &title, &workflow_type)`
- [x] 修改 `src/commands/explores/mod.rs`：新增 `#[tauri::command] #[specta::specta] pub fn update_explore_workflow_type(stores: State<'_, WorkspaceStores>, root: String, name: String, workflow_type: String) -> Result<ExploreRecord, String>`（blank root → `Err`；`phase_table(&workflow_type).is_none()` → `Err`；`stores.for_root(&root)?.set_explore_workflow_type(&root, &name, &workflow_type).map_err(|e| e.to_string())`）
- [x] 修改 `src/commands/mod.rs`：`all_commands!` 增 `$crate::commands::explores::update_explore_workflow_type`

## 阶段七：bindings 再生

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生 `src/types/generated/bindings.ts`（`createChange` 四参、`promoteExplore` 三参、新增 `updateExploreWorkflowType`、`ExploreRecord.workflowType` 字段出线；非手改）——先于阶段八 / 九前端读 bindings 新签名

## 阶段八：前端 explore 侧（stance / 解析 / 动作面 / 详情选择器）

- [x] 修改 `src/lib/explore-stance.ts`：`stancePreamble` 的「笔记落盘」段在首行标题约定后增「第二行 `workflow_type: <requirement|test-only>`」约定与判断标准（目标是补测试 / 生成测试用例 → test-only，否则 requirement）；注释与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链随动
- [x] 新增 `src/lib/explore-workflow-type.ts`：`extractWorkflowType(content): "requirement" | "test-only" | null`——逐行扫描首个 `^workflow_type:\s*(\S+)\s*$`，值 trim 后 case-insensitive 命中 `{requirement, test-only}` 返回小写规范值，否则 `null`
- [x] 修改 `src/views/explores/hooks/use-explore-list.ts`：`ExploreListState.promote` 改 `(name: string, workflowType: string) => Promise<void>`；新增 `updateWorkflowType: (name: string, workflowType: string) => Promise<void>`；`ExploreDetailActions` 同步；`useExploreDetailActions` 的 `promote` 调 `commands.promoteExplore(root, name, workflowType)`、新增 `updateWorkflowType` 调 `commands.updateExploreWorkflowType(root, name, workflowType)` 后 `refresh()`（失败 onError + rethrow）
- [x] 修改 `src/views/explores/explore-detail-view.tsx`：新增 `useWorkflowTypeBackfill(content, recordName, currentWorkflowType, promotedTo, updateWorkflowType)`（沿 `useTitleBackfill`，`extractWorkflowType` 命中且不同则 `updateWorkflowType(...).catch(() => {})` 静默）；`usePromoteAction` 增 `workflowType` 本地态（默认 `record.workflowType`）+ `onWorkflowTypeChange`（`updateWorkflowType` 落库）；`ExploreDetailHeader` 草稿态在「启动变更」旁内联两选项 select（`data-testid="explore-workflow-type-select"`）；`onPromote` 调 `promote(record.name, workflowType)`

## 阶段九：前端新建类型选择器

- [x] 修改 `src/views/changes/components/change-create-dialog.tsx`：`CreateFormData` 加 `workflowType: string`（初值 `'requirement'`）；`CreateForm` 增 `WorkflowTypeField`（内联普通 select，两字面选项 requirement 默认 / test-only，`data-testid="change-create-workflow-type"`）；`onSubmit` 调 `commands.createChange(root, trimmedName, trimmedGoal, formData.workflowType)`；成功后 reset 时 `workflowType` 归 `'requirement'`

## 阶段十：前端流程图层列序出线

- [x] 修改 `src/views/changes/flow/layout.ts`：删 `export const PIPELINE_PHASES`（D12 彻底删除）；`COL_W` / `ROW_H` / `COLUMN_HEADER_H` / `nodePosition` 保留
- [x] 修改 `src/views/changes/flow/graph.ts`：去 `PIPELINE_PHASES` import；`buildFlowGraph` 的 `columns` 改 `detail.pipeline.map((entry, colIndex) => ({ id: \`col:${entry.phase}\`, phase: entry.phase, colIndex }))`
- [x] 修改 `src/views/changes/flow/run-state.ts`：去 `PIPELINE_PHASES` import；`runStepNodes(steps: ChangeStepState[], phaseOrder: string[]): RuntimeFlowNode[]` 增 `phaseOrder` 入参，`colIndex = phaseOrder.indexOf(step.phase)`（`< 0` 不入图）；`nodeIdentityKey` / `unifiedRunNodes` / `unifiedRunEdges` / 词汇归一保留
- [x] 修改 `src/views/changes/flow/attachments.ts`：去 `PIPELINE_PHASES` import；`artifactColumn` 改收 pipeline 相位集（或由 `mountMaterials` 内部 `new Set(detail.pipeline.map(p => p.phase))` 派生后闭包传参），`PIPELINE_PHASES.indexOf` → 相位集成员判断；`mountMaterials(detail, envelopes)` 签名不变
- [x] 修改 `src/views/changes/change-detail-view.tsx`：`useFlowGraphData` 内 `const phaseOrder = detail.pipeline.map(p => p.phase);` 并 `runStepNodes(unifiedRunNodes(...), phaseOrder)` 传序；`buildFlowGraph(detail, runNodes, runEdges)` / `mountMaterials(detail, artifacts)` 调用不变

## 阶段十一：版本交付（归档时执行）

- [x] 修改 `packages/desktop/package.json`：`version` 0.4.40 → 0.4.41（`src-tauri/tauri.conf.json` 维持 `../package.json` 引用自动跟随，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 保持 2.10.44 零改动）
