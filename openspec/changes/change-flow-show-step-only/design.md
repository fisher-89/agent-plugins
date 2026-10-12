# 设计: change-flow-show-step-only

> **变更**: change-flow-show-step-only
> **日期**: 2026-10-12
> **基于**: proposal.md、explore.md（已收敛五点）、specs/ 四个能力 delta（desktop-change-flow-view / desktop-checks-domain / desktop-test-execution / desktop-change-orchestration）

---

## 提案与规格同步状态

`proposal.md` 与 `specs/` 四份 delta 已由提案阶段定稿，本设计基于其文本展开，只覆盖「变更范围 - 实现文件」，并把探索收敛的五点与两项边界决策落到类型面 / 命令面 / 前端面 / 记录面的具体形态。

工作树实况（本变更起点，`packages/desktop` 0.4.37 基线）：

- `flow/graph.ts` 现为三分类归并：`collectEvals`（eval 骨干）+ `collectActive`（active）+ `mergeByStartAt` 插入 + runtime overlay 追加；`FlowNode = EvalFlowNode | ActiveFlowNode | RuntimeFlowNode`。
- `flow/attachments.ts` 现为双层挂载：文档挂列（`columnDocs`）+ eval-checklist 挂节点（`nodeChecklists`，经 `NODE_PRECEDENCE` / `locateNode` 按 `(phase, attempt)` 投影）。
- `flow/detail-drawer.tsx` 现为双列：左列会话转录 + 右列 `DrawerDocsSection` / `DrawerEvalSection`（含 `InlineChecklist`）。
- `infra/exec` 的 `record_exec_output` 现写 `.log` + `.err` 双文件；`read_exec_output` 读 `.log`。
- `infra/checks` 的 `ProcessStaticCheck` 现默认 `ExecOutput::Transient`（`record_anchor=None`），`recorded(change)` 升 Recorded。
- `infra/checks::testexec` 的 record 现为 `serde_json::to_vec(summary)` 重序列化（`record_summary`）。

本设计描述**本变更完成后的目标形态**：图节点单分类（仅 runtime step）、抽屉单列按信息源分派、static-check 默认 recorded + `2>&1` 交错、test-execution record 改脚本真实输出。

---

## 关键设计决策（proposal 定稿 + 本设计展开）

| # | 决策 | 定稿 | 理由 |
|---|------|------|------|
| D1 | 移除相位节点（eval + active），仅保留 runtime step 节点 | `graph.ts` 删 `collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs`；`sequence = [...runNodes]`；`FlowNode = RuntimeFlowNode`；`types.ts` 删 `FlowNodeKind` / `EvalFlowNode` / `ActiveFlowNode` | 相位节点与 step 节点信息重叠；attempt verdict 已由 step 状态承载，运行中由 running 态 runtime 节点 pulse + header Badge（`detail.activePhase`）承载；减转换 / 渲染 / 挂载优先级 / golden 全链成本 |
| D2 | 抽屉单列按信息源分派 | `role !== null` → 会话转录（复用 `SessionTranscriptPanel`）；`outputRef !== null` → stdout 面板（新组件）；列头 → 该站文档 + eval-checklist（`ArtifactTabs`）。删 `DrawerEvalSection` + `InlineChecklist` + eval/active 分支 | 单列更聚焦；eval report 随 eval 节点退役无承载，eval-checklist 素材上移列头 |
| D3 | 无点击判据：`sessionId`/`role` 与 `outputRef` 皆 null 不可点击 | `change-flow-graph.tsx` 的 `onNodeClick` 前置判据：runtime 节点 `role === null && outputRef === null` → 不 `onSelect`；列头仍可点（`column` 类型不受判据约束） | 流转两词 / gate 三门 / git 四步 / preflight / seal 的 detail 短文本已在节点内显示，抽屉无增量信息 |
| D4 | static-check 启用 recorded + spawn 层 `2>&1` 交错合并 | `script = format!("{command} 2>&1")`；仅 `.stdout(Stdio::piped())`（stderr 并入 stdout）；`diagnostics = output.stdout`（交错）、`record = record_exec_output(root, name, output.stdout)`；`ProcessStaticCheck` 去 `record_anchor` 字段与 `recorded()` 方法，`run(root, name)` | 交错顺序在分离收集时即丢失、record 层无法还原；OS 层 `2>&1` 使两 fd 指向同一管道、写入顺序即真实顺序，单一 `.log` 完整还原终端 |
| D5 | test-execution record 改脚本真实输出（stdout+stderr 合并，**不加 `2>&1`**） | `PlanExecution` 增 `output: Vec<u8>`（runner 捕获的 stdout_bytes + stderr_bytes 拼接）；`mod.rs` 聚合各 suite `output` 按 plan 序（`plans` 迭代序）拼接落单一 `.log`；`record_summary` 退役 | summary JSON 重序列化非脚本真实输出；rust 模板 `cargo test > "{results_file}"` 已重定向 stdout，`2>&1` 会把 stderr 混入污染 `parse_cargo_test` |
| D6 | eval-checklist 素材挂列头、eval report 不展示 | `attachments.ts` 的 eval-checklist 分支改走 `artifactColumn(payload.phase)` 挂 `columnDocs`；删 `NODE_PRECEDENCE` / `locateNode` / `nodeChecklists`；`mountMaterials(detail, artifacts)` 去 `graph` 参数 | payload 自带 `phase` + `attempt` + `verdict` + `items`，`artifactColumn` 已有 pipeline 外相位校验、`EvalChecklistRenderer` 已能渲染全套元数据，列头经 `ArtifactTabs` 即达（零成本） |
| D7 | 削减流转类 step（phase_start / phase_log）留后续 change | 本轮不做：后端 walker 源头仍照常 emit；`run-state.ts` / `StepNode` / `RunStepKind` / `ChangeStepKind` 词汇枚举保留；仅前端过滤不引入 | blast radius 大（walker 源头不 emit ×3 变体 + `entry_kind` 迁移 + 前端兜底），混淆本轮五点收敛 |
| D8 | stdout 面板与取数 hook 契约（本设计定稿） | `hooks/use-exec-output.ts` 导出 `useExecOutput({ root, changeName, execId }) -> { text: string | null; loading: boolean; error: string | null }`；`flow/stdout-panel.tsx` 导出 `StdoutPanel({ root, changeName, execId })` 单列 `<pre>` 文本区；`text === null`（miss）→ 空态，invoke 抛错 → 错误态（不炸图） | 命令返回 `Option<String>`（miss=None），磁盘读失败经 `.ok()` 归一为 None，故「空态」与「错误态」以 hook 内 invoke 拒绝面区分；`text` 为 UTF-8 lossy 字符串 |
| D9 | `record_exec_output` 签名收敛（单一 `.log`） | `record_exec_output(root: &Path, change: &str, output: &[u8]) -> Result<String, String>`；仅写 `<exec_id>.log`（`.err` 退役）；`read_exec_output` 读同一 `.log`（签名不变） | 单一交错流 / 合并流即完整还原终端，分文件 `.log`/`.err` 是分离快照、与终端不一致 |
| D10 | test-execution `output` 线程化（本设计定稿） | `runner.rs` 在 spawn 成功分支拼接 `let mut output = stdout_bytes; output.extend_from_slice(&stderr_bytes);`（超时分支 `output = Vec::new()`）；`mod.rs` 的 `execute` 循环内 `merged_output.extend_from_slice(&execution.output);` 后再 `build_sub_report(plan, execution, suites)`（借用后移动）；`outcome(..., &merged_output)` 落盘 | `build_sub_report` 按值消费 `PlanExecution`，须先借用复制 `output` 再移动；`outcome` 是 record 落盘单点 |

---

## 架构组件

| 组件 | 职责 | 文件位置 | 依赖 | 技术 |
|------|------|----------|------|------|
| 前端节点模型单分类 | `FlowNode = RuntimeFlowNode`；删 `FlowNodeKind` / `EvalFlowNode` / `ActiveFlowNode`；`FlowMaterials` 删 `nodeChecklists` | `src/views/changes/flow/types.ts` | `types/dto` | TypeScript（类型面） |
| 前端转换层单分类 | `buildFlowGraph` 删 eval/active 归并，`sequence = [...runNodes]`；`tableEdges` / `backtrackReason` / `assignOrders` 保留（`pipeline` 仍供 backtrack label） | `src/views/changes/flow/graph.ts` | `layout`、`run-state`、`types` | TypeScript（纯函数） |
| 前端渲染薄层交互判据 | 删 `nodeTypes.event` / `toChartNodes` 非 runtime 分支 / `onNodeClick` 'event' 判；`onNodeClick` 加双 null 不可点击判据 | `src/views/changes/flow/change-flow-graph.tsx` | `layout`、`run-step-node`、`types` | TypeScript、React、@xyflow/react |
| 前端素材挂载单层 | `mountMaterials(detail, artifacts)`：文档 + eval-checklist 均挂 `columnDocs`；删 `NODE_PRECEDENCE` / `locateNode` / `nodeChecklists` | `src/views/changes/flow/attachments.ts` | `layout`、`types` | TypeScript（纯函数） |
| 前端抽屉单列分派 | `DetailDrawer` 删 `DrawerEvalSection` + `InlineChecklist` + eval/active 分支；按 role → 转录 / outputRef → stdout / 列头 → 文档分派；新增 `name` prop | `src/views/changes/flow/detail-drawer.tsx` | `session-transcript-panel`、`stdout-panel`、`renderers/artifact-tabs`、`types` | TypeScript、React |
| 前端 stdout 面板与取数 | `StdoutPanel` 单列渲染脚本输出；`useExecOutput` 经 `commands.readExecOutput(root, changeName, execId)` 取数（UTF-8 lossy；miss 空态） | `src/views/changes/flow/stdout-panel.tsx` + `src/views/changes/hooks/use-exec-output.ts` | `types/generated/bindings` | TypeScript、React |
| 前端数据源与透传 | `useFlowGraphData` 的 materials memo 依赖收窄 `[detail, artifacts]`；`DetailDrawer` 传 `name={detail.name}` | `src/views/changes/change-detail-view.tsx` | `attachments`、`graph`、`detail-drawer`、hooks | TypeScript、React |
| static-check 进程执行 | `ProcessStaticCheck` 去 `record_anchor` 构造；`run(root, name)`；spawn `{command} 2>&1` 单一流；`diagnostics` = 交错 stdout；record = 单一 `.log` + `output_ref` 回填 | `crates/infra/checks/src/static_check.rs` | `exec`、`orchestration::port`、`config` | Rust、tokio async |
| 输出记录单点 | `record_exec_output` 收敛单一 `.log`（`.err` 退役）；`read_exec_output` 读同一 `.log`；`ExecOutput` 两模式不变 | `crates/infra/exec/src/lib.rs` | `foundation::layout` | Rust（sync） |
| test-execution 进程执行 | `PlanExecution` 增 `output: Vec<u8>`；`mod.rs` 聚合各 suite output 按 plan 序拼接落单一 `.log`，去 serde summary JSON | `crates/infra/checks/src/testexec/runner.rs` + `mod.rs` | `checks`、`exec`、`orchestration::port` | Rust、tokio async |
| static-check port 契约 | `StaticCheckRunner::run(root, name)` 对齐 `TestExecutionRunner::run(root, name)` | `crates/core/orchestration/src/port.rs` | — | Rust（trait 定义） |
| 编排消费点 name 解析 | `ToolCommand::StaticCheck` 分支经 `store.get_change(id).map(name)` 解析 name（抄 TestExecution 分支）；未建档显式 `Err` | `crates/core/orchestration/src/steps.rs` | `orchestration::port`、`store` | Rust、tokio async |
| 命令面装配 | `ProcessStaticCheck::new()` 构造不变（去 `recorded()` 装配分支、注释随动） | `src/commands/change_flow/mod.rs` | `checks_runtime`、`orchestration` | Rust、tauri |
| IPC 读面 | 新增 `read_exec_output(root, change_name, exec_id) -> Option<String>`（UTF-8 lossy；miss=None）暴露 `infra/exec::read_exec_output` | `src/commands/changes/mod.rs` | `exec`、tauri、specta | Rust、tauri IPC |
| 命令登记与类型再生 | `all_commands!` 注册 `read_exec_output`；`bindings.ts` 经 `bindings:export` 再生 `readExecOutput` typed 包装 | `src/commands/mod.rs` + `src/types/generated/bindings.ts` | — | Rust 宏 + codegen |
| 版本交付 | `version` 0.4.37 → 0.4.38（`tauri.conf.json` 经 `../package.json` 自动跟随，`Cargo.toml` 不随动） | `packages/desktop/package.json` | — | package.json |

---

## 变更清单

### 新增文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/views/changes/flow/stdout-panel.tsx` | stdout 单列面板：经 `useExecOutput` 取数，渲染脚本真实输出为单列 `<pre>` 文本区；miss → 空态、invoke 拒绝 → 错误态（不炸图）。见「公共函数 / API」的 `StdoutPanel` |
| `packages/desktop/src/views/changes/hooks/use-exec-output.ts` | `readExecOutput` 取数 hook：以 `(root, changeName, execId)` 调 `commands.readExecOutput`，返回 `{ text, loading, error }`。见「公共函数 / API」的 `useExecOutput` |

### 修改文件

| 文件路径 | 修改内容 | 说明 |
|----------|----------|------|
| `packages/desktop/src/views/changes/flow/types.ts` | 删 `FlowNodeKind` / `EvalFlowNode` / `ActiveFlowNode` 三类型；`FlowNode` 改 `type FlowNode = RuntimeFlowNode`；`FlowMaterials` 删 `nodeChecklists` 字段（仅留 `columnDocs`）；`FlowNodeBase` / `RuntimeFlowNode` / `FlowEdge` / `FlowGraph` / `DrawerSelection` / `RunStepGroup` / `FlowRoleLabel` / `RoleSessionRef` 保留 | 节点模型单分类 + 素材单层 |
| `packages/desktop/src/views/changes/flow/graph.ts` | 删 `collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs`；`buildFlowGraph` 内 `sequence = [...runNodes]`（不再 eval 骨干 + active 插入）；`tableEdges` / `backtrackReason` / `assignOrders` / `colIndexOf` 保留（`pipeline` 仍供 backtrack label）；imports 收窄（去 `EvalFlowNode` / `ActiveFlowNode`） | 转换层单分类 |
| `packages/desktop/src/views/changes/flow/change-flow-graph.tsx` | 删 `flow-event-node` import 与 `EventFlowNode` / `nodeTypes.event`；`ChartNode` 收敛为 `ColumnFlowNode | RunStepFlowNode`；`toChartNodes` 删非 runtime 分支（`events` flatMap 恒产 `runStep`）；`onNodeClick` 删 `node.type === 'event'` 判、`runStep` 分支加 `role === null && outputRef === null` 不可点击判据 | 渲染薄层单分类 + 交互判据 |
| `packages/desktop/src/views/changes/flow/attachments.ts` | 删 `NODE_PRECEDENCE` / `locateNode` / `nodeChecklists`；`mountMaterials` 去 `graph` 参数（签名见「公共函数 / API」）；eval-checklist 分支改走 `artifactColumn(envelope.payload.phase)` 挂 `columnDocs`（`isChecklistRef` / `artifactColumn` 保留） | 素材挂列头、单层 |
| `packages/desktop/src/views/changes/flow/detail-drawer.tsx` | 删 `DrawerEvalSection` + `InlineChecklist` + `DrawerDocsSection` 右列结构 + `RightSections` + eval/active 分支；改单列分派：`selection.scope === 'column'` → `ArtifactTabs(columnDocs)`；runtime 节点 `role !== null` → `SessionTranscriptPanel`（`selectionRoleRefs` 收窄只处理 runtime）；`outputRef !== null` → `StdoutPanel`；新增 `name` prop | 抽屉单列 + name 透传 |
| `packages/desktop/src/views/changes/change-detail-view.tsx` | `useFlowGraphData` 的 materials memo 依赖收窄 `[detail, artifacts]`（去 `graph`）；`mountMaterials(detail, artifacts)` 调用随签名；`DetailDrawer` 传 `name={detail.name}` | 数据源收窄 + name 透传 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/port.rs` | `StaticCheckRunner::run(&self, root: &str)` → `run(&self, root: &str, name: &str)`（对齐 `TestExecutionRunner`）；`StaticCheckOutcome` 语义不变（`output_ref` 恒回填） | port 契约 name 化 |
| `packages/desktop/src-tauri/crates/core/orchestration/src/steps.rs` | `ToolCommand::StaticCheck { change_id }` 分支：先 `store.get_change(&change_id).map(|record| record.name)` 解析 name（未建档显式 `Err`，抄 TestExecution 分支），再 `static_check.run(&step.root, &name)` | 消费点 name 解析 |
| `packages/desktop/src-tauri/crates/infra/checks/src/static_check.rs` | `ProcessStaticCheck` 删 `record_anchor` 字段与 `recorded()` 方法；`run(root, name)`；`execute(root, name)`；spawn 改 `exec::SystemCommand::script(&format!("{command} 2>&1"))` 且仅 `.stdout(Stdio::piped())`（去 `.stderr`）；`diagnostics = output.stdout`（交错）；`output_ref = exec::record_exec_output(Path::new(root), name, &output.stdout).ok()` | static-check recorded + 2>&1 |
| `packages/desktop/src-tauri/crates/infra/exec/src/lib.rs` | `record_exec_output(root: &Path, change: &str, output: &[u8])`：仅写 `<exec_id>.log`（删 `.err` 写入）；`read_exec_output` 读同一 `.log`（实现不变，注释随动）；`ExecOutput` 枚举与 `mint_exec_id` / `is_valid_exec_id` / `exec_output_dir` 保留 | 输出记录单点收敛 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/runner.rs` | `PlanExecution` 增 `output: Vec<u8>`；spawn 成功分支拼接 `let mut output = stdout_bytes; output.extend_from_slice(&stderr_bytes);`（`output_text` 改自 `&output` 派生供 error 尾部）；超时分支 `output = Vec::new()`；`PlanExecution { ..., output }` | 脚本真实输出捕获 |
| `packages/desktop/src-tauri/crates/infra/checks/src/testexec/mod.rs` | `execute` 循环内 `merged_output.extend_from_slice(&execution.output)` 后 `build_sub_report(plan, execution, suites)`；`outcome` 增 `merged_output: &[u8]` 参数并经 `exec::record_exec_output(root, change, merged_output)` 落单一 `.log`；删 `record_summary`（去 serde summary JSON） | 聚合 + 脚本输出落盘 |
| `packages/desktop/src-tauri/src/commands/changes/mod.rs` | 新增 `read_exec_output` 命令（签名见「公共函数 / API」）；`use exec;` import | IPC 读面 |
| `packages/desktop/src-tauri/src/commands/mod.rs` | `all_commands!` 增 `$crate::commands::changes::read_exec_output` | 命令登记 |
| `packages/desktop/src-tauri/src/commands/change_flow/mod.rs` | `Arc::new(ProcessStaticCheck::new())` 装配不变；删「默认 Transient / 升 recorded(change)」注释段、改「默认 Recorded」注释 | 构造器简化随动 |
| `packages/desktop/src/types/generated/bindings.ts` | 随 `read_exec_output` 命令经 `bindings:export` 再生（`readExecOutput` typed 包装出线；非手改） | 类型再生跟随 |
| `packages/desktop/package.json` | `version` 0.4.37 → 0.4.38 | 版本交付 |

### 删除文件

| 文件路径 | 说明 |
|----------|------|
| `packages/desktop/src/views/changes/flow/flow-event-node.tsx` | 事件节点组件随 eval/active 相位节点退役删除（其测试 `flow-event-node.test.tsx` 一并删除，由测试相位承接） |

> **零改动沿用**（proposal「变更范围 - 实现文件」之外或明确不动的既有面）：`flow/layout.ts`（`PIPELINE_PHASES` 10 列、`COL_W`/`ROW_H` 常量不变）、`flow/run-state.ts`（`runStepNodes` / `unifiedRunNodes` / `unifiedRunEdges` / `nodeIdentityKey` 与词汇归一零 delta，`outputRef` / `sessionId` / `role` 已出线）、`flow/run-step-node.tsx` / `phase-column-node.tsx` / `session-transcript-panel.tsx`（复用零改动）、`renderers/`（`ArtifactTabs` / `EvalChecklistRenderer` / `artifact-view` 已能渲染全套元数据）、`desktop-change-queries` / `desktop-change-state-store` 的 `output_ref` 线面与落库（零 delta）、`plugins/dev-team`（零改动）。

### 公共函数 / API

| 标识符 | 所在文件 | 新增/修改 | 签名 | 说明 |
|--------|----------|----------|------|------|
| `buildFlowGraph` | `src/views/changes/flow/graph.ts` | 修改 | `export function buildFlowGraph(detail: ChangeDetail, runNodes?: RuntimeFlowNode[], runEdges?: readonly ChangeRunEdgeRecord[]): FlowGraph` | 签名不变；内部删 eval/active 归并，`sequence = [...runNodes]`；`pipeline` 仍供 `tableEdges` backtrack label |
| `mountMaterials` | `src/views/changes/flow/attachments.ts` | 修改 | `export function mountMaterials(detail: ChangeDetail, envelopes: ArtifactEnvelope[]): FlowMaterials` | 去 `graph` 参数；文档 + eval-checklist 均挂 `columnDocs` |
| `useExecOutput` | `src/views/changes/hooks/use-exec-output.ts` | 新增 | `export function useExecOutput(params: { root: string \| null; changeName: string \| null; execId: string \| null }): { text: string \| null; loading: boolean; error: string \| null }` | 经 `commands.readExecOutput(root, changeName, execId)` 取数；`text === null` = miss 空态；invoke 拒绝 → `error` |
| `StdoutPanel` | `src/views/changes/flow/stdout-panel.tsx` | 新增 | `export function StdoutPanel(props: { root: string \| null; changeName: string; execId: string }): React.JSX.Element` | 单列 `<pre>` 渲染脚本输出；miss 空态 / 错误态不炸图 |
| `DetailDrawer` | `src/views/changes/flow/detail-drawer.tsx` | 修改 | `export function DetailDrawer({ selection, graph, materials, root, changeId, name, transcriptRefreshKey, onClose }: DetailDrawerProps): React.JSX.Element \| null` | 增 `name: string`（`read_exec_output` 磁盘面按 change name 定位）；单列分派 |
| `ChangeFlowGraph` | `src/views/changes/flow/change-flow-graph.tsx` | 修改 | `export function ChangeFlowGraph(props: ChangeFlowGraphProps): React.JSX.Element` | 签名不变（`graph` / `materials` / `onSelect`）；删 event 通道、加点击判据 |
| `record_exec_output` | `crates/infra/exec/src/lib.rs` | 修改 | `pub fn record_exec_output(root: &Path, change: &str, output: &[u8]) -> Result<String, String>` | 单一 `.log`（`.err` 退役）；`output` 为单一交错 / 合并流 |
| `read_exec_output` | `crates/infra/exec/src/lib.rs` | 修改 | `pub fn read_exec_output(root: &Path, change: &str, exec_id: &str) -> Option<Vec<u8>>` | 读同一 `.log`（实现不变，注释随动）；miss = None |
| `ProcessStaticCheck::new` | `crates/infra/checks/src/static_check.rs` | 修改 | `pub fn new() -> Self` | 去 `record_anchor` 字段；删 `recorded()` 方法；默认即 recorded |
| `StaticCheckRunner::run`（trait） | `crates/core/orchestration/src/port.rs` | 修改 | `fn run(&self, root: &str, name: &str) -> BoxToolFuture` | name 供 infra 侧 output 落盘 |
| `execute_plan` | `crates/infra/checks/src/testexec/runner.rs` | 修改 | `pub(crate) async fn execute_plan(plan: &TestPlan, report_dir: &Path) -> Result<PlanExecution, String>` | 签名不变；`PlanExecution.output` 增合并流 |
| `outcome` | `crates/infra/checks/src/testexec/mod.rs` | 修改 | `fn outcome(root: &Path, change: &str, summary: &SummaryReport, sub_reports: &[SubReport], reports_dir: &Path, merged_output: &[u8]) -> ToolStepOutput` | 增 `merged_output`；record 改脚本输出 |
| `record_summary` | `crates/infra/checks/src/testexec/mod.rs` | 删除（退役） | — | serde summary JSON 重序列化退役 |
| `read_exec_output`（IPC） | `src/commands/changes/mod.rs` | 新增 | `#[tauri::command] #[specta::specta] pub fn read_exec_output(root: String, change_name: String, exec_id: String) -> Option<String>` | UTF-8 lossy；miss = None；暴露 `infra/exec::read_exec_output` |

### 类型定义

| 类型名 | 所在文件 | 新增/修改 | 说明 |
|--------|----------|----------|------|
| `FlowNodeKind` | `src/views/changes/flow/types.ts` | 删除（退役） | `'eval' \| 'active'` 概念整体退役 |
| `EvalFlowNode` | `src/views/changes/flow/types.ts` | 删除（退役） | eval 事件节点退役 |
| `ActiveFlowNode` | `src/views/changes/flow/types.ts` | 删除（退役） | active 事件节点退役 |
| `FlowNode` | `src/views/changes/flow/types.ts` | 修改 | `type FlowNode = RuntimeFlowNode`（单分类） |
| `FlowMaterials` | `src/views/changes/flow/types.ts` | 修改 | 仅 `columnDocs: Record<string, ArtifactEnvelope[]>`；删 `nodeChecklists` |
| `PlanExecution` | `crates/infra/checks/src/testexec/runner.rs` | 修改 | 增 `output: Vec<u8>`（stdout+stderr 合并流，不加 2>&1）；`exit_code` / `duration_ms` / `cases` / `source_files` / `measured` / `error` 语义不变 |
| `ProcessStaticCheck` | `crates/infra/checks/src/static_check.rs` | 修改 | 删 `record_anchor` 私有字段（`pub struct` 零字段 / 单位语义，构造 `new()` 不变） |

### 配置

| 配置键 | 所在文件 | 新增/修改 | 值类型 | 默认值 | 说明 |
|--------|----------|----------|--------|--------|------|
| `version` | `packages/desktop/package.json` | 修改 | string | `0.4.38` | 0.4.37 → 0.4.38（`src-tauri/tauri.conf.json` 经 `../package.json` 自动跟随；`src-tauri/Cargo.toml` 版本不随动） |

---

## 数据模型

### 前端节点模型（单分类）

`FlowNode = RuntimeFlowNode`，字段（`types.ts`，前端转换 / 渲染薄层消费，无持久化）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `string` | `run:<phase>:<attempt>:<step>:<instanceNo>`（`run-state.ts` 派生） |
| `kind` | `'runtime'` | 恒值（单分类） |
| `phase` / `attempt` / `colIndex` / `order` / `parentId` | `string` / `number` / `number` / `number` / `string` | 列容器与列内归并序 |
| `runStepKind` | `ChangeStepKind` | 十八值全词汇（不变） |
| `group` | `RunStepGroup` | `workerAgent \| toolStep \| git \| gate` |
| `role` | `FlowRoleLabel \| null` | WorkerAgent 步角色（executor / evaluator / decision / spec_sync / resolve_conflicts），其余 null |
| `instanceNo` / `status` / `sessionId` / `detail` / `outputRef` | `number` / `ChangeStepStatus` / `string \| null` / `string \| null` / `string \| null` | 稳定身份键第四段与载荷八面 |

### 前端素材模型（单层）

`FlowMaterials = { columnDocs: Record<string, ArtifactEnvelope[]> }`——键为列 id `col:<phase>`，值为该站过程文档（`markdown-doc` / `tasks-progress`）+ eval-checklist 信封（挂列头后经 `ArtifactTabs` 渲染全套元数据）。`nodeChecklists` 退役。

### 节点点击分派判据（纯函数，交互层）

```
runtime 节点点击 →
  role !== null       → 单列会话转录（SessionTranscriptPanel，sessionId 直查优先 / sourceRef 反查兜底）
  outputRef !== null  → 单列 stdout 面板（StdoutPanel，经 read_exec_output）
  两者皆 null          → 不可点击（不 onSelect；detail 短文本已在节点内显示）
列头点击               → 单列该站文档 + eval-checklist（ArtifactTabs）
```

### 后端输出记录模型（单文件）

- 记录目录：`<report_dir>/exec-outputs/<exec_id>.log`（`foundation::layout::change_test_reports(root, change)` 下，与 test-execution 报告树同锚）。
- `record_exec_output(root, change, output)` 仅写单一 `.log`；`.err` 退役。`read_exec_output(root, change, exec_id)` 读同一 `.log`，未知 / 非法 `exec_id` → `None`。
- static-check：`output` = `2>&1` 交错 stdout（stderr 空），`diagnostics` 逐字等于该流。
- test-execution：`output` = 各 suite 的 `PlanExecution.output`（stdout+stderr 合并，不加 `2>&1`）按 plan 序拼接。

### 持久化

- 零新表。`output_ref`（`RunNodeRecord.output_ref`）落库引用既有，语义不变（exec_id 字符串）；`read_exec_output` 磁盘面按 change **name** 定位（与 `record_exec_output` 同口径），change **id** 仅作寻址键 / 会话 sourceRef 身份段。
- 会话记录仍经 `SessionEventRecord`（`session_id` 重放唯一来源），MUST NOT 内嵌进 `ExecOutput`（转录单表不变量不变）。

---

## 路由 / API 设计

| method | path | 说明 | 输入 | 输出 | auth |
|--------|------|------|------|------|------|
| IPC `read_exec_output` | — | 读 change 某 exec 输出的脚本真实输出（stdout 面板取数） | `root`, `change_name`, `exec_id` | `Option<String>`（UTF-8 lossy；miss = None） | 桌面本地 |
| IPC `get_change_detail` | — | 统一视图（库读史 ∪ 在飞 run 活面，`runs[].nodes[].outputRef` 线面既有） | `root`, `id` | `ChangeDetailUnified` | 桌面本地 |
| IPC `read_artifact` | — | 读单个产物（eval-checklist 信封 payload 自带 phase） | `root`, `id`, `kind`, `source` | `ArtifactEnvelope \| null` | 桌面本地 |

---

## 依赖

### 运行时依赖

- `infra/checks → infra/exec`（已有）：static-check / test-execution 经 `record_exec_output` / `read_exec_output` 单点落盘与回查。
- `orchestration → infra/checks`（经 port 注入，已有）：`StaticCheckRunner::run(root, name)` name 化由 `steps.rs` 消费点解析。
- `desktop-app → infra/exec`（已有，Cargo.toml `exec` 依赖）：`commands/changes/mod.rs` 新增 `use exec;` 暴露 `read_exec_output`。

### 构建 / 测试依赖

- 零新增 crate / 依赖；`bindings:export` / `bindings:check` / tsc / cargo 既有链路承载 `readExecOutput` 再生与一致性守卫。

---

## 待决问题

- 无未决事项。proposal 的 D1–D7 已在「关键设计决策」定稿，本设计补充 D8（stdout 面板 / hook 契约）、D9（`record_exec_output` 签名收敛）、D10（test-execution `output` 线程化）三点定稿，全部落到类型面 / 命令面 / 前端面 / 记录面。
