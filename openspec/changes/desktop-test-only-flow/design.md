# 设计: desktop-test-only-flow

> **变更**: desktop-test-only-flow
> **日期**: 2026-10-12
> **基于**: proposal.md、explore.md（三轮已收敛）、specs/ 八个能力 delta（desktop-workflow-write-face / desktop-change-queries / desktop-change-flow-view / desktop-change-create / desktop-change-orchestration / desktop-explore-page / desktop-explore-queries / desktop-workspace-store）

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 八份 delta 已由提案阶段定稿，本设计基于其文本展开，只覆盖「变更范围 - 实现文件」，并把探索收敛的八项决策与三项「design 定稿」事项落到类型面 / 命令面 / 前端面 / 记录面的具体形态。

工作树实况（本变更起点，`packages/desktop` 0.4.40 基线）：

- `write/phase_table.rs`：`phase_table(workflow_type)` 只认 `"requirement"`（其余 `None`）；`PhaseDefinition` 无 `prerequisites` 字段；独立 `prerequisites()` match 写死 requirement 依赖图；`dependents(table, phase)` 依赖它；无 `CODE_ANALYZE_STEPS` / `TEST_ONLY_TABLE`；`prompts/` 14 个角色 md，无 code-analyze。
- `write/create.rs:205`：`workflow_type` 硬编码 `"requirement"`；`write/promote.rs` 无 `workflow_type` 入参。
- `write/{phase_next,phase_start,phase_log,backtrack}.rs`：`phase_table` `None` 时文案「V1 仅 requirement 工作流」。
- `queries/detail.rs`：`PIPELINE_PHASES` 10 站常量（含 code-analyze 占位列）双侧同步；`change_detail` 用固定 10 站建 pipeline、产物清单 `phase_table(...).unwrap_or_default()`。
- `commands/change_flow/mod.rs:103`：前置校验拒绝非 requirement，文案「仅支持 requirement 工作流」。
- `commands/changes/mod.rs`：`create_change` / `create_change_with` 三参（root/name/goal）；`commands/explores/mod.rs`：`promote_explore` / `promote_explore_with` 二参（root/name），无 `update_explore_workflow_type`。
- `store/model.rs`：`ExploreRecord` native_model v2（`from = ExploreRecordV1`），无 `workflow_type` 字段；`store/store.rs` 有 `set_explore_title` / `mark_explore_promoted`，无 `set_explore_workflow_type`。
- 前端 `flow/layout.ts` 的 `PIPELINE_PHASES`（10 站）为列序来源，`graph.ts` / `run-state.ts` / `attachments.ts` 三处消费 `PIPELINE_PHASES.indexOf`；`explore-stance.ts` 只约定首行标题；`explore-title.ts` 只 `extractMarkdownTitle`；`use-explore-list.ts` `promote(name)`；`change-create-dialog.tsx` 无类型选择器。

本设计描述**本变更完成后的目标形态**：`workflow_type` 两值分派（requirement 9 相位 / test-only 6 相位）、code-analyze 相位补全、前置依赖表内自描述、读写都严格、列序由 `detail.pipeline` 出线、建档入口透出类型、promote 类型推算与回填。

---

## 关键设计决策（proposal 定稿 + 本设计展开）

| # | 决策 | 定稿 | 理由 |
|---|------|------|------|
| D1 | `TEST_ONLY_TABLE` = 6 相位（5 路由 + archive） | `proposal → code-analyze → test-design → test-gen → test-execution → archive`；archive 复用 `ARCHIVE_STEPS`（executor/evaluator 均 None） | desktop 的 archive 是 `stop_before=archive` 手动门 + 确定性相位，requirement 同理；与 requirement「archive 表内末相位」形状一致 |
| D2 | phase→step 先复用；code-analyze 新增 | proposal / test-design / test-gen / test-execution / archive 复用 requirement 同名 static；新增 `CODE_ANALYZE_STEPS`（P→E，与 proposal/dev-design 同形 `[PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]`）+ 新增 `code-analyze-planner.md` / `code-analyze-evaluator.md` | 同一 phase 的 step 映射不受类型影响；test-execution 沿用 desktop 确定性格（`[PhaseStart, TestExecution, PhaseLog]`），不照搬插件侧 Executor→E（既有差异非本次引入） |
| D3 | `PhaseDefinition` 加 `prerequisites` 字段，删独立 `prerequisites()` match，`dependents` 遍历表内自描述 | `pub prerequisites: &'static [&'static str]`；`dependents(table, phase)` 改 `candidate.prerequisites.contains(&phase)`；requirement 照搬现 match、test-only 照 pge spec | 表内自描述杜绝第二份 match，类型感知随表分派天然成立 |
| D4 | 列序由 `detail.pipeline` 出线承载，前端 `PIPELINE_PHASES` 退役 | `change_detail` 建站改 `phase_table(record.workflow_type)` 相位序；前端 `buildFlowGraph` 列序 = `detail.pipeline.map(p => p.phase)`；`runStepNodes(steps, phaseOrder)` 显式传序 | `detail.pipeline` 已按相位序出线，前端零第二份类型→相位表，杜绝双侧同步漂移 |
| D5 | 读写都严格，不 fallback requirement | `phase_table` 只认 `{requirement, test-only}`；写面 / 发起面未知类型显式 `Err`；读面未知类型 `change_detail` 返回 `None`（见 D9）；存量 ChangeRecord 恒 requirement（零迁移）、ExploreRecord v3 默认 requirement | 正常形态不存在未知类型 change；脏值不应被静默当 requirement 跑 |
| D6 | promote 类型推算走 title 同模式 | stance 约定笔记写 `workflow_type:` 标记行 → 前端 `extractWorkflowType` 解析 → `update_explore_workflow_type` 显式回填 | 与 `extractMarkdownTitle` / `update_explore_title` 同构，零新机制 |
| D7 | `ExploreRecord.workflow_type` 单字段 | 人工覆盖后回填落库，promote 读它 | 沿 title 改即写模式，语义最简；suggested vs selected 双字段为两处真相源 |
| D8 | 类型选择器不抽共享组件 | promote 与 change_create 各自内联两选项普通 select | 跨入口复用一个两选项 select 属 speculative abstraction，违背「无单次使用抽象」纪律 |
| D9 | 未知 workflow_type 读面呈现形态（本设计定稿） | `change_detail` 对 `phase_table(record.workflow_type) == None` 返回 `None`（与未知 id 同语义「详情不可达」）；IPC 面 `get_change_detail` 保持 `Option<ChangeDetailUnified>` 零签名改动 | 三选一（None / 空 pipeline / Result）中 `None` 最贴合既有 `Option` 签名、不牵动 IPC 面，且 MUST NOT 静默当 requirement 跑 |
| D10 | `extractWorkflowType` 标记行格式与文件归属（本设计定稿） | stance 约定「首行 `# <标题>`、第二行 `workflow_type: <requirement\|test-only>`」；解析器旁置新文件 `src/lib/explore-workflow-type.ts`，逐行扫描首个匹配 `^workflow_type:\s*(\S+)\s*$`，值 case-insensitive 命中 `{requirement, test-only}` 返回小写规范值，否则 `null` | title 同模式但解析比「只读首行」更容忍 agent 输出抖动（R4 缓解）；旁置新文件保持 `explore-title.ts` 语义纯净 |
| D11 | store 自持 workflow_type 合法值集（本设计定稿） | `store.rs` 私有 `fn is_valid_workflow_type(value: &str) -> bool`（trim + case-insensitive 命中 `{requirement, test-only}`），写入落 canonical 小写值；不依赖 workflow crate（store MUST NOT 依赖 workflow）；命令层另经 `phase_table` 校验（双保险） | store 自持校验与既有的 kebab-case 自持校验同纪律；workflow_type 合法值唯一权威仍是后端 `phase_table` |
| D12 | requirement 列数 10→9、`PIPELINE_PHASES` 常量彻底删除（本设计定稿） | 后端 `PIPELINE_PHASES` 常量退役删除；前端 `layout.ts` 的 `PIPELINE_PHASES` 彻底删除（非退役为测试夹具）；测试各自内联 fixture | 「10 列恒定」随 D4 被推翻，code-analyze 占位列消失；生产侧零残留更彻底，测试 fixture 由测试相位自备 |
| D13 | code-analyze executor/evaluator 档位（本设计定稿） | 均 `ModelLevel::High`（与 proposal / dev-design / test-design 同档的 P→E 设计相位） | 分析与设计相位同档，无证据需降档 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 相位表两值分派 + 计划图单源 | `PhaseDefinition` 加 `prerequisites` 字段；删独立 `prerequisites()` match；`dependents` 遍历表内自描述；新增 `CODE_ANALYZE_STEPS` / `TEST_ONLY_TABLE`；`phase_table` 两值分派（未知 `None`） | `crates/core/workflow/src/write/phase_table.rs` | `agent::ModelLevel`、`crate::state` | Rust（编译期 const 表 + `include_str!`） |
| code-analyze 角色 prompt | 新增 planner / evaluator 两个全静态角色知识 md，`include_str!` 显式字面路径装配进 `CODE_ANALYZE_STEPS` 与 `PhaseAgentSpec` | `crates/core/workflow/src/prompts/code-analyze-planner.md` + `code-analyze-evaluator.md` | `phase_table.rs` 编译期装配 | Markdown（静态文本） |
| 写面建档类型入参 | `create` 加 `workflow_type` 入参（前置校验 + 落 canonical 小写），替换硬编码；`promote_explore` 加 `workflow_type` 透传 `create` | `crates/core/workflow/src/write/create.rs` + `promote.rs` | `phase_table`、`state`、`worktree` | Rust（sync） |
| 写面严格类型分派 | `phase_next` / `phase_start` / `phase_log` / `backtrack` 对 `phase_table` `None` 显式 `Err`，文案通吃 `{requirement, test-only}` | `crates/core/workflow/src/write/{phase_next,phase_start,phase_log,backtrack}.rs` | `phase_table` | Rust（sync） |
| 详情聚合列序出线 | 删 `PIPELINE_PHASES` 常量；`change_detail` 先取 `phase_table(record.workflow_type)`（`None` → 返回 `None`），建站与产物清单同用该表相位序 | `crates/core/workflow/src/queries/detail.rs` | `phase_table`、`state`、`artifacts`、`foundation::layout` | Rust（sync） |
| 发起前置校验类型分派 | `change_flow_start` 保留「`phase_table` `None` → 显式拒绝」门，文案改通吃 `{requirement, test-only}`；walker 零 workflow_type 分支 | `src/commands/change_flow/mod.rs` | `workflow::write::phase_table` | Rust、tauri |
| IPC 建档命令面 | `create_change` / `create_change_with` 加 `workflow_type` 入参透传写面 | `src/commands/changes/mod.rs` | `workflow::write`、`store`、`vcs_runtime` | Rust、tauri、specta |
| IPC promote / 类型回填命令面 | `promote_explore` / `promote_explore_with` 加 `workflow_type` 入参；新增 `update_explore_workflow_type` 命令 | `src/commands/explores/mod.rs` | `workflow::write`、`workflow::queries`、`store` | Rust、tauri、specta |
| 命令登记面 | `all_commands!` 登记 `update_explore_workflow_type` | `src/commands/mod.rs` | — | Rust 宏 |
| ExploreRecord v3 | `ExploreRecord` 升 v3（`from = ExploreRecordV2`）加 `workflow_type`；新增 `ExploreRecordV2` 历史形态 + 双向 `From`（升级默认 `"requirement"`）；`ExploreRecord::new` 默认 `"requirement"` | `crates/infra/store/src/model.rs` | `native_model`、`specta` | Rust |
| explore 类型回填操作面 | 新增 `set_explore_workflow_type(root, name, workflow_type)`（in-place 写 + 刷新 `updated_at`；空白 / 非法值 → `Err`）；`create_explore_record` 经 `ExploreRecord::new` 默认 requirement | `crates/infra/store/src/store.rs` | `model` | Rust（sync，自持校验不依赖 workflow） |
| 前端 stance 类型推算约定 | `stancePreamble` 增「第二行 `workflow_type:` 标记行」约定 + 判断标准（补测试 / 生成测试用例 → test-only，否则 requirement） | `src/lib/explore-stance.ts` | — | TypeScript（纯文本模板） |
| 前端类型标记解析 | `extractWorkflowType(content)` 纯函数：逐行首个 `workflow_type:` 标记 → `"requirement"` / `"test-only"` / `null` | `src/lib/explore-workflow-type.ts`（旁置新文件） | — | TypeScript（纯函数） |
| 前端 explore 类型回填 + promote 选择器 | `useWorkflowTypeBackfill`（沿 `useTitleBackfill`）；promote 入口内联两选项 select（默认 `record.workflowType`，可改），改选经 `updateWorkflowType` 落库 | `src/views/explores/explore-detail-view.tsx` | `explore-workflow-type`、`use-explore-list` | TypeScript、React |
| 前端 explore 动作面 | `promote(name, workflowType)` 与 `updateWorkflowType(name, workflowType)` 签名随动 | `src/views/explores/hooks/use-explore-list.ts` | `types/generated/bindings` | TypeScript、React |
| 前端新建类型选择器 | `CreateFormData` 加 `workflowType`（默认 requirement）；内联普通 select；提交四参 `createChange(root, name, goal, workflowType)` | `src/views/changes/components/change-create-dialog.tsx` | `types/generated/bindings` | TypeScript、React |
| 前端流程图层列序出线 | `layout.ts` 删 `PIPELINE_PHASES`；`graph.ts` 列序改 `detail.pipeline.map(p => p.phase)`；`run-state.ts` `runStepNodes(steps, phaseOrder)` 显式传序；`attachments.ts` `artifactColumn` 改收 pipeline 相位集；`change-detail-view.tsx` 传 `phaseOrder` | `src/views/changes/flow/{layout,graph,run-state,attachments}.ts` + `src/views/changes/change-detail-view.tsx` | `types/dto` | TypeScript（纯函数 + React） |
| bindings 再生 | `create_change` 四参 / `promote_explore` 三参 / `update_explore_workflow_type` 经 tauri-specta 再生 | `src/types/generated/bindings.ts` | — | codegen |
| 版本交付 | `version` 0.4.40 → 0.4.41（归档时执行） | `packages/desktop/package.json` | — | package.json |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/code-analyze-planner.md` | code-analyze 相位 executor 角色知识 md（全静态零占位）；经 `include_str!` 显式字面路径装配为 `CODE_ANALYZE_EXECUTOR_PROMPT` 与 `CODE_ANALYZE_STEPS` 的 Executor 步 `prompt_ref`。见「公共函数 / API」的 `CODE_ANALYZE_EXECUTOR_PROMPT` 装配引用 |
| `packages/desktop/src-tauri/crates/core/workflow/src/prompts/code-analyze-evaluator.md` | code-analyze 相位 evaluator 角色知识 md（全静态零占位）；装配为 `CODE_ANALYZE_EVALUATOR_PROMPT` 与 Evaluator 步 `prompt_ref` |
| `packages/desktop/src/lib/explore-workflow-type.ts` | `extractWorkflowType` 纯函数（D10）：解析笔记约定标记行。见「公共函数 / API」的 `extractWorkflowType` |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` | `PhaseDefinition` 加 `pub prerequisites: &'static [&'static str]` 字段；`REQUIREMENT_TABLE` 每相位补 `prerequisites`（proposal=[] / dev-design=[proposal] / test-design=[proposal,dev-design] / implement=[dev-design] / test-gen=[test-design,implement] / test-execution=[test-gen,implement] / code-review=[test-gen,implement] / acceptance=[proposal,dev-design,implement] / archive=[]）；删独立 `fn prerequisites()` match；`dependents` 改 `candidate.prerequisites.contains(&phase)`；新增 `CODE_ANALYZE_EXECUTOR_PROMPT` / `CODE_ANALYZE_EVALUATOR_PROMPT` 两个 `include_str!` 常量、`CODE_ANALYZE_STEPS`（`[PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]`，VerdictGate `on_fail=Terminal`）、`TEST_ONLY_WORKFLOW_TYPE` 常量与 `TEST_ONLY_TABLE`（6 相位，proposal description=「测试向提案」、code-analyze executor/evaluator `ModelLevel::High`、archive executor/evaluator None）；`phase_table` 改两值分派（trim + eq_ignore_ascii_case 命中 `requirement` / `test-only`，其余 `None`） | 相位表两值分派 + 前置依赖表内自描述 + code-analyze 补全（AC-1/2/3） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` | `create` 签名加 `workflow_type: &str`（位于 `title` 之后）；前置②校验段加 `let workflow_type = workflow_type.trim().to_ascii_lowercase(); if phase_table(&workflow_type).is_none() { return Err(...) }`；`ChangeStateRecord` 的 `workflow_type` 字段由硬编码 `"requirement".to_owned()` 改为 `workflow_type.clone()` | 建档类型入参 + 前置校验（AC-7） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs` | `promote_explore` 签名加 `workflow_type: &str`；`super::create(...)` 调用加 `workflow_type` 透传 | promote 类型透传（AC-8） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_next.rs` | `phase_table` `None` 文案 `workflow_type "{wf}" 不受支持（V1 仅 requirement 工作流）` → `workflow_type "{wf}" 不受支持（支持 requirement / test-only 工作流）` | 写面严格分派文案（AC-4） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_start.rs` | 同上文案改写 | 写面严格分派文案（AC-4） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_log.rs` | 同上文案改写 | 写面严格分派文案（AC-4） |
| `packages/desktop/src-tauri/crates/core/workflow/src/write/backtrack.rs` | 同上文案改写 | 写面严格分派文案（AC-4；backtrack stale 传播随表内 prerequisites 天然类型感知） |
| `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` | 删 `pub const PIPELINE_PHASES`；`change_detail` 在 `get_change` 命中后立即 `let table = phase_table(&record.workflow_type)?;`（`None` → 返回 `None`，读面不 fallback）；pipeline 建站改 `table.iter().map(\|def\| PhaseEntry { phase: def.id.to_string(), attempts: Vec::new() })`；产物清单 `let manifest = table;`（去 `unwrap_or_default`） | 列序随 workflow_type 出线 + 读面严格（AC-4/5） |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | `create_change` 加 `workflow_type: String` 入参（四参）；`title = name.clone()` 后调 `create_change_with(app, root, name, goal, title, workflow_type)`；`create_change_with` 加 `workflow_type: String` 入参并在 `spawn_blocking` 内 `write::create(..., &title, &workflow_type)` | IPC 建档四参透传（AC-7） |
| `packages/desktop/src-tauri/src/commands/explores/mod.rs` | `use workflow::write::{self, phase_table, PromoteOutcome};`；`promote_explore` 加 `workflow_type: String` 入参（三参）→ `promote_explore_with(app, root, name, workflow_type)`；`promote_explore_with` 加 `workflow_type` 入参、校验 `phase_table(&workflow_type).is_none()` → `Err`、`write::promote_explore(..., &title, &workflow_type)`；新增 `#[tauri::command] #[specta::specta] pub fn update_explore_workflow_type(stores, root, name, workflow_type) -> Result<ExploreRecord, String>`（blank root / `phase_table` 不命中 → `Err`，调 `set_explore_workflow_type`） | IPC promote 三参 + 类型回填命令（AC-8/9） |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | 前置校验 2 文案 `仅支持 requirement 工作流（change "{}" 的 workflow_type 为 "{}"）` → `仅支持 requirement / test-only 工作流（change "{}" 的 workflow_type 为 "{}"）`；注释同步 | 发起前置校验通吃文案（AC-4） |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `all_commands!` 增 `$crate::commands::explores::update_explore_workflow_type` | 命令登记（AC-9） |
| `packages/desktop/src-tauri/crates/infra/store/src/model.rs` | `ExploreRecord` 升 `#[native_model(id = 4, version = 3, from = ExploreRecordV2)]` 加 `pub workflow_type: String` 字段；新增 `ExploreRecordV2` 历史形态（`#[native_model(id = 4, version = 2, from = ExploreRecordV1)]`，字段 = 现 v2 七字段）；`impl From<ExploreRecordV1> for ExploreRecordV2`（原 v1→v2 升级语义平移：title=name、promoted_to=None）；`impl From<ExploreRecordV2> for ExploreRecordV1`（降级：丢 title/promoted_to）；`impl From<ExploreRecordV2> for ExploreRecord`（升级：workflow_type=`"requirement"`，其余直透）；`impl From<ExploreRecord> for ExploreRecordV2`（降级：丢 workflow_type）；`ExploreRecord::new` 加 `workflow_type: "requirement".to_owned()` | ExploreRecord v3 decode-only 升级（AC-9） |
| `packages/desktop/src-tauri/crates/infra/store/src/store.rs` | 新增私有 `fn is_valid_workflow_type(value: &str) -> bool`（trim + case-insensitive 命中 `{requirement, test-only}`，自持不依赖 workflow）；新增 `pub fn set_explore_workflow_type(&self, root: &str, name: &str, workflow_type: &str) -> Result<ExploreRecord, StoreError>`（非法值 → `Err`；`find_explore_record` → in-place 写 `workflow_type`（canonical 小写）+ 刷新 `updated_at` → `upsert` + commit，抄 `set_explore_title` 事务形态） | 类型回填操作面（AC-9） |
| `packages/desktop/src/lib/explore-stance.ts` | `stancePreamble` 的「笔记落盘」段在首行标题约定后增：`- 笔记第二行写 \`workflow_type: <requirement|test-only>\`（推算变更类型：目标是补测试 / 生成测试用例 → test-only，否则 requirement）`；注释与 `plugins/dev-team/skills/openspec-explore/SKILL.md` 双源互链随动 | stance 类型推算约定（AC-8） |
| `packages/desktop/src/views/explores/hooks/use-explore-list.ts` | `ExploreListState.promote` 改 `(name: string, workflowType: string) => Promise<void>`；新增 `updateWorkflowType: (name: string, workflowType: string) => Promise<void>`；`ExploreDetailActions` 同步；`useExploreDetailActions` 的 `promote` 调 `commands.promoteExplore(root, name, workflowType)`、新增 `updateWorkflowType` 调 `commands.updateExploreWorkflowType(root, name, workflowType)` 后 `refresh()`（失败 onError + rethrow） | 详情动作面签名随动（AC-8） |
| `packages/desktop/src/views/explores/explore-detail-view.tsx` | 新增 `useWorkflowTypeBackfill(content, recordName, currentWorkflowType, promotedTo, updateWorkflowType)`（沿 `useTitleBackfill`，`extractWorkflowType` 命中且不同则回填，失败静默）；`usePromoteAction` 增 `workflowType` 本地态（默认 `record.workflowType`）+ `onWorkflowTypeChange`（改即写 `updateWorkflowType` 落库）；`ExploreDetailHeader` 草稿态在「启动变更」按钮旁内联两选项 select（`data-testid="explore-workflow-type-select"`）；`onPromote` 调 `promote(record.name, workflowType)` | promote 类型选择器 + 回填（AC-8） |
| `packages/desktop/src/views/changes/components/change-create-dialog.tsx` | `CreateFormData` 加 `workflowType: string`（初值 `'requirement'`）；`CreateForm` 增 `WorkflowTypeField`（内联普通 select，两字面选项 requirement 默认 / test-only，`data-testid="change-create-workflow-type"`）；`onSubmit` 调 `commands.createChange(root, trimmedName, trimmedGoal, formData.workflowType)`；成功后 reset 时 `workflowType` 归 `'requirement'` | 新建类型选择器 + 四参 invoke（AC-7） |
| `packages/desktop/src/views/changes/flow/layout.ts` | 删 `export const PIPELINE_PHASES`（D12 彻底删除）；其余 `COL_W` / `ROW_H` / `COLUMN_HEADER_H` / `nodePosition` 保留 | 前端列序常量退役（AC-5） |
| `packages/desktop/src/views/changes/flow/graph.ts` | 去 `PIPELINE_PHASES` import；`buildFlowGraph` 的 `columns` 改 `detail.pipeline.map((entry, colIndex) => ({ id: \`col:${entry.phase}\`, phase: entry.phase, colIndex }))`（空 pipeline 仍返回空图） | 列序自 pipeline 出线（AC-5/6） |
| `packages/desktop/src/views/changes/flow/run-state.ts` | 去 `PIPELINE_PHASES` import；`runStepNodes(steps: ChangeStepState[], phaseOrder: string[]): RuntimeFlowNode[]` 增 `phaseOrder` 入参，`colIndex = phaseOrder.indexOf(step.phase)`（`< 0` 不入图，列内序仍按 phase 计数）；`nodeIdentityKey` / `unifiedRunNodes` / `unifiedRunEdges` / 词汇归一保留 | run 步节点显式传序（AC-5） |
| `packages/desktop/src/views/changes/flow/attachments.ts` | 去 `PIPELINE_PHASES` import；`artifactColumn(phase, pipelinePhases: Set<string> | null)` 改收 pipeline 相位集（或由 `mountMaterials` 内部从 `detail.pipeline` 派生相位集后闭包传参），`PIPELINE_PHASES.indexOf` → `pipelinePhases.has(phase)`；`mountMaterials(detail, envelopes)` 签名不变，内部先 `new Set(detail.pipeline.map(p => p.phase))` | 素材归属校验随出线序（AC-5） |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | `useFlowGraphData` 内先 `const phaseOrder = detail.pipeline.map(p => p.phase)`；`runStepNodes(unifiedRunNodes(...), phaseOrder)` 传序；`buildFlowGraph(detail, runNodes, runEdges)` 不变；memo 依赖 `[detail, activeRun, phaseOrder]` 按需（phaseOrder 自 detail 派生，可并入 detail 依赖） | 前端传序接线（AC-5/6） |
| `packages/desktop/src/types/generated/bindings.ts` | 随 `create_change` 四参 / `promote_explore` 三参 / 新增 `update_explore_workflow_type` 与 `ExploreRecord.workflowType` 字段经 `bindings:export` 再生（非手改） | 类型再生跟随（AC-10） |
| `packages/desktop/package.json` | `version` 0.4.40 → 0.4.41（归档时执行） | 版本交付（AC-11） |

### 删除文件

- 无（`PIPELINE_PHASES` 常量为删除（D12），不删整文件；独立 `prerequisites()` 函数随 phase_table.rs 内编辑删除）。

> **零改动沿用**（proposal「变更范围 - 实现文件」之外或明确不动的既有面）：`crates/core/orchestration`（walker 步执行 / 会话 provenance / 决策协议 / 检查域 / vcs 缝零改动，walker 零 workflow_type 分支）；`ChangeRecord` 模型与 `desktop-change-state-store`（`workflow_type` 字段既有，零 model delta）；`RunNodeRecord` / `RunStepKind` / `StepNode` 十八值词汇（code-analyze 复用既有 Executor / Evaluator / VerdictGate 步词）；`crates/core/workflow/src/prompts/` 既有 14 个角色 md（零改动，仅新增 code-analyze 两 md）；`plugins/dev-team` 全部（版本 2.10.44、三类交付产物零改动）；`workflow_type` 合法值权威仍为后端 `phase_table`（前端仅两处内联 select 字面选项，零第二份值表）。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `phase_table` | `crates/core/workflow/src/write/phase_table.rs` | 修改 | `pub fn phase_table(workflow_type: &str) -> Option<&'static [PhaseDefinition]>` | 两值分派：`requirement` → 9 相位、`test-only` → 6 相位、其余（含空白 / bug-fix）→ `None`（不退化） |
| `PhaseDefinition` | `crates/core/workflow/src/write/phase_table.rs` | 修改 | `pub struct PhaseDefinition { pub id, pub description, pub steps, pub artifact_manifest, pub executor, pub evaluator, pub prerequisites: &'static [&'static str] }` | 增 `prerequisites` 表内自描述字段 |
| `create` | `crates/core/workflow/src/write/create.rs` | 修改 | `pub fn create(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name: &str, goal: &str, title: &str, workflow_type: &str) -> Result<CreateOutcome, String>` | 加 `workflow_type` 入参；前置校验 `phase_table` 命中 + 落 canonical 小写；替换硬编码 |
| `promote_explore`（写面） | `crates/core/workflow/src/write/promote.rs` | 修改 | `pub fn promote_explore(main_root: &Path, worktree_root: &Path, store: &dyn ChangeStateStore, vcs: &dyn WorktreePort, name: &str, note: &str, title: &str, workflow_type: &str) -> Result<PromoteOutcome, String>` | 加 `workflow_type` 透传 `create` |
| `create_change`（IPC） | `src/commands/changes/mod.rs` | 修改 | `#[tauri::command] #[specta::specta] pub async fn create_change(app: AppHandle, root: String, name: String, goal: String, workflow_type: String) -> Result<CreateOutcome, String>` | 三参 → 四参；`title = name`；透传 `create_change_with` |
| `create_change_with` | `src/commands/changes/mod.rs` | 修改 | `pub(crate) async fn create_change_with<R: tauri::Runtime>(app: AppHandle<R>, root: String, name: String, goal: String, title: String, workflow_type: String) -> Result<CreateOutcome, String>` | 泛型测试缝加 `workflow_type` 透传写面 |
| `promote_explore`（IPC） | `src/commands/explores/mod.rs` | 修改 | `#[tauri::command] #[specta::specta] pub async fn promote_explore(app: AppHandle, root: String, name: String, workflow_type: String) -> Result<PromoteOutcome, String>` | 二参 → 三参 |
| `promote_explore_with` | `src/commands/explores/mod.rs` | 修改 | `pub(crate) async fn promote_explore_with<R: tauri::Runtime>(app: AppHandle<R>, root: String, name: String, workflow_type: String) -> Result<PromoteOutcome, String>` | 加 `workflow_type`（`phase_table` 命中校验 + 透传写面） |
| `update_explore_workflow_type`（IPC） | `src/commands/explores/mod.rs` | 新增 | `#[tauri::command] #[specta::specta] pub fn update_explore_workflow_type(stores: State<'_, WorkspaceStores>, root: String, name: String, workflow_type: String) -> Result<ExploreRecord, String>` | blank root / 非法值 → `Err`；调 store `set_explore_workflow_type`；workflow_type 回填唯一写入口 |
| `ExploreRecord::new` | `crates/infra/store/src/model.rs` | 修改 | `pub fn new(root: &str, name: &str, now: i64) -> Self` | 签名不变；新增 `workflow_type = "requirement"` 默认 |
| `set_explore_workflow_type` | `crates/infra/store/src/store.rs` | 新增 | `pub fn set_explore_workflow_type(&self, root: &str, name: &str, workflow_type: &str) -> Result<ExploreRecord, StoreError>` | in-place 写 + 刷新 `updated_at`；非法值 → `Err`；记录 miss → `Err` |
| `buildExplorePrompt` | `src/lib/explore-stance.ts` | 修改 | `export function buildExplorePrompt(userInput: string, topic: string): string` | 签名不变；stance 文本增 `workflow_type:` 标记行约定与判断标准 |
| `extractWorkflowType` | `src/lib/explore-workflow-type.ts` | 新增 | `export function extractWorkflowType(content: string): "requirement" \| "test-only" \| null` | 逐行首个 `workflow_type:` 标记 → 小写规范值 / `null`（无标记 / 非法值不报错） |
| `runStepNodes` | `src/views/changes/flow/run-state.ts` | 修改 | `export function runStepNodes(steps: ChangeStepState[], phaseOrder: string[]): RuntimeFlowNode[]` | 增 `phaseOrder`；`colIndex = phaseOrder.indexOf(step.phase)`，外相位不入图 |
| `buildFlowGraph` | `src/views/changes/flow/graph.ts` | 修改 | `export function buildFlowGraph(detail: ChangeDetail, runNodes?: RuntimeFlowNode[], runEdges?: readonly ChangeRunEdgeRecord[]): FlowGraph` | 签名不变；`columns` 改 `detail.pipeline.map(...)`（colIndex = 下标） |
| `mountMaterials` | `src/views/changes/flow/attachments.ts` | 修改 | `export function mountMaterials(detail: ChangeDetail, envelopes: ArtifactEnvelope[]): FlowMaterials` | 签名不变；内部以 `detail.pipeline` 相位集校验归属，去 `PIPELINE_PHASES.indexOf` |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `PhaseDefinition` | `crates/core/workflow/src/write/phase_table.rs` | 修改 | 增 `prerequisites: &'static [&'static str]` 字段 |
| `ExploreRecord` | `crates/infra/store/src/model.rs` | 修改 | 增 `workflow_type: String`（合法值 `requirement` \| `test-only`；v3 decode-only 升级默认 `"requirement"`） |
| `ExploreRecordV2` | `crates/infra/store/src/model.rs` | 新增 | `ExploreRecord` v2 历史形态（`id=4, version=2, from=ExploreRecordV1`），无 `workflow_type`；仅作 native_model 升级链解码目标，`pub(crate)` |
| `ExploreRecord`（TS bindings） | `src/types/generated/bindings.ts` | 修改 | 增 `workflowType: string`（随 v3 再生） |
| `ExploreListState` | `src/views/explores/hooks/use-explore-list.ts` | 修改 | `promote: (name: string, workflowType: string) => Promise<void>`；增 `updateWorkflowType: (name: string, workflowType: string) => Promise<void>` |
| `ExploreDetailActions` | `src/views/explores/hooks/use-explore-list.ts` | 修改 | 同上（`promote` 签名 + 增 `updateWorkflowType`） |
| `CreateFormData` | `src/views/changes/components/change-create-dialog.tsx` | 修改 | 增 `workflowType: string`（初值 `'requirement'`） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.4.41` | 0.4.40 → 0.4.41（归档时执行；`src-tauri/tauri.conf.json` 维持 `../package.json` 引用自动跟随，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 保持 2.10.44） |

> 测试文件（proposal「变更范围 - 测试文件」）由 test-design / test-gen / test-execution 相位承接，本清单不展开（仅实现文件）。它们包括：`write/phase_table_test.rs`、`write/{phase_next,phase_start,phase_log,backtrack}_test.rs`、`write/create_test.rs`、`queries/detail_test.rs`、`src/commands/changes/mod_test.rs`、`src/commands/explores/mod_test.rs`、`store/model_test.rs`（或既有 store 测试）、`src/lib/explore-title.test.ts`（或旁置）、`src/views/explores/explore-detail-view.test.tsx`、`src/views/changes/components/change-create-dialog.test.tsx`、`src/views/changes/flow/{graph,run-state,attachments,layout}.test.ts` 与 `change-flow-graph.test.tsx`（「恒 10 列」断言 / golden 显式重写）。

---

## 数据模型

### PhaseDefinition（编译期 const，无持久化）

新增 `prerequisites: &'static [&'static str]`（相位直接前置依赖，stale 传播与 `allowed_backtrack_phases` 推导的唯一事实源；MUST NOT 存在第二份独立 match）。两表数据源：

- requirement 照搬现 `prerequisites()` match（proposal=[]、dev-design=[proposal]、test-design=[proposal,dev-design]、implement=[dev-design]、test-gen=[test-design,implement]、test-execution=[test-gen,implement]、code-review=[test-gen,implement]、acceptance=[proposal,dev-design,implement]、archive=[]）。
- test-only 照 pge spec（proposal=[]、code-analyze=[proposal]、test-design=[proposal,code-analyze]、test-gen=[test-design]、test-execution=[test-gen]、archive=[]）。

### TEST_ONLY_TABLE 复用矩阵

| phase | steps | executor prompt | evaluator prompt | artifact_manifest |
|-------|-------|-----------------|------------------|-------------------|
| proposal | 复用 `PROPOSAL_STEPS` | 复用 `PROPOSAL_EXECUTOR_PROMPT` | 复用 `PROPOSAL_EVALUATOR_PROMPT` | 复用 `PROPOSAL_MANIFEST` |
| code-analyze | 新增 `CODE_ANALYZE_STEPS`（P→E） | 新增 `code-analyze-planner.md` | 新增 `code-analyze-evaluator.md` | `&[]`（零产物清单） |
| test-design | 复用 `TEST_DESIGN_STEPS` | 复用 `TEST_DESIGN_EXECUTOR_PROMPT` | 复用 `TEST_DESIGN_EVALUATOR_PROMPT` | `&[]` |
| test-gen | 复用 `TEST_GEN_STEPS` | 复用 `TEST_GEN_EXECUTOR_PROMPT` | 复用 `TEST_GEN_EVALUATOR_PROMPT` | `&[]` |
| test-execution | 复用 `TEST_EXECUTION_STEPS`（确定性门禁） | 复用 | 复用 | 复用 `TEST_EXECUTION_MANIFEST` |
| archive | 复用 `ARCHIVE_STEPS`（10 步确定性） | —（None） | —（None） | `&[]` |

### ChangeRecord（零 model delta）

`ChangeRecord.workflow_type: String` 字段既有（v4 已含），本变更零 model delta；写面 `create` 由硬编码改为入参写入（落 canonical 小写）。存量 `ChangeRecord.workflow_type` 自 V1 起恒 `"requirement"`，严格校验后存量天然合法，零迁移。

### ExploreRecord v3（持久化，decode-only 升级）

字段面新增 `workflow_type: String`（合法值 `requirement` | `test-only`；创建与升级默认 `"requirement"`，agent 推算类型经 `set_explore_workflow_type` 回填覆盖）。native_model 升级链：`ExploreRecordV1`（v1）→ `ExploreRecordV2`（v2，无 `workflow_type`）→ `ExploreRecord`（v3）。升级语义：v2→v3 读出 `workflow_type = "requirement"`、`title` / `promoted_to` 原值不变；降级半边形丢弃 `workflow_type`（只保升级语义真实性，不作数据承诺）。存量库 additive 打开，零手工迁移。

### 持久化

- 零新表。`ChangeRecord.workflow_type` / `ExploreRecord.workflow_type` 均为既有 workspace 库记录字段演进；`PhaseDefinition`（含 `prerequisites` / `steps` / `artifact_manifest`）为编译期 const，MUST NOT 落库。
- `set_explore_workflow_type` 为 in-place 写（保主键 → 保会话链绑定），刷新 `updated_at`；`promote_explore` 读 `record.workflow_type` 作为选择器默认（promote 入参 = 用户最终选择）。

---

## 路由 / API 设计

| method | path | 说明 | 输入 | 输出 | auth |
|--------|------|------|------|------|------|
| IPC `create_change` | — | 新建 change（四参，类型透传写面） | `root`, `name`, `goal`, `workflow_type` | `Result<CreateOutcome, String>` | 桌面本地 |
| IPC `promote_explore` | — | 探索启动变更（三参，类型透传写面） | `root`, `name`, `workflow_type` | `Result<PromoteOutcome, String>` | 桌面本地 |
| IPC `update_explore_workflow_type` | — | explore 类型回填（唯一写入口，读路径零写入） | `root`, `name`, `workflow_type` | `Result<ExploreRecord, String>` | 桌面本地 |
| IPC `get_change_detail` | — | 统一视图；未知 id 或未知 `workflow_type` 均 `None`（不静默当 requirement） | `root`, `id` | `Option<ChangeDetailUnified>` | 桌面本地 |
| IPC `list_changes` | — | 列表（`ChangeSummary` 不含 workflow_type，零改动） | `root` | `ChangeList` | 桌面本地 |

`workflow_type` 非法值（`phase_table` 不命中）在写面 `create` / 命令层 `promote_explore_with` / `update_explore_workflow_type` 均显式 `Err`；`update_explore_workflow_type` 的 store 层再自持校验兜底（D11）。

---

## 依赖

### 运行时依赖

- 零新增 crate / npm 依赖。
- `core/workflow::write::create` / `promote` → `phase_table`（既有 crate 内依赖）：新增 `workflow_type` 合法性校验。
- `src/commands/changes` / `src/commands/explores` → `workflow::write::{create, promote_explore, phase_table}`（既有）：签名随动 + 类型校验。
- `src/commands/explores` → `store::set_explore_workflow_type`（既有 store 依赖）：新增回填命令消费。
- 前端 `use-explore-list` / `change-create-dialog` → `types/generated/bindings`（既有）：`createChange` / `promoteExplore` / `updateExploreWorkflowType` typed 包装随再生。
- `store` 自持 workflow_type 合法值集（`{requirement, test-only}`），MUST NOT 新增对 `workflow` crate 的依赖（沿 store 自持校验纪律）。

### 构建 / 测试依赖

- 零新增；`bindings:export`（`cargo run -p export-bindings`）再生 `create_change` 四参 / `promote_explore` 三参 / `update_explore_workflow_type` 与 `ExploreRecord.workflowType`，`bindings:check` 一致性守卫拦截漂移；golden 走 `DESKTOP_GOLDEN_REWRITE=1` 显式重写（desktop-corpus-regression）。

---

## 待决问题

- **R3 语义边界（留待看效果，非本次实现阻塞）**：test-only 的 `proposal` 复用 requirement proposal prompt（描述字段改「测试向提案」），是否拆分独立 test-proposal prompt 留待实际产出效果评估后另行 change。
- 无其余未决事项。proposal 的 D1–D8 已定稿，本设计补充 D9（未知类型读面返回 `None`）、D10（`workflow_type:` 标记行格式与旁置解析文件）、D11（store 自持合法值集）、D12（requirement 10→9 列 + `PIPELINE_PHASES` 彻底删除）、D13（code-analyze 档位 `High`）五点定稿，全部落到类型面 / 命令面 / 前端面 / 记录面。
