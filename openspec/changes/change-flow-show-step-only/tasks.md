# 任务: change-flow-show-step-only

> 测试文件的编写与执行由 test-design / test-gen / test-execution 阶段承接，本列表只覆盖实现文件。顺序按依赖排列：infra/exec 输出记录单点 → static-check recorded + 2>&1 → test-execution 脚本输出 → 编排消费点 name 解析 → IPC 命令面 → bindings 再生 → 前端节点模型单分类 → 素材挂载单层 → stdout 面板与取数 → 抽屉单列与数据源 → 版本交付。
>
> 依赖序要点：static-check / test-execution 均消费 `record_exec_output` 新签名（阶段二 / 三依赖阶段一）；`steps.rs` 消费 `StaticCheckRunner::run(root, name)` 新 port 签名（阶段四依赖阶段二）；`read_exec_output` 命令暴露 `infra/exec::read_exec_output`（阶段五依赖阶段一）；bindings 再生（阶段六）依赖命令面（阶段五）并先于前端 `use-exec-output`（阶段九，读 `bindings.ts` 的 `readExecOutput`）；前端 `attachments.ts`（阶段八）消费 `types.ts` 删 `nodeChecklists`（阶段七）；`change-detail-view.tsx`（阶段十）消费 `mountMaterials` 新签名（阶段八）与 `DetailDrawer.name` / `StdoutPanel`（阶段九 / 十内部序）。

## 阶段一：infra/exec 输出记录单点收敛

- [x] 修改 `crates/infra/exec/src/lib.rs`：`record_exec_output(root: &Path, change: &str, output: &[u8]) -> Result<String, String>`——仅写 `<exec_id>.log`（删 `.err` 写入分支）；`read_exec_output` 读同一 `.log`（实现不变，注释随动）；`ExecOutput` 枚举 / `mint_exec_id` / `is_valid_exec_id` / `exec_output_dir` 保留

## 阶段二：static-check recorded + `2>&1` 交错

- [x] 修改 `crates/core/orchestration/src/port.rs`：`StaticCheckRunner::run(&self, root: &str, name: &str) -> BoxToolFuture`（对齐 `TestExecutionRunner`；`StaticCheckOutcome` 语义不变）
- [x] 修改 `crates/infra/checks/src/static_check.rs`：`ProcessStaticCheck` 删 `record_anchor` 字段与 `recorded()` 方法；`StaticCheckRunner::run` 收 `name`；`execute(root, name)`；spawn 改 `exec::SystemCommand::script(&format!("{command} 2>&1"))` 且仅 `.stdout(Stdio::piped())`（去 `.stderr`）；`diagnostics = output.stdout`（交错）、`output_ref = exec::record_exec_output(Path::new(root), name, &output.stdout).ok()`

## 阶段三：test-execution record 改脚本真实输出

- [x] 修改 `crates/infra/checks/src/testexec/runner.rs`：`PlanExecution` 增 `output: Vec<u8>`；spawn 成功分支 `let mut output = stdout_bytes; output.extend_from_slice(&stderr_bytes);`（`output_text` 改自 `&output` 派生供 error 尾部），超时分支 `output = Vec::new()`；`PlanExecution { ..., output }`
- [x] 修改 `crates/infra/checks/src/testexec/mod.rs`：`execute` 循环内 `merged_output.extend_from_slice(&execution.output);` 后再 `build_sub_report(plan, execution, suites)`；`outcome` 增 `merged_output: &[u8]` 参数并经 `exec::record_exec_output(root, change, merged_output)` 落单一 `.log`（回填 `exec_id`）；删 `record_summary`（去 serde summary JSON）

## 阶段四：编排消费点 name 解析

- [x] 修改 `crates/core/orchestration/src/steps.rs`：`ToolCommand::StaticCheck { change_id }` 分支先 `store.get_change(&change_id).map(|record| record.name)` 解析 name（未建档显式 `Err`，抄 TestExecution 分支），再 `static_check.run(&step.root, &name)`
- [x] 修改 `src/commands/change_flow/mod.rs`：`Arc::new(ProcessStaticCheck::new())` 装配不变；删「默认 Transient / 升 recorded(change)」注释段、改「默认 Recorded」注释

## 阶段五：IPC 命令面

- [x] 修改 `src/commands/changes/mod.rs`：新增 `use exec;`；新增 `#[tauri::command] #[specta::specta] pub fn read_exec_output(root: String, change_name: String, exec_id: String) -> Option<String>`（`exec::read_exec_output(Path::new(&root), &change_name, &exec_id).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())`）
- [x] 修改 `src/commands/mod.rs`：`all_commands!` 增 `$crate::commands::changes::read_exec_output`

## 阶段六：bindings 再生

- [x] 执行 `pnpm -C packages/desktop run bindings:export` 再生 `src/types/generated/bindings.ts`（`readExecOutput` typed 包装出线；非手改）——先于阶段九前端 `use-exec-output`（读 `bindings.ts` 的 `readExecOutput`）

## 阶段七：前端节点模型单分类

- [x] 修改 `src/views/changes/flow/types.ts`：删 `FlowNodeKind` / `EvalFlowNode` / `ActiveFlowNode`；`FlowNode` 改 `type FlowNode = RuntimeFlowNode`；`FlowMaterials` 删 `nodeChecklists`（仅留 `columnDocs`）
- [x] 修改 `src/views/changes/flow/graph.ts`：删 `collectEvals` / `collectActive` / `mergeByStartAt` / `anchorMs`；`sequence = [...runNodes]`；`tableEdges` / `backtrackReason` / `assignOrders` / `colIndexOf` 保留（`pipeline` 仍供 backtrack label）；imports 收窄
- [x] 删除 `src/views/changes/flow/flow-event-node.tsx`
- [x] 修改 `src/views/changes/flow/change-flow-graph.tsx`：删 `flow-event-node` import / `EventFlowNode` / `nodeTypes.event`；`ChartNode` 收敛为 `ColumnFlowNode | RunStepFlowNode`；`toChartNodes` 删非 runtime 分支；`onNodeClick` 删 `'event'` 判、`runStep` 分支加 `role === null && outputRef === null` 不可点击判据（不 `onSelect`）

## 阶段八：前端素材挂载单层

- [x] 修改 `src/views/changes/flow/attachments.ts`：删 `NODE_PRECEDENCE` / `locateNode` / `nodeChecklists`；`mountMaterials(detail, envelopes)` 去 `graph` 参数；eval-checklist 分支改走 `artifactColumn(envelope.payload.phase)` 挂 `columnDocs`（`isChecklistRef` / `artifactColumn` 保留）

## 阶段九：stdout 面板与取数 hook

- [x] 新增 `src/views/changes/hooks/use-exec-output.ts`：`useExecOutput({ root, changeName, execId }) -> { text: string | null; loading: boolean; error: string | null }`——任一入参 null → 清空态；否则 `commands.readExecOutput(root, changeName, execId)` 取数（`text` = 返回串或 null = miss 空态；invoke 拒绝 → `error`）
- [x] 新增 `src/views/changes/flow/stdout-panel.tsx`：`StdoutPanel({ root, changeName, execId })` 单列 `<pre>` 渲染 `text`；`text === null` → 空态、`error !== null` → 错误态（不炸图）

## 阶段十：抽屉单列与数据源收窄

- [x] 修改 `src/views/changes/flow/detail-drawer.tsx`：删 `DrawerEvalSection` + `InlineChecklist` + `RightSections` + eval/active 分支；单列分派（列头 → `ArtifactTabs(columnDocs)`；runtime `role !== null` → `SessionTranscriptPanel`（`selectionRoleRefs` 收窄只处理 runtime）；`outputRef !== null` → `StdoutPanel`）；新增 `name: string` prop 透传给 `StdoutPanel`
- [x] 修改 `src/views/changes/change-detail-view.tsx`：`useFlowGraphData` 的 materials memo 依赖收窄 `[detail, artifacts]`（去 `graph`）；`mountMaterials(detail, artifacts)` 调用随签名；`DetailDrawer` 传 `name={detail.name}`

## 阶段十一：版本交付

- [x] 修改 `packages/desktop/package.json`：`version` 0.4.37 → 0.4.38（`tauri.conf.json` 经 `../package.json` 自动跟随，`Cargo.toml` 不随动）
