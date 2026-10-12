# 提案: desktop-test-only-flow

> **变更**: desktop-test-only-flow
> **日期**: 2026-10-12
> **状态**: draft
> **探索**: `openspec/changes/desktop-test-only-flow/explore.md`（三轮探索已收敛：类型区分 + test-only 六相位 + 表内自描述前置依赖 + detail.pipeline 出线承载列序 + promote 类型推算）

---

## 问题

`packages/desktop` 的 change 工作流目前只有 `requirement` 一种类型，但 `workflow_type` 字段其实**早已埋在数据面**（`ChangeStateRecord.workflow_type: String`）：建档时硬编码为 `"requirement"`（`write/create.rs:205`），`phase_table(workflow_type)` 只认 `"requirement"`（其余一律 `None`，`write/phase_table.rs:523`），`code-analyze` 只是 10 列布局里的**占位列**（无 PhaseDefinition、无 step 序列、无 prompt），前端完全不知道 workflow_type（`ChangeDetail` / `ChangeSummary` 不透出，列布局靠前端写死的 `PIPELINE_PHASES` 10 站）。

需求是把「类型区分 + test-only 流程」补全：

1. **相位组合随类型分派**：`requirement` 与 `test-only` 的相位组合（有序列表）不同；同一 phase 无论出现在哪个类型里，其 phase → step 映射一致（决策 2：先复用同名 static）。
2. **code-analyze 缺位补全**：`test-only` 需要 `code-analyze` 相位，而它当前无 `CODE_ANALYZE_STEPS`、无 prompt、无 artifact 清单。
3. **前置依赖类型感知**：`prerequisites(phase)` 目前是 requirement 写死的独立 match，`test-only` 的前置依赖不同，backtrack 的 stale 传播需要类型感知。
4. **列序权威错位**：`PIPELINE_PHASES` 前后端双侧同步，是「10 列恒定」的布局常量；类型区分后列序必须随 `workflow_type` 出线，否则 test-only 会错误沿用 requirement 的 10 列。
5. **入口缺类型**：手动新建与 explore promote 两条建档入口都不带 `workflow_type`，promote 路径也没有「探索产出 → 类型推算 → 回填」的机制。

explore 已把方案收敛为「把埋好的 `workflow_type` 缝从单值硬编码打通成两值分派，并补上 `code-analyze` 相位」，三轮逐轮定稿见下。

## 提案

打通 `workflow_type` 的两值分派，并补全 test-only 流程与入口类型选择：

1. **相位表两值分派 + test-only 表**：`phase_table(workflow_type)` 改为 `"requirement"` → `REQUIREMENT_TABLE`（9 相位，不变）、`"test-only"` → `TEST_ONLY_TABLE`（6 相位：proposal → code-analyze → test-design → test-gen → test-execution → archive），其余 → `None`（**不退化**）。`TEST_ONLY_TABLE` 复用 requirement 的同名 step 静态（proposal / test-design / test-gen / test-execution 的 step 序列与 prompt 全复用；test-execution 沿用 desktop 确定性格：`[PhaseStart, TestExecution, PhaseLog]`，零 agent，不照搬插件侧 Executor→E）；archive 复用 `ARCHIVE_STEPS`；**新增 `CODE_ANALYZE_STEPS`**（P→E，与 proposal/dev-design 同形：`[PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]`）+ 新增 `code-analyze-planner.md` / `code-analyze-evaluator.md` 两个角色 prompt。
2. **前置依赖表内自描述**：`PhaseDefinition` 新增 `prerequisites: &'static [&'static str]` 字段，删除独立 `prerequisites()` match；`dependents(table, phase)` 改为遍历 `candidate.prerequisites.contains(&phase)`。requirement 照搬现有 match；test-only 照 pge spec（proposal=[]，code-analyze=[proposal]，test-design=[proposal, code-analyze]，test-gen=[test-design]，test-execution=[test-gen]，archive=[]）。
3. **读写都严格（不 fallback requirement）**：`phase_table` 只认 `{requirement, test-only}`。写面（`phase_next` / `phase_start` / `phase_log` / `backtrack`）与发起面（`change_flow_start`）对未知类型显式 `Err`（文案从「V1 仅 requirement」改为通吃 `{requirement, test-only}` 的未知类型拒绝）；读面（`change_detail` 建站 + 产物清单）对未知类型不 fallback requirement——`change_detail` 当前返回 `Option<ChangeDetail>`，未知类型的呈现形态（返回 None / 空 pipeline / 改 Result 签名）由 design 定稿，但 MUST NOT 静默当 requirement 跑。存量 `ChangeRecord.workflow_type` 恒为 `"requirement"`（写面 V1 起硬编码），严格校验后存量天然合法，零迁移。
4. **列序由 detail.pipeline 出线承载**：后端 `queries/detail.rs` 的 `PIPELINE_PHASES` 常量退役，`change_detail` 建站改用 `phase_table(record.workflow_type)` 的相位序（requirement 9 列、test-only 6 列；archive 在 requirement 为第 9 列、在 test-only 为第 6 列）。前端 `flow/layout.ts` 的 `PIPELINE_PHASES` 退役为测试夹具常量（或彻底删除），列序 = `detail.pipeline.map(p => p.phase)`；`graph.ts` / `run-state.ts` / `attachments.ts` 四处消费面改为按出线列序（`runStepNodes(steps, phaseOrder)` 显式传序，colIndex = 数组下标）。
5. **建档入口透出类型**：`write::create` 增加 `workflow_type` 入参（替换硬编码）；`create_change` IPC 由三参 `(root, name, goal)` 扩为四参 `(root, name, goal, workflow_type)`（`create_change_with` 泛型缝同扩）；`promote_explore` IPC 由 `(root, name)` 扩为 `(root, name, workflow_type)`，`write::promote_explore` 加 `workflow_type` 透传 `create`。手动新建对话框与 explore 详情 promote 入口各加一个两选项普通 select（requirement / test-only），**不抽共享组件**（架构边界优先，无单次使用抽象）。
6. **promote 类型推算（title 同模式）**：`ExploreRecord` v2 → v3 新增 `workflow_type` 字段（decode-only 升级默认 `"requirement"`）；`buildExplorePrompt` 的 stance 增加「推算变更类型并写约定标记行」约定（与首行 `# <标题>` 同构，具体格式由 design 定稿）；前端新增 `extractWorkflowType` 解析 + `update_explore_workflow_type` 显式回填命令（沿 `extractMarkdownTitle` / `update_explore_title` 先例）；promote 入口类型选择器默认 = 推算值（可改 requirement / test-only），人工修改经回填落库（沿 title 改即写模式）。
7. **「10 列恒定」spec 随本次一起改**：`desktop-change-flow-view` 与 `desktop-change-queries` 里「10 列恒定」「code-analyze 占位列」「archive 恒第 10 列」等断言改写为「列总数 = 该 change 相位表长度、列序随 workflow_type 出线」，测试夹具与 golden 显式重写。

版本交付：`packages/desktop` 0.4.40 → 0.4.41（归档时执行）；`plugins/dev-team` 零改动（保持 2.10.44）。

## 关键决策

explore 三轮逐轮定稿，每项列明理由与被拒绝的备选方案：

| # | 决策 | 理由 | 被拒绝的备选方案 |
|---|------|------|------------------|
| D1 | `TEST_ONLY_TABLE` = 6 相位（5 路由 + archive） | desktop 的 archive 是 `stop_before=archive` 手动门 + 确定性相位，requirement 同理；pge spec 是插件侧叙述，desktop 侧 archive 进表不冲突 | 5 相位（不含 archive，照抄 pge spec）：与 desktop requirement「archive 表内第 9 相位」形状不一致 |
| D2 | phase→step 先复用：proposal / test-design / test-gen / test-execution 复用 requirement 同名 static（step 序列 + prompt 全复用）；code-analyze 新增 | 同一 phase 的 step 映射不受类型影响；test-execution 沿用 desktop 确定性格（`[PhaseStart, TestExecution, PhaseLog]`），不照搬插件侧 Executor→E（既有差异非本次引入） | test-only 各相位独立 prompt（测试向提案独立 prompt 留待看效果，暂不拆） |
| D3 | `PhaseDefinition` 加 `prerequisites` 字段，删独立 `prerequisites()` match，`dependents` 遍历表内自描述 | 表内自描述杜绝第二份 match，类型感知随表分派天然成立 | 保留独立函数做类型分派：第二份 match，随类型增多重复发散 |
| D4 | 列序由 `detail.pipeline` 出线承载，前端 `PIPELINE_PHASES` 退役，`runStepNodes(steps, phaseOrder)` 显式传序 | `detail.pipeline` 已按相位序出线（PhaseEntry[]），前端零第二份类型→相位表，杜绝双侧同步漂移 | 只透 workflow_type、前端自持 type→phases 表：回到双侧同步 |
| D5 | 读写都严格，不 fallback requirement；存量 ChangeRecord 恒 requirement（零迁移）、ExploreRecord v3 默认 requirement | 正常形态不存在未知类型 change；脏值不应被静默当 requirement 跑 | 未知类型静默跑 requirement（被第三轮推翻）：掩盖脏数据 |
| D6 | promote 类型推算走 title 同模式（笔记写约定标记行 → 前端解析 → 显式回填） | 与 `extractMarkdownTitle` / `update_explore_title` 同构，零新机制 | 独立 tool / 收敛事件路线：成本高 |
| D7 | `ExploreRecord.workflow_type` 单字段（人工覆盖后回填落库，promote 读它） | 沿 title 改即写模式，语义最简 | suggested vs selected 双字段：两处真相源，无必要 |
| D8 | 类型选择器不抽共享组件，promote 与 change_create 各自内联两选项普通 select | 跨入口复用一个两选项 select 属 speculative abstraction，违背「无单次使用抽象」纪律 | 抽 `WorkflowTypeSelect` 共享组件：单次复用抽象 |

## 能力

### 新增能力

- 无（本变更修改八个既有能力）。

### 修改的能力

- **desktop-workflow-write-face** — `phase_table` 两值分派（`requirement` → 9 相位 / `test-only` → 6 相位，未知 → `None`）；新增 `TEST_ONLY_TABLE` 与 `CODE_ANALYZE_STEPS` + 两个 code-analyze 角色 prompt；`PhaseDefinition` 加 `prerequisites` 字段、删除独立 `prerequisites()` match、`dependents` 改表内遍历；写面「V1 仅 requirement」文案改为严格 `{requirement, test-only}` 分派。
- **desktop-change-queries** — `detail.rs` 的 `PIPELINE_PHASES` 常量退役，`change_detail` 建站改用 `phase_table(workflow_type)` 相位序；列总数随相位表长度（requirement 9 列、test-only 6 列）；未知类型读面不 fallback requirement（呈现形态 design 定稿）。
- **desktop-change-flow-view** — 前端 `PIPELINE_PHASES` 退役，列序改由 `detail.pipeline.map(p => p.phase)` 出线；`graph.ts` / `run-state.ts` / `attachments.ts` / `layout.ts` 四处消费面随动；「10 列恒定」「code-analyze 占位列」「archive 恒第 10 列」断言改写为「列总数随相位表长度」。
- **desktop-change-create** — `write::create` 与 `create_change` / `create_change_with` 增加 `workflow_type` 入参；新建对话框加类型选择器（默认 requirement）；「工作流类型恒 requirement」V1 边界修订。
- **desktop-change-orchestration** — `change_flow_start` 前置校验保留「`phase_table` None → 显式拒绝」门，仅把文案改为通吃 `{requirement, test-only}`；「仅 requirement 工作流」V1 边界修订。
- **desktop-explore-page** — stance 增加类型推算约定；前端新增 `extractWorkflowType` 解析与类型回填（沿 title 先例）；promote 入口加类型选择器（默认 = 推算值，可改）。
- **desktop-explore-queries** — `promote_explore` IPC 签名 `(root, name)` → `(root, name, workflow_type)`；`write::promote_explore` 加 `workflow_type` 透传 `create`；新增 `update_explore_workflow_type` 显式回填命令。
- **desktop-workspace-store** — `ExploreRecord` v2 → v3 新增 `workflow_type`（decode-only 升级默认 `"requirement"`）；新增 `set_explore_workflow_type` 操作面。

### 引用沿用（零 delta）

- **desktop-change-state-store** — `ChangeRecord.workflow_type` 字段既有（v4 已含），本变更零 model delta；存量恒 requirement 天然合法，严格校验由写面 / 查询面消费。
- **desktop-ipc-type-bindings** — `create_change` / `promote_explore` / `update_explore_workflow_type` 的签名变化经 tauri-specta 再生与一致性守卫既有纪律承载，机制零改动。
- **desktop-corpus-regression** — detail 线面 `pipeline` 列序 / 列数随 workflow_type 变化，golden 走 `DESKTOP_GOLDEN_REWRITE=1` 显式重写流程。
- **test-only-workflow** / **pge-workflow-engine** — 插件侧产品叙述与前置依赖规范性数据，desktop 相位表引用其 test-only 前置依赖（proposal=[]，code-analyze=[proposal]，test-design=[proposal, code-analyze]，test-gen=[test-design]，test-execution=[test-gen]），两能力零 delta。

---

## 变更范围

### 实现文件

- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table.rs` — `PhaseDefinition` 加 `prerequisites`；删 `prerequisites()` 独立 match；`dependents` 改 `candidate.prerequisites.contains(&phase)`；新增 `CODE_ANALYZE_STEPS` + `TEST_ONLY_TABLE`；`phase_table()` 两值分派（`TEST_ONLY_WORKFLOW_TYPE` 常量）
- `packages/desktop/src-tauri/crates/core/workflow/src/prompts/code-analyze-planner.md` + `code-analyze-evaluator.md` — 新增（全静态角色知识 md，`include_str!` 显式装配）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/create.rs` — `create` 增加 `workflow_type` 入参，替换硬编码 `"requirement"`（前置校验 workflow_type 为合法值）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/promote.rs` — `promote_explore` 增加 `workflow_type` 入参并透传 `create`
- `packages/desktop/src-tauri/crates/core/workflow/src/write/{phase_next,phase_start,phase_log,backtrack}.rs` — 「V1 仅 requirement」文案改为严格 `{requirement, test-only}` 分派文案（未知类型显式 `Err`）
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail.rs` — `PIPELINE_PHASES` 常量退役；`change_detail` 建站改 `phase_table(record.workflow_type)` 相位序；未知类型呈现形态 design 定稿（None / 空 pipeline / Result）
- `packages/desktop/src-tauri/src/commands/changes/mod.rs` — `create_change` / `create_change_with` 增加 `workflow_type` 入参并透传写面（IPC 四参）
- `packages/desktop/src-tauri/src/commands/explores/mod.rs` — `promote_explore` / `promote_explore_with` 增加 `workflow_type` 入参；新增 `update_explore_workflow_type` 命令
- `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` — 前置校验文案改通吃 `{requirement, test-only}`（未知类型仍显式拒绝）
- `packages/desktop/src-tauri/src/commands/mod.rs` — `all_commands!` 登记 `update_explore_workflow_type`
- `packages/desktop/src-tauri/crates/infra/store/src/model.rs` — `ExploreRecord` v2 → v3（`native_model version = 3, from = ExploreRecordV2`），加 `workflow_type`；`ExploreRecordV2` 历史形态 + 双向 `From`（默认 `"requirement"`）
- `packages/desktop/src-tauri/crates/infra/store/src/store.rs` — `set_explore_workflow_type(root, name, workflow_type)` 操作面（in-place 写 + 刷新 `updated_at`；空白 / 非法值 → `Err`）；`ExploreRecord::new` 默认 `"requirement"`
- `packages/desktop/src/lib/explore-stance.ts` — stance 增加「推算变更类型并写约定标记行」约定（与首行 `# <标题>` 同构，格式 design 定稿）
- `packages/desktop/src/lib/explore-title.ts`（或旁置新文件） — 新增 `extractWorkflowType` 纯函数
- `packages/desktop/src/views/explores/explore-detail-view.tsx` — `useWorkflowTypeBackfill`（沿 `useTitleBackfill`）；promote 入口加类型选择器（默认 = 推算值，`promote(name, workflowType)`）
- `packages/desktop/src/views/explores/hooks/use-explore-list.ts` — `promote(name, workflowType)` 签名随动
- `packages/desktop/src/views/changes/components/change-create-dialog.tsx` — 加类型选择器（默认 requirement，内联普通 select）；`createChange(root, name, goal, workflowType)`
- `packages/desktop/src/views/changes/flow/layout.ts` — `PIPELINE_PHASES` 退役为测试夹具常量（或彻底删除）
- `packages/desktop/src/views/changes/flow/graph.ts` — `buildFlowGraph` 列序改 `detail.pipeline.map(p => p.phase)`；`tableEdges` 仍读 `detail.pipeline`
- `packages/desktop/src/views/changes/flow/run-state.ts` — `runStepNodes(steps, phaseOrder)` 显式传序（colIndex = 数组下标），去掉 `PIPELINE_PHASES.indexOf`
- `packages/desktop/src/views/changes/flow/attachments.ts` — `artifactColumn` 改收 pipeline 相位集（或 phaseOrder），去掉 `PIPELINE_PHASES.indexOf`
- `packages/desktop/src/types/generated/bindings.ts` — 随 `create_change` / `promote_explore` / `update_explore_workflow_type` 签名再生
- `packages/desktop/package.json` — `version` 0.4.40 → 0.4.41（归档时执行）

### 测试文件

- `packages/desktop/src-tauri/crates/core/workflow/src/write/phase_table_test.rs` — `phase_table("test-only")` 六相位序、未知类型 `None`、`CODE_ANALYZE_STEPS` 形态、`prerequisites` 表内自描述、`dependents` 遍历正确（requirement 与 test-only 两表）
- `packages/desktop/src-tauri/crates/core/workflow/src/write/{phase_next,phase_start,phase_log,backtrack}_test.rs` — 未知类型显式 `Err` 文案、test-only 类型分派可走通
- `packages/desktop/src-tauri/crates/core/workflow/src/write/create_test.rs`（或既有 create 测试）— `workflow_type` 入参落库、非法值拒绝
- `packages/desktop/src-tauri/crates/core/workflow/src/queries/detail_test.rs` — requirement 9 列 / test-only 6 列 pipeline、未知类型呈现形态、golden 显式重写
- `packages/desktop/src-tauri/src/commands/changes/mod_test.rs` — `create_change` 四参签名与 `workflow_type` 透传
- `packages/desktop/src-tauri/src/commands/explores/mod_test.rs` — `promote_explore` 三参签名 + `workflow_type` 透传、`update_explore_workflow_type` 回填
- `packages/desktop/src-tauri/crates/infra/store/src/model_test.rs`（或既有 store 测试）— `ExploreRecord` v2 → v3 升级默认 `"requirement"`、`set_explore_workflow_type` 写入 / 拒绝面
- `packages/desktop/src/lib/explore-title.test.ts`（或旁置）— `extractWorkflowType` 命中 / 空 / 非约定行
- `packages/desktop/src/views/explores/explore-detail-view.test.tsx` — 类型回填 + promote 类型选择器（默认推算值、可改、传参）
- `packages/desktop/src/views/changes/components/change-create-dialog.test.tsx` — 类型选择器默认 requirement + 四参 invoke
- `packages/desktop/src/views/changes/flow/{graph,run-state,attachments,layout}.test.ts` — 列序由 `detail.pipeline` 出线（requirement 9 列 / test-only 6 列）、`runStepNodes(steps, phaseOrder)` 显式传序、pipeline 外相位不入图

### 删除文件

- 无（`PIPELINE_PHASES` 常量为退役 / 收缩，不删整文件；`prerequisites()` 独立函数随 phase_table.rs 内编辑删除）

### 不要修改

- `plugins/dev-team` 全部（版本 2.10.44、三类交付产物；插件侧 `workflow.ts` 相位表 / `test-only-workflow` 产品叙述零改动）
- `crates/core/workflow/src/prompts/` 既有 14 个角色 md（prompt 全静态零占位红线不翻案；仅新增 code-analyze 两 md）
- `crates/core/orchestration` 的 walker 步执行 / 会话 provenance / 决策协议 / 检查域 / vcs 缝（walker 零自持转移规则不变，相位表分派由写面承担，walker 无 workflow_type 分支）
- `ChangeRecord` 模型与 `desktop-change-state-store` 的 change 状态模型（`workflow_type` 字段既有，零 model delta；不建新表）
- `RunNodeRecord` / `RunStepKind` / `StepNode` 十八值词汇（code-analyze 复用既有 Executor / Evaluator / VerdictGate 步词，零新步词）
- `workflow_type` 合法值集的前端第二份「值表」权威（前端仅两处内联 select 字面选项，后端 `phase_table` 仍是唯一权威）

---

## 验收标准

| ID | 变更实现 | 验收条件 |
|----|---------|----------|
| AC-1 | 相位表两值分派 | `phase_table("requirement")` 返回 9 相位（proposal…archive）、`phase_table("test-only")` 返回 6 相位（proposal / code-analyze / test-design / test-gen / test-execution / archive）、未知类型返回 `None`；`TEST_ONLY_TABLE` 的 proposal / test-design / test-gen / test-execution 复用 requirement 同名 step 静态，archive 复用 `ARCHIVE_STEPS`，code-analyze 用新增 `CODE_ANALYZE_STEPS`（P→E） |
| AC-2 | code-analyze 相位补全 | `CODE_ANALYZE_STEPS` = `[PhaseStart, Executor, Evaluator, VerdictGate, PhaseLog]`；`prompts/code-analyze-planner.md` / `code-analyze-evaluator.md` 存在且经 `include_str!` 显式装配；code-analyze 无 artifact_manifest（产物清单空） |
| AC-3 | 前置依赖表内自描述 | `PhaseDefinition` 含 `prerequisites: &'static [&'static str]`；`phase_table.rs` 无独立 `prerequisites()` match；`dependents(table, phase)` 遍历 `candidate.prerequisites.contains(&phase)`；test-only 的 code-analyze=[proposal]、test-design=[proposal, code-analyze]、test-gen=[test-design]、test-execution=[test-gen]、proposal/archive=[]，backtrack stale 传播在 test-only 表下正确 |
| AC-4 | 读写都严格 | `phase_next` / `phase_start` / `phase_log` / `backtrack` 与 `change_flow_start` 对 `phase_table` 返回 `None` 的 change 显式 `Err`（文案通吃 `{requirement, test-only}`）；`change_detail` 对未知类型不 fallback requirement（呈现形态 design 定稿，MUST NOT 静默当 requirement 跑）；存量 requirement 记录零迁移、照常可读可推进 |
| AC-5 | 列序随 workflow_type 出线 | `queries/detail.rs` 无 `PIPELINE_PHASES` 常量；`change_detail` 的 `pipeline` 按 `phase_table(workflow_type)` 相位序出线（requirement 9 列、test-only 6 列，archive 列位置随表）；前端列序 = `detail.pipeline.map(p => p.phase)`，`graph.ts` / `run-state.ts` / `attachments.ts` 零 `PIPELINE_PHASES.indexOf` |
| AC-6 | 前端 test-only 视图 | 打开 test-only change 详情：流程图 6 列（proposal / code-analyze / test-design / test-gen / test-execution / archive），code-analyze 列不再是占位列（节点上图），archive 为第 6 列；打开 requirement change：9 列（无 code-analyze 占位列，archive 第 9 列） |
| AC-7 | 建档入口透出类型 | `create_change(root, name, goal, workflow_type)` 四参经 `create_change_with` 透传写面，`ChangeRecord.workflow_type` = 入参（test-only 传入则落 test-only）；新建对话框类型选择器默认 requirement、可改 test-only；非法 workflow_type 显式 `Err` 零建档 |
| AC-8 | promote 类型推算与透传 | `buildExplorePrompt` 的 stance 含类型推算约定；`extractWorkflowType` 解析笔记约定标记并 `update_explore_workflow_type` 回填（`read_explore` 路径零写入）；promote 入口类型选择器默认 = 推算值、可改；`promote_explore(root, name, workflow_type)` 三参经写面 `promote_explore` 透传 `create`，铸出 change 的 `workflow_type` = 用户最终选择 |
| AC-9 | ExploreRecord v3 | `ExploreRecord` native_model version 3，含 `workflow_type`；存量 v2 记录打开自动升级读出 `workflow_type = "requirement"`（无手工迁移）；`set_explore_workflow_type` 写入合法值、空白 / 非法值 `Err` |
| AC-10 | 全管线通过 | `pnpm -C packages/desktop run client:check`（fmt / lint / knip）与 `pnpm -C packages/desktop run test` 全绿；`cargo test --workspace`（src-tauri 根）全绿；bindings 一致性守卫绿；golden 经 `DESKTOP_GOLDEN_REWRITE=1` 显式重写且无新增豁免 |
| AC-11 | desktop 版本升级（归档时执行） | `packages/desktop/package.json` `version` 0.4.40 → 0.4.41；`tauri.conf.json` 维持 `../package.json` 引用，`src-tauri/Cargo.toml` 版本不随动；`plugins/dev-team` 保持 2.10.44 零改动 |

---

## 风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| R1 「10 列恒定」约束被推翻 | `desktop-change-flow-view` / `desktop-change-queries` 大量「恒 10 列」「archive 第 10 列」「code-analyze 占位」断言与测试夹具全量重写，front golden 漂移 | 高（确定发生） | AC-5/6/10 显式断言新列序；golden 走 `DESKTOP_GOLDEN_REWRITE=1` 显式重写留痕；`layout.test.ts` / `graph.test.ts` / `change-flow-graph.test.tsx` 同步重写 |
| R2 未知 workflow_type 读面的呈现形态未定 | `change_detail` 对脏值若静默当 requirement 会掩盖脏数据，若改 Result 会牵动 IPC 面 | 中 | 探索定稿「读写都严格，不 fallback」；呈现形态（None / 空 pipeline / Result）交 design 三选一，AC-4 显式断言「MUST NOT 静默当 requirement」 |
| R3 test-only 复用 requirement prompt 语义边界 | test-only 的 proposal「测试向提案」语义若与 requirement 提案混同，产出偏离 | 中 | 决策 2 先复用（step 结构 + prompt 全复用），是否拆独立 test-proposal prompt **留待看效果**；code-analyze prompt 为新增，语义由新 md 承载 |
| R4 promote 类型推算的 agent 输出不可控 | stance 引导不足时 agent 不写约定标记或写错值，类型回填失败 | 中 | stance 给出判断标准（「目标是补测试 / 生成测试用例 → test-only；否则 requirement」）；`extractWorkflowType` 解析失败保持 `record.workflow_type` 不变（零报错），人工可在 promote 入口改选 |
| R5 入口类型选择器不抽共享组件带来的两处内联 | 两处 select 的选项字面量 / 默认值语义各自演化，未来漂移 | 低 | 决策 8 有意收敛（无单次使用抽象）；合法值集权威仍为后端 `phase_table`，前端仅字面选项；两处默认值语义在 spec 各自显式（手动 requirement / promote 推算值） |
| R6 IPC 签名变化牵动 bindings 与既有测试锚 | `create_change` 三参 → 四参、`promote_explore` 二参 → 三参打破既有签名断言（mod_test.rs:734 等） | 中 | AC-7/8 显式覆盖；bindings 经 tauri-specta 再生 + 一致性守卫；既有测试断言随签名同步改写 |
| R7 test-execution 复用 desktop 确定性格与插件侧 Executor→E 不一致 | 未来对照插件语义时被误读为 desktop 缺失执行步骤 | 低 | 探索已定性为**既有差异**（desktop `TEST_EXECUTION_STEPS` 为确定性门禁，walker 不消费 executor/evaluator）；spec 显式留痕「复用 = 沿用 desktop 确定性格」 |
